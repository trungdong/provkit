//! `provkit compare`: the records found on one side only, printed as
//! PROV-N.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Args;
use provkit::{Document, Error, FormalRef, Namespaces, QName, Record, provn};

use crate::{Hint, read_document, write_stdout};

/// Arguments of `provkit compare`.
#[derive(Args)]
pub struct CompareArgs {
    /// First document.
    a: PathBuf,
    /// Second document.
    b: PathBuf,
    /// Merge records that share an identifier on each side before comparing.
    #[arg(long)]
    unified: bool,
}

/// Exits 0 when the documents are equal, 1 when they differ.
pub fn run(args: &CompareArgs) -> Result<ExitCode> {
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
/// prefix stays in `namespaces` only when the name is written, because
/// `provn::name` leaves the table untouched on an error.
fn written_name(name: &QName, namespaces: &mut Namespaces) -> Option<String> {
    provn::name(name, namespaces).ok()
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
