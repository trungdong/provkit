//! The `provkit` command-line tool.

mod compare;
mod format;

use std::fs;
use std::io::{self, Read as _, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand};
use provkit::{Document, json, provn};

use compare::CompareArgs;
use format::Format;

/// Convert and compare W3C PROV documents.
#[derive(Parser)]
#[command(name = "provkit", version, about)]
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
    /// Formats are taken from -f when given, else from each file's extension
    /// (.json, .provn), else by looking at the content. Names the output
    /// cannot write with a declared prefix get an `nsN` prefix. A `prefix`
    /// line, printed after the records of its scope, applies to the whole
    /// scope. The listing is not a PROV-N document: its names resolve with
    /// the declarations of the file it came from.
    ///
    /// Exit codes: 0 when the documents are equal (nothing is printed), 1
    /// when they differ, 2 on any error.
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

/// How the tool names `path` in a message: `<stdin>` for `-`.
fn shown(path: &Path) -> String {
    if path == Path::new("-") {
        "<stdin>".to_owned()
    } else {
        path.display().to_string()
    }
}

/// A library error from reading `source`, as `source:line:column: message`
/// when the error has a position, so an editor can jump to it.
fn located(source: &str, error: provkit::Error) -> anyhow::Error {
    match error {
        provkit::Error::Syntax {
            line,
            column,
            message,
            ..
        } => anyhow!("{source}:{line}:{column}: {message}"),
        provkit::Error::At {
            line,
            column,
            error,
            ..
        } => anyhow!("{source}:{line}:{column}: {error}"),
        provkit::Error::Json(e) => match (e.line(), e.column()) {
            (Some(line), Some(column)) => anyhow!("{source}:{line}:{column}: {}", message_of(&e)),
            _ => anyhow!("{source}: {e}"),
        },
        other => anyhow!("{source}: {other}"),
    }
}

/// A `JsonError`'s message without the ` at line L column C` it ends with,
/// for the `file:line:column:` form that states the position once.
fn message_of(e: &provkit::JsonError) -> String {
    let text = e.to_string();
    let end = text.rfind(" at line ").unwrap_or(text.len());
    text[..end].to_owned()
}

/// Reads `path` (or standard input for `-`) and parses it with the resolved
/// format. Extensibility expressions the PROV-N reader ignores are reported
/// once on standard error.
fn read_document(path: &Path, from: Option<Format>) -> Result<Document> {
    let is_stdin = path == Path::new("-");
    let label = shown(path);
    let text = if is_stdin {
        io::read_to_string(io::stdin())
    } else {
        fs::read_to_string(path)
    }
    .map_err(|e| {
        let hint = if !is_stdin
            && e.kind() == io::ErrorKind::InvalidData
            && starts_with_utf16_mark(path)
        {
            "; the file starts with a UTF-16 byte-order mark; convert it to UTF-8"
        } else {
            ""
        };
        anyhow!("{label}: {e}{hint}")
    })?;
    // `-` has no extension, so standard input goes straight to sniffing.
    let format = from
        .or_else(|| Format::from_path(path))
        .or_else(|| Format::sniff(&text))
        .ok_or_else(|| {
            if is_stdin {
                anyhow!("cannot tell the format of standard input; use -f json or -f provn")
            } else {
                anyhow!("{label}: cannot tell the format; use -f json or -f provn")
            }
        })?;
    let document = match format {
        Format::Json => json::read(&text).map_err(|e| located(&label, e))?,
        Format::Provn => {
            let report = provn::read_report(&text, provn::Profile::Default)
                .map_err(|e| located(&label, e))?;
            warn_ignored(&label, &report.ignored);
            report.document
        }
    };
    Ok(document)
}

/// Whether the file at `path` begins with a UTF-16 byte-order mark, little
/// or big endian. Read only after the UTF-8 read has failed.
fn starts_with_utf16_mark(path: &Path) -> bool {
    let mut head = [0u8; 2];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok_and(|()| matches!(head, [0xFF, 0xFE] | [0xFE, 0xFF]))
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
    let doc = read_document(&args.input, args.from)?;

    // Serialise first, so a library error never leaves a partial file.
    let mut buf = Vec::new();
    match format {
        Format::Json => json::write(&doc, &mut buf),
        Format::Provn => provn::write(&doc, &mut buf),
    }
    .with_context(|| shown(&args.input))?;
    match file {
        None => write_stdout(&buf).context("<stdout>")?,
        Some(path) => fs::write(path, &buf).with_context(|| path.display().to_string())?,
    }
    Ok(())
}
