//! The `provkit` command-line tool.

mod format;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand};
use provkit::{Document, json, provn};

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

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Convert(args) => convert(&args),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
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
    let mut out: Box<dyn Write> = match file {
        None => Box::new(io::stdout().lock()),
        Some(path) => Box::new(io::BufWriter::new(
            fs::File::create(path).with_context(|| path.display().to_string())?,
        )),
    };
    match format {
        Format::Json => json::write(&doc, &mut out)?,
        Format::Provn => provn::write(&doc, &mut out)?,
    }
    out.flush()?;
    Ok(())
}
