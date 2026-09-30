//! The `provkit` command-line tool.

mod format;

use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand};
use provkit::{Document, Error, FormalRef, Namespaces, QName, Record, json, provn};

use format::Format;

/// Convert and compare W3C PROV documents.
///
/// Formats are taken from -f/-t when given, else from the file extension
/// (.json, .provn), else by looking at the content. Standard output defaults
/// to PROV-N, and a file named with -o needs -t unless its extension is .json
/// or .provn. Use `-` for standard input or output.
#[derive(Parser)]
#[command(name = "provkit", version, about, long_about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Convert a document between PROV-JSON and PROV-N.
    ///
    /// Formats are taken from -f/-t when given, else from the file extension
    /// (.json, .provn), else by looking at the content. Standard output
    /// defaults to PROV-N, and a file named with -o needs -t unless its
    /// extension is .json or .provn. Use `-` for standard input or output.
    Convert(ConvertArgs),
    /// Compare two documents and print the records found on one side only.
    ///
    /// Formats come from the file extension (.json, .provn), else from the
    /// content. Names the output cannot write with a declared prefix get an
    /// `nsN` prefix, declared after the records of their scope. Exit codes: 0 when the documents are equal (nothing is printed), 1
    /// when they differ, 2 on a read error or, with --unified, a unification
    /// failure.
    Compare(CompareArgs),
}

#[derive(Args)]
struct ConvertArgs {
    /// Input file, or - for standard input.
    input: PathBuf,
    /// Output file, or - for standard output (the default).
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Input format; otherwise the extension, then the content, decide.
    #[arg(short = 'f', long = "from", value_enum)]
    from: Option<Format>,
    /// Output format; otherwise the output extension decides (.json or
    /// .provn), or PROV-N on standard output. Required for a file with any
    /// other extension.
    #[arg(short = 't', long = "to", value_enum)]
    to: Option<Format>,
}

#[derive(Args)]
struct CompareArgs {
    /// First document.
    a: PathBuf,
    /// Second document.
    b: PathBuf,
    /// Merge records that share an identifier on each side before comparing.
    #[arg(long)]
    unified: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Convert(args) => convert(&args).map(|()| ExitCode::SUCCESS),
        Command::Compare(args) => compare(&args),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("provkit: {e:#}");
            ExitCode::from(2)
        }
    }
}

/// How a failure to tell the format is worded.
#[derive(Clone, Copy)]
enum Hint {
    /// The command has `-f`, so the message names it.
    FromFlag,
    /// The command has no `-f`.
    None,
}

/// Reads `path` (or standard input for `-`) and parses it with the resolved
/// format. Extensibility expressions the PROV-N reader ignores are reported
/// once on standard error.
fn read_document(path: &Path, from: Option<Format>, hint: Hint) -> Result<Document> {
    let is_stdin = path == Path::new("-");
    let text = if is_stdin {
        io::read_to_string(io::stdin()).context("-")?
    } else {
        fs::read_to_string(path).with_context(|| path.display().to_string())?
    };
    let format = from
        .or_else(|| {
            if is_stdin {
                None
            } else {
                Format::from_path(path)
            }
        })
        .or_else(|| Format::sniff(&text))
        .ok_or_else(|| match (is_stdin, hint) {
            (true, Hint::None) => anyhow!("cannot tell the format of standard input"),
            (false, Hint::None) => anyhow!("{}: cannot tell the format", path.display()),
            (_, Hint::FromFlag) => anyhow!(
                "{}: cannot tell the format; use -f json or -f provn",
                path.display()
            ),
        })?;
    let document = match format {
        Format::Json => json::read(&text),
        Format::Provn => provn::read_report(&text, provn::Profile::Default).map(|(document, ignored)| {
            if let Some(first) = ignored.first() {
                let file = if is_stdin {
                    "standard input".to_owned()
                } else {
                    path.display().to_string()
                };
                eprintln!(
                    "provkit: warning: {file}: ignored {} extensibility expression(s), first {} at line {}, column {}",
                    ignored.len(),
                    first.name,
                    first.line,
                    first.column
                );
            }
            document
        }),
    };
    document.with_context(|| path.display().to_string())
}

/// Writes `bytes` to standard output. A closed pipe is a normal end.
fn write_stdout(bytes: &[u8]) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    match stdout.write_all(bytes).and_then(|()| stdout.flush()) {
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other,
    }
}

fn convert(args: &ConvertArgs) -> Result<()> {
    let file = args.output.as_deref().filter(|p| *p != Path::new("-"));
    let format = match (args.to, file) {
        (Some(format), _) => format,
        (None, None) => Format::Provn,
        (None, Some(path)) => Format::from_path(path).ok_or_else(|| {
            anyhow!(
                "cannot tell the output format from '{}'; use -t json or -t provn",
                path.display()
            )
        })?,
    };
    let doc = read_document(&args.input, args.from, Hint::FromFlag)?;
    let name = file.map_or_else(|| "-".to_owned(), |p| p.display().to_string());

    // Serialise first, so a library error never leaves a partial file.
    let mut buf = Vec::new();
    match format {
        Format::Json => json::write(&doc, &mut buf),
        Format::Provn => provn::write(&doc, &mut buf),
    }
    .with_context(|| name.clone())?;
    match file {
        None => write_stdout(&buf).with_context(|| name)?,
        Some(path) => fs::write(path, &buf).with_context(|| name)?,
    }
    Ok(())
}

/// Exits 0 when the documents are equal, 1 when they differ.
fn compare(args: &CompareArgs) -> Result<ExitCode> {
    let mut a = read_document(&args.a, None, Hint::None)?;
    let mut b = read_document(&args.b, None, Hint::None)?;
    if args.unified {
        a = a.unified().with_context(|| args.a.display().to_string())?;
        b = b.unified().with_context(|| args.b.display().to_string())?;
    }
    if a == b {
        return Ok(ExitCode::SUCCESS);
    }

    // Build the report first, so a failure never leaves half of it printed.
    let mut report = String::new();
    report_side(&mut report, &args.a, &a, &b)?;
    report_side(&mut report, &args.b, &b, &a)?;
    write_stdout(report.as_bytes()).context("-")?;
    Ok(ExitCode::from(1))
}

/// The records of `mine` that are not in `theirs`, in order, each once.
fn only_in<'a>(mine: &[&'a Record], theirs: &[&Record]) -> Vec<&'a Record> {
    let theirs: HashSet<&Record> = theirs.iter().copied().collect();
    let mut seen = HashSet::new();
    mine.iter()
        .copied()
        .filter(|r| !theirs.contains(r) && seen.insert(*r))
        .collect()
}

/// Appends the records of `this` that `other` lacks: top-level records first,
/// then each bundle under its heading. Each scope renders against one table
/// and declares the prefixes it minted after its records.
fn report_side(out: &mut String, name: &Path, this: &Document, other: &Document) -> Result<()> {
    use std::fmt::Write as _;

    let top = only_in(
        &this.records.iter().collect::<Vec<_>>(),
        &other.records.iter().collect::<Vec<_>>(),
    );

    // Bundles sharing an identifier are one bundle, as in `Document` equality.
    let this_bundles = this.merged_bundles();
    let other_bundles = other.merged_bundles();
    let mut bundles = Vec::new();
    for bundle in this_bundles.iter() {
        let theirs = other_bundles.iter().find(|b| b.id == bundle.id);
        let records = only_in(
            &bundle.records.iter().collect::<Vec<_>>(),
            &theirs.map_or_else(Vec::new, |b| b.records.iter().collect()),
        );
        if !records.is_empty() || theirs.is_none() {
            bundles.push((bundle, records));
        }
    }
    if top.is_empty() && bundles.is_empty() {
        return Ok(());
    }

    writeln!(out, "only in {}:", name.display())?;
    let mut table = this.namespaces.clone();
    let declared = table.iter().count();
    for record in top {
        writeln!(out, "  {}", statement_line(record, &mut table)?)?;
    }
    declare_minted(out, "  ", &table, declared)?;
    for (bundle, records) in bundles {
        // A bundle sees its own prefixes first, then the document's.
        let mut scope = bundle.namespaces.with_fallback(&this.namespaces);
        let declared = scope.iter().count();
        match written_name(&bundle.id, &mut scope) {
            Some(id) => writeln!(out, "  bundle {id}:")?,
            None => writeln!(out, "  bundle <{}>: {UNWRITABLE}", bundle.id)?,
        }
        for record in records {
            writeln!(out, "    {}", statement_line(record, &mut scope)?)?;
        }
        declare_minted(out, "    ", &scope, declared)?;
    }
    Ok(())
}

/// Prints a `prefix` line for each binding `table` gained after its first
/// `declared` ones.
fn declare_minted(
    out: &mut String,
    indent: &str,
    table: &Namespaces,
    declared: usize,
) -> std::fmt::Result {
    use std::fmt::Write as _;

    for (prefix, iri) in table.iter().skip(declared) {
        writeln!(out, "{indent}prefix {prefix} <{iri}>")?;
    }
    Ok(())
}

/// `name` as PROV-N text, or `None` when PROV-N cannot write it. A minted
/// prefix stays in `namespaces` only when the name is written.
fn written_name(name: &QName, namespaces: &mut Namespaces) -> Option<String> {
    let mut trial = namespaces.clone();
    let text = provn::name(name, &mut trial).ok()?;
    *namespaces = trial;
    Some(text)
}

/// Marks a line whose record or bundle PROV-N cannot write.
const UNWRITABLE: &str = "// not valid PROV-N";

/// `record` as PROV-N, or, when PROV-N cannot write a name in it, a heading
/// that names it: the keyword, the identifier, and the name PROV-N cannot
/// write as `<full IRI>` (inside `[...]` when it belongs to an attribute),
/// followed by [`UNWRITABLE`]. Prefixes minted for the line stay in
/// `namespaces`.
fn statement_line(record: &Record, namespaces: &mut Namespaces) -> Result<String> {
    let mut trial = namespaces.clone();
    let offending = match provn::statement(record, &mut trial) {
        Ok(line) => {
            *namespaces = trial;
            return Ok(line);
        }
        Err(Error::Unwritable { name, .. }) => name,
        Err(e) => return Err(e.into()),
    };

    let mut parts = Vec::new();
    if let Some(id) = record.id() {
        parts.push(written_name(id, namespaces).unwrap_or_else(|| format!("<{id}>")));
    }
    if Some(&offending) != record.id() {
        // Attribute-side names sit inside the attribute brackets, as in PROV-N.
        let is_formal = record
            .formals()
            .into_iter()
            .any(|(_, f)| matches!(f, FormalRef::Name(q) if *q == offending));
        parts.push(if is_formal {
            format!("<{offending}>")
        } else {
            format!("[<{offending}>]")
        });
    }
    let separator = if record.kind().is_element() {
        ", "
    } else {
        "; "
    };
    Ok(format!(
        "{}({}) {UNWRITABLE}",
        record.kind().name(),
        parts.join(separator)
    ))
}

#[cfg(test)]
mod tests {
    use provkit::Bundle;

    use super::*;

    fn read(text: &str) -> Document {
        provn::read(text).unwrap()
    }

    /// A hand-built document may repeat a bundle identifier, which the
    /// readers reject. Equality groups such bundles, so the report must too.
    #[test]
    fn same_id_bundles_report_as_one() {
        let one = read(
            "document\n prefix ex <http://example.org/>\n bundle ex:b\n  entity(ex:e1)\n  entity(ex:e2)\n endBundle\nendDocument\n",
        );
        let source = one.bundles[0].clone();
        let mut split = Document::new();
        split.namespaces = one.namespaces.clone();
        for record in &source.records {
            split.bundles.push(Bundle {
                id: source.id.clone(),
                namespaces: source.namespaces.clone(),
                records: vec![record.clone()],
            });
        }
        assert_eq!(split.bundles.len(), 2);
        assert_eq!(split, one);

        let mut out = String::new();
        report_side(&mut out, Path::new("a"), &split, &one).unwrap();
        report_side(&mut out, Path::new("b"), &one, &split).unwrap();
        assert_eq!(out, "");
    }
}
