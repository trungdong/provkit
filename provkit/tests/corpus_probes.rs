//! The shared PROV-N probes in `corpus/provn/probes`: every probe reads or
//! fails as `expected.tsv` says, under both profiles.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::{Report, corpus_dir, files, only, tsv};
use provkit::provn::{self, Profile};

/// Number of probes in the corpus.
const PROBE_COUNT: usize = 215;

/// The columns of `expected.tsv`.
const HEADER: [&str; 5] = ["name", "strict", "default", "rule", "provpy"];

/// What a profile must do with a probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Accept,
    Reject,
}

impl Outcome {
    fn parse(word: &str) -> Option<Outcome> {
        match word {
            "accept" => Some(Outcome::Accept),
            "reject" => Some(Outcome::Reject),
            _ => None,
        }
    }

    fn of(result: &provkit::Result<provkit::Document>) -> Outcome {
        if result.is_ok() {
            Outcome::Accept
        } else {
            Outcome::Reject
        }
    }
}

/// One row of `expected.tsv`.
struct Expected {
    strict: Outcome,
    default: Outcome,
}

/// The rows of `expected.tsv` by probe name, or the malformed rows.
fn expected(report: &mut Report) -> BTreeMap<String, Expected> {
    let rows = tsv(&corpus_dir("provn/probes").join("expected.tsv"));
    let (header, rows) = rows.split_first().expect("expected.tsv is empty");
    assert_eq!(header, &HEADER, "unexpected header");
    let mut by_name = BTreeMap::new();
    for row in rows {
        // The last column is empty, and may be absent, for a probe on which
        // ProvPy agrees.
        let ([name, strict, default, rule] | [name, strict, default, rule, _]) = row.as_slice()
        else {
            let name = row.first().map_or("", String::as_str);
            report.fail(name, format!("expected 5 columns, got {}", row.len()));
            continue;
        };
        let (Some(strict), Some(default)) = (Outcome::parse(strict), Outcome::parse(default))
        else {
            report.fail(name, "strict and default must be accept or reject");
            continue;
        };
        if rule.is_empty() {
            report.fail(name, "the rule column is empty");
        }
        if by_name
            .insert(name.clone(), Expected { strict, default })
            .is_some()
        {
            report.fail(name, "listed twice");
        }
    }
    by_name
}

#[test]
fn probes_read_as_the_grammar_requires() {
    let mut report = Report::default();
    let expected = expected(&mut report);
    let probes = files(&corpus_dir("provn/probes"), "provn");
    let names: BTreeSet<&str> = probes
        .iter()
        .map(|(file, _)| file.trim_end_matches(".provn"))
        .collect();
    for (file, text) in &probes {
        let name = file.trim_end_matches(".provn");
        report.checked += 1;
        let Some(row) = expected.get(name) else {
            report.fail(name, "no row in expected.tsv");
            continue;
        };
        for (label, profile, want) in [
            ("strict", Profile::Strict, row.strict),
            ("default", Profile::Default, row.default),
        ] {
            let got = Outcome::of(&provn::read_with(text, profile));
            if got != want {
                report.fail(name, format!("{label}: expected {want:?}, got {got:?}"));
            }
        }
    }
    if only().is_none() {
        for name in expected.keys() {
            if !names.contains(name.as_str()) {
                report.fail(name, "row has no probe file");
            }
        }
    }
    report.finish(PROBE_COUNT);
}
