//! The `provkit` command-line tool.

mod compare;
mod format;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand};
use provkit::{Document, json, provn};

use compare::CompareArgs;
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
    /// `nsN` prefix. A `prefix` line, printed after the records of its scope,
    /// applies to the whole scope.
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
    /// Output format; otherwise the output extension decides (.json or
    /// .provn), or PROV-N on standard output. Required for a file with any
    /// other extension.
    #[arg(short = 't', long = "to", value_enum)]
    to: Option<Format>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Convert(args) => convert(&args).map(|()| ExitCode::SUCCESS),
        Command::Compare(args) => compare::run(&args),
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
        Format::Provn => {
            provn::read_report(&text, provn::Profile::Default).map(|(document, ignored)| {
                let source = if is_stdin {
                    "standard input".to_owned()
                } else {
                    path.display().to_string()
                };
                warn_ignored(&source, &ignored);
                document
            })
        }
    };
    document.with_context(|| path.display().to_string())
}

/// Prints one warning on standard error for the extensibility expressions
/// the PROV-N reader left out of `source`, if any.
fn warn_ignored(source: &str, ignored: &[provn::Ignored]) {
    if let Some(first) = ignored.first() {
        eprintln!(
            "provkit: warning: {source}: ignored {} extensibility expression(s), first {} at line {}, column {}",
            ignored.len(),
            first.name,
            first.line,
            first.column
        );
    }
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
