//! The `provkit` command-line tool.

mod format;

use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand};
use provkit::{Document, Record, json, provn};

use format::Format;

/// Convert and compare W3C PROV documents.
///
/// Formats are taken from -f/-t when given, else from the file extension
/// (.json, .provn), else by looking at the content. Standard output defaults
/// to PROV-N. Use `-` for standard input or output.
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
    /// defaults to PROV-N. Use `-` for standard input or output.
    Convert(ConvertArgs),
    /// Compare two documents and print the records found on one side only.
    ///
    /// Exit codes: 0 when the documents are equal (nothing is printed), 1
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
    /// Output format; otherwise the output extension decides, or PROV-N on
    /// standard output.
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

/// Reads `path` (or standard input for `-`) and parses it with the resolved
/// format.
fn read_document(path: &Path, from: Option<Format>) -> Result<Document> {
    let is_stdin = path == Path::new("-");
    let mut text = if is_stdin {
        io::read_to_string(io::stdin()).context("-")?
    } else {
        fs::read_to_string(path).with_context(|| path.display().to_string())?
    };
    // A leading byte-order mark is an encoding signature, not content.
    if text.starts_with('\u{FEFF}') {
        text.drain(..'\u{FEFF}'.len_utf8());
    }
    let format = from
        .or_else(|| {
            if is_stdin {
                None
            } else {
                Format::from_path(path)
            }
        })
        .or_else(|| Format::sniff(&text))
        .ok_or_else(|| {
            anyhow!(
                "{}: cannot tell the format; use -f json or -f provn",
                path.display()
            )
        })?;
    let parsed = match format {
        Format::Json => json::read(&text),
        Format::Provn => provn::read(&text),
    };
    parsed.with_context(|| path.display().to_string())
}

fn convert(args: &ConvertArgs) -> Result<()> {
    let doc = read_document(&args.input, args.from)?;
    let file = args.output.as_deref().filter(|p| *p != Path::new("-"));
    let format = args
        .to
        .or_else(|| file.and_then(Format::from_path))
        .unwrap_or(Format::Provn);
    let name = file.map_or_else(|| "-".to_owned(), |p| p.display().to_string());

    // Serialise first, so a library error never leaves a partial file.
    let mut buf = Vec::new();
    match format {
        Format::Json => json::write(&doc, &mut buf),
        Format::Provn => provn::write(&doc, &mut buf),
    }
    .with_context(|| name.clone())?;
    match file {
        None => {
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(&buf)
                .and_then(|()| stdout.flush())
                .with_context(|| name)?;
        }
        Some(path) => fs::write(path, &buf).with_context(|| name)?,
    }
    Ok(())
}

/// Exits 0 when the documents are equal, 1 when they differ.
fn compare(args: &CompareArgs) -> Result<ExitCode> {
    let mut a = read_document(&args.a, None)?;
    let mut b = read_document(&args.b, None)?;
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
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(report.as_bytes())
        .and_then(|()| stdout.flush())
        .context("-")?;
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

/// The records of every bundle in `document` named `id`.
fn bundle_records<'a>(document: &'a Document, id: &provkit::QName) -> Vec<&'a Record> {
    document
        .bundles
        .iter()
        .filter(|b| b.id == *id)
        .flat_map(|b| &b.records)
        .collect()
}

/// Appends the records of `this` that `other` lacks: top-level records first,
/// then each bundle under its heading. Names use the table of `this`.
fn report_side(out: &mut String, name: &Path, this: &Document, other: &Document) -> Result<()> {
    use std::fmt::Write as _;

    let top = only_in(
        &this.records.iter().collect::<Vec<_>>(),
        &other.records.iter().collect::<Vec<_>>(),
    );

    // Bundles sharing an identifier are one bundle, as in `Document` equality.
    let mut seen_ids = HashSet::new();
    let mut bundles = Vec::new();
    for bundle in &this.bundles {
        if !seen_ids.insert(&bundle.id) {
            continue;
        }
        let records = only_in(
            &bundle_records(this, &bundle.id),
            &bundle_records(other, &bundle.id),
        );
        if !records.is_empty() || other.bundle(&bundle.id).is_none() {
            bundles.push((bundle, records));
        }
    }
    if top.is_empty() && bundles.is_empty() {
        return Ok(());
    }

    writeln!(out, "only in {}:", name.display())?;
    for record in top {
        writeln!(out, "  {}", provn::statement(record, &this.namespaces)?)?;
    }
    for (bundle, records) in bundles {
        // A bundle sees its own prefixes first, then the document's.
        let scope = bundle.namespaces.with_fallback(&this.namespaces);
        writeln!(
            out,
            "  bundle {}:",
            provn::statement_name(&bundle.id, &scope)?
        )?;
        for record in records {
            writeln!(out, "    {}", provn::statement(record, &scope)?)?;
        }
    }
    Ok(())
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
