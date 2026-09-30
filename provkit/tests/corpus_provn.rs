//! PROV-N corpus: the Recommendation examples, `ProvToolbox`'s documents and
//! writer output, and a PROV-N round trip of the PROV-JSON corpus.

mod common;

use common::{JSON_CORPUS_SIZE, Report, corpus_dir, files};
use provkit::Document;
use provkit::provn::Profile;

/// Recommendation examples that the formal grammar rejects or that are not
/// complete PROV-N. Names and reasons follow `corpus/provn/README.md`.
const EXCLUDED_SPEC_EXAMPLES: &[(&str, &str)] = &[
    (
        "prov-n-example-16.provn",
        "the sixth wasGeneratedBy variant reuses tr:WD-prov-dm-20111215 as its time argument, which is not a dateTime; a copy-paste artefact in the Recommendation's own text",
    ),
    (
        "prov-n-example-37.provn",
        "wasAssociatedWith(ex:a1, ex:ag1) gives the agent without the paired plan; production [20] requires them together",
    ),
    (
        "prov-n-example-51.provn",
        "a default declaration follows a prefix declaration; production [45] puts the default before every prefix",
    ),
    (
        "prov-n-example-52.provn",
        "a bare typed-literal pair, not a statement",
    ),
    (
        "prov-n-example-53.provn",
        "a bare typed-literal pair, not a statement",
    ),
    (
        "prov-n-example-54.provn",
        "a bare qualified-name literal pair, not a statement",
    ),
    (
        "prov-n-example-55.provn",
        "a list of bare literals, not a statement",
    ),
    (
        "prov-n-example-56.provn",
        "a list of bare literals, not a statement",
    ),
    (
        "prov-n-example-59.provn",
        "a literal ellipsis elides the rest of the example, not complete PROV-N",
    ),
    (
        "prov-n-example-61.provn",
        "the bundle identifier 'b' has no declared default namespace; the example illustrates qualified-name resolution for ex:e001, not the bundle name itself",
    ),
    (
        "prov-dm-example-03.provn",
        "used and wasGeneratedBy give only the entity/activity without the paired time; the formal grammar requires them together",
    ),
    (
        "prov-dm-example-04.provn",
        "used and wasGeneratedBy give only the entity/activity without the paired time",
    ),
    (
        "prov-dm-example-05.provn",
        "a literal ellipsis elides the bundle's content, not complete PROV-N",
    ),
    (
        "prov-dm-example-06.provn",
        "a literal ellipsis elides the bundle's content, not complete PROV-N",
    ),
    (
        "prov-dm-example-16.provn",
        "wasGeneratedBy gives only the entity without the paired time",
    ),
    (
        "prov-dm-example-19.provn",
        "a spec typo leaves the agent's attribute list missing its closing ']'",
    ),
    (
        "prov-dm-example-24.provn",
        "wasAssociatedWith and wasGeneratedBy give only the activity/entity without the paired plan or time",
    ),
    (
        "prov-dm-example-34.provn",
        "wasAssociatedWith gives only the activity and agent without the paired plan",
    ),
    (
        "prov-dm-example-42.provn",
        "a statement follows an endBundle at document level; production [1] puts every expression before the bundles",
    ),
    (
        "prov-dm-example-52.provn",
        "wasAssociatedWith gives only the activity and agent without the paired plan",
    ),
    (
        "prov-dm-example-53.provn",
        "used gives only the activity and entity without the paired time",
    ),
    (
        "prov-dm-example-55.provn",
        "used and wasGeneratedBy give only the activity/entity without the paired time",
    ),
    (
        "prov-dm-example-56.provn",
        "used gives only the activity and entity without the paired time",
    ),
    (
        "prov-dm-example-57.provn",
        "a list of bare literals, not a statement",
    ),
    (
        "prov-dm-example-58.provn",
        "a bare typed-literal pair, not a statement",
    ),
    (
        "prov-dm-example-59.provn",
        "a bare qualified-name literal, not a statement",
    ),
    (
        "prov-dm-example-63.provn",
        "used gives only the activity and entity without the paired time",
    ),
];

/// `ProvToolbox`'s own documents outside what the default profile reads.
/// Names and reasons follow `corpus/provn/README.md`.
const EXCLUDED_PROVTOOLBOX_DOCUMENTS: &[(&str, &str)] = &[
    (
        "container0.provn",
        "several entity identifiers are bare local names ('\\--', '\\-', '\\.') with no default namespace declared anywhere in the document",
    ),
    (
        "container1.provn",
        "'\"hello2\"@EN %% xsd:string' gives one string both a language tag and a datatype; productions [41] and [43] allow one or the other",
    ),
    (
        "container2.provn",
        "'%% <http://example.org/type>' uses a bare IRI as a datatype; production [42] requires datatype to be a qualifiedName",
    ),
];

/// Why `ProvToolbox`'s writer output differs from its JSON twin
/// (`corpus/provn/README.md`, "Known differences").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Difference {
    /// The generator embeds the wall-clock time of the corpus build.
    DynamicTimestamp,
    /// The generator's datatype palette no longer lines up with the fixture.
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

fn reason<'a>(list: &[(&str, &'a str)], name: &str) -> Option<&'a str> {
    list.iter().find(|(n, _)| *n == name).map(|(_, r)| *r)
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
    let mut excluded = 0;
    for sub in ["provn/spec/prov-n", "provn/spec/prov-dm"] {
        for (name, text) in files(&corpus_dir(sub), "provn") {
            let result = provkit::provn::read_with(&text, Profile::Strict);
            if let Some(why) = reason(EXCLUDED_SPEC_EXAMPLES, &name) {
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
        assert_eq!(excluded, EXCLUDED_SPEC_EXAMPLES.len());
    }
    report.finish(127 - EXCLUDED_SPEC_EXAMPLES.len());
}

#[test]
fn provtoolbox_files_parse_under_default() {
    let mut report = Report::default();
    for (name, text) in files(&corpus_dir("provn/provtoolbox"), "provn") {
        let result = provkit::provn::read_with(&text, Profile::Default);
        if let Some(why) = reason(EXCLUDED_PROVTOOLBOX_DOCUMENTS, &name) {
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
    report.finish(9 - EXCLUDED_PROVTOOLBOX_DOCUMENTS.len());
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
        let (_, ignored) = provkit::provn::read_report(&text, Profile::Strict).unwrap();
        let names: Vec<&str> = ignored.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["dictExt:hadMembers"], "{example}");
    }

    let text =
        std::fs::read_to_string(corpus_dir("provn/provtoolbox").join("prov-family.provn")).unwrap();
    let (_, ignored) = provkit::provn::read_report(&text, Profile::Default).unwrap();
    assert_eq!(ignored.len(), 12);
    assert!(
        ignored
            .iter()
            .all(|i| i.name == "prov:derivedByInsertionFrom")
    );
}
