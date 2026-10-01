//! PROV-N corpus: the Recommendation examples, `ProvToolbox`'s documents and
//! writer output, and a PROV-N round trip of the PROV-JSON corpus.

mod common;

use common::{JSON_CORPUS_SIZE, Report, corpus_dir, files};
use provkit::Document;
use provkit::provn::Profile;

/// Recommendation examples that the formal grammar rejects or that are not
/// complete PROV-N, from `tests/data/excluded_spec_examples.tsv` (file name,
/// tab, reason). Names and reasons follow `corpus/provn/README.md`.
fn excluded_spec_examples() -> Vec<(String, String)> {
    exclusions("excluded_spec_examples.tsv")
}

/// `ProvToolbox`'s own documents outside what the default profile reads,
/// from `tests/data/excluded_provtoolbox_documents.tsv` (file name, tab,
/// reason). Names and reasons follow `corpus/provn/README.md`.
fn excluded_provtoolbox_documents() -> Vec<(String, String)> {
    exclusions("excluded_provtoolbox_documents.tsv")
}

/// The (file name, reason) rows of an exclusion list in `tests/data`.
fn exclusions(file: &str) -> Vec<(String, String)> {
    common::tsv(&common::data_path(file))
        .into_iter()
        .map(|row| match row.as_slice() {
            [name, reason] => (name.clone(), reason.clone()),
            other => panic!("{file}: expected a name and a reason, got {other:?}"),
        })
        .collect()
}

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
/// (file name, tab, category). `ProvPy` 3.2.2 keeps the same list.
fn known_differences() -> Vec<(String, Difference)> {
    common::tsv(&common::data_path("known_differences.tsv"))
        .into_iter()
        .map(|row| match row.as_slice() {
            [name, category] => {
                let category = match category.as_str() {
                    "DynamicTimestamp" => Difference::DynamicTimestamp,
                    "DatatypePalette" => Difference::DatatypePalette,
                    other => panic!("unknown category {other} for {name}"),
                };
                (name.clone(), category)
            }
            other => panic!("expected a name and a category, got {other:?}"),
        })
        .collect()
}

#[test]
fn writes_all_398() {
    let mut report = Report::default();
    for (name, text) in files(&corpus_dir("json"), "json") {
        report.checked += 1;
        let Some(document) = report.read_json(&name, &text) else {
            continue;
        };
        match provkit::provn::to_string(&document) {
            Ok(s) if s.starts_with("document\n") && s.ends_with("endDocument\n") => {}
            Ok(_) => report.fail(&name, "output is not wrapped in document/endDocument"),
            Err(e) => report.fail(&name, format!("write: {e}")),
        }
    }
    report.finish(JSON_CORPUS_SIZE);
}

fn reason<'a>(list: &'a [(String, String)], name: &str) -> Option<&'a str> {
    list.iter()
        .find(|(n, _)| n == name)
        .map(|(_, r)| r.as_str())
}

/// The PROV-JSON twin of a PROV-N corpus file, as a per-file result.
fn json_twin(name: &str) -> Result<Document, String> {
    let path = corpus_dir("json").join(name.replace(".provn", ".json"));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    provkit::json::read(&text).map_err(|e| format!("JSON twin: {e}"))
}

#[test]
fn spec_examples_parse_under_strict() {
    let mut report = Report::default();
    let excluded_examples = excluded_spec_examples();
    let mut excluded = 0;
    for sub in ["provn/spec/prov-n", "provn/spec/prov-dm"] {
        for (name, text) in files(&corpus_dir(sub), "provn") {
            let result = provkit::provn::read_with(&text, Profile::Strict);
            if let Some(why) = reason(&excluded_examples, &name) {
                excluded += 1;
                if result.is_ok() {
                    report.fail(&name, format!("listed as excluded ({why}) but accepted"));
                }
                continue;
            }
            report.checked += 1;
            if let Err(e) = result {
                report.fail(&name, e);
            }
        }
    }
    if std::env::var("PROVKIT_ONLY").is_err() {
        assert_eq!(excluded, excluded_examples.len());
    }
    report.finish(127 - excluded_examples.len());
}

#[test]
fn provtoolbox_files_parse_under_default() {
    let mut report = Report::default();
    let excluded_documents = excluded_provtoolbox_documents();
    for (name, text) in files(&corpus_dir("provn/provtoolbox"), "provn") {
        let result = provkit::provn::read_with(&text, Profile::Default);
        if let Some(why) = reason(&excluded_documents, &name) {
            if result.is_ok() {
                report.fail(&name, format!("listed as excluded ({why}) but accepted"));
            }
            continue;
        }
        report.checked += 1;
        match result {
            Ok(d) if d.records.is_empty() && d.bundles.is_empty() => {
                report.fail(&name, "read as an empty document");
            }
            Ok(_) => {}
            Err(e) => report.fail(&name, e),
        }
    }
    report.finish(9 - excluded_documents.len());
}

#[test]
fn provtoolbox_corpus_equals_json_twins() {
    let mut report = Report::default();
    let known_differences = known_differences();
    let mut known = Vec::new();
    for (name, text) in files(&corpus_dir("provn/provtoolbox-corpus"), "provn") {
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
    if std::env::var("PROVKIT_ONLY").is_err() {
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
        let Some(document) = report.read_json(&name, &text) else {
            continue;
        };
        let provn = match provkit::provn::to_string(&document) {
            Ok(s) => s,
            Err(e) => {
                report.fail(&name, format!("write: {e}"));
                continue;
            }
        };
        match provkit::provn::read_with(&provn, Profile::Default) {
            Ok(back) if back == document => {}
            Ok(_) => report.fail(&name, "differs after the PROV-N round trip"),
            Err(e) => report.fail(&name, e),
        }
    }
    report.finish(JSON_CORPUS_SIZE);
}

/// The extensibility expressions the reader leaves out of corpus documents.
#[test]
fn corpus_extensibility_expressions_are_reported() {
    for example in ["prov-n-example-63.provn", "prov-n-example-64.provn"] {
        let text = std::fs::read_to_string(corpus_dir("provn/spec/prov-n").join(example)).unwrap();
        let provkit::provn::ReadReport { ignored, .. } =
            provkit::provn::read_report(&text, Profile::Strict).unwrap();
        let names: Vec<&str> = ignored.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["dictExt:hadMembers"], "{example}");
    }

    let text =
        std::fs::read_to_string(corpus_dir("provn/provtoolbox").join("prov-family.provn")).unwrap();
    let provkit::provn::ReadReport { ignored, .. } =
        provkit::provn::read_report(&text, Profile::Default).unwrap();
    assert_eq!(ignored.len(), 12);
    assert!(
        ignored
            .iter()
            .all(|i| i.name == "prov:derivedByInsertionFrom")
    );
}
