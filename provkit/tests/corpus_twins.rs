//! PROV-N corpus files against their PROV-JSON twins, and a PROV-N round trip
//! of the PROV-JSON corpus.

mod common;

use common::{JSON_CORPUS_SIZE, Report, corpus_dir, files, only, pairs};
use provkit::Document;
use provkit::provn::Profile;

/// Why `ProvToolbox`'s writer output differs from its JSON twin
/// (`corpus/provn/README.md`, "Known differences").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Difference {
    /// The generator embeds the wall-clock time of the corpus build.
    DynamicTimestamp,
    /// The generator's datatype palette differs from the fixture's.
    DatatypePalette,
}

/// The twins not asserted equal, read from `tests/data/known_differences.tsv`
/// (file name, tab, category). `ProvPy` 3.3.0 keeps the same list.
fn known_differences() -> Vec<(String, Difference)> {
    pairs("known_differences.tsv")
        .into_iter()
        .map(|(name, category)| {
            let category = match category.as_str() {
                "DynamicTimestamp" => Difference::DynamicTimestamp,
                "DatatypePalette" => Difference::DatatypePalette,
                other => panic!("unknown category {other} for {name}"),
            };
            (name, category)
        })
        .collect()
}

/// The PROV-JSON twin of a PROV-N corpus file, as a per-file result.
fn json_twin(name: &str) -> Result<Document, String> {
    let path = corpus_dir("json").join(name.replace(".provn", ".json"));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    provkit::json::read(&text).map_err(|e| format!("JSON twin: {e}"))
}

#[test]
fn provtoolbox_corpus_equals_json_twins() {
    let mut report = Report::default();
    let known_differences = known_differences();
    let mut known = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (name, text) in files(&corpus_dir("provn/provtoolbox-corpus"), "provn") {
        seen.insert(name.clone());
        let outcome = json_twin(&name).and_then(|expected| {
            provkit::provn::read_with(&text, Profile::Default)
                .map(|d| d == expected)
                .map_err(|e| e.to_string())
        });
        report.checked += 1;
        let known_as = known_differences.iter().find(|(n, _)| *n == name);
        match (outcome, known_as) {
            // Only equality is waived for a known difference, not reading.
            (Err(e), _) => report.fail(&name, e),
            (Ok(equal), Some((_, category))) => known.push((name, *category, equal)),
            (Ok(true), None) => {}
            (Ok(false), None) => report.fail(&name, "differs from its PROV-JSON twin"),
        }
    }
    // Known differences are reported, and none may read as equal.
    for category in [Difference::DynamicTimestamp, Difference::DatatypePalette] {
        let of = |want: bool| {
            known
                .iter()
                .filter(|(_, c, equal)| *c == category && *equal == want)
                .map(|(n, _, _)| n.as_str())
                .collect::<Vec<_>>()
        };
        let (equal, unequal) = (of(true), of(false));
        eprintln!(
            "{category:?}: {} equal, {} unequal\n  equal: {}\n  unequal: {}",
            equal.len(),
            unequal.len(),
            equal.join(" "),
            unequal.join(" "),
        );
        assert!(
            equal.is_empty(),
            "{category:?} twins now read as equal: {}",
            equal.join(" ")
        );
    }
    if only().is_none() {
        // The list is checked both ways: every row names a file that was
        // read, once.
        assert_eq!(
            known_differences.len(),
            159,
            "rows in known_differences.tsv"
        );
        let mut listed = std::collections::HashSet::new();
        for (name, _) in &known_differences {
            assert!(listed.insert(name), "{name} is listed twice");
            assert!(seen.contains(name), "{name} names no corpus file");
        }

        let count = |want| known.iter().filter(|(_, c, _)| *c == want).count();
        assert_eq!(count(Difference::DynamicTimestamp), 80);
        assert_eq!(count(Difference::DatatypePalette), 79);
    }
    report.finish(388);
}

#[test]
fn json_corpus_round_trips_through_provn() {
    let mut report = Report::default();
    for (name, text) in files(&corpus_dir("json"), "json") {
        report.checked += 1;
        if let Some(document) = report.read_json(&name, &text) {
            report.round_trip(&name, &document, provkit::provn::to_string, |text| {
                provkit::provn::read_with(text, Profile::Default)
            });
        }
    }
    report.finish(JSON_CORPUS_SIZE);
}
