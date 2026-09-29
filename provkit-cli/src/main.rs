//! The `provkit` command-line tool.

use clap::Parser;

/// Convert and compare W3C PROV documents.
#[derive(Parser)]
#[command(name = "provkit", version, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
