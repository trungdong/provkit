//! PROV-N corpus: the Recommendation examples, `ProvToolbox`'s documents and
//! the extensibility expressions the reader leaves out.

mod common;

use common::{Report, corpus_dir, files, files_unchecked, require_a_match};
use provkit::Error;
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

fn reason<'a>(list: &'a [(String, String)], name: &str) -> Option<&'a str> {
    list.iter()
        .find(|(n, _)| n == name)
        .map(|(_, r)| r.as_str())
}

/// Whether `error` is the kind the exclusion of `name` gives as its reason.
/// Example 61 uses a bundle name with no default namespace. Every other
/// exclusion is a grammar rejection.
fn fails_as_listed(name: &str, error: &Error) -> bool {
    match name {
        "prov-n-example-61.provn" => matches!(
            error,
            Error::At { error, .. } if matches!(**error, Error::NoDefaultNamespace(_))
        ),
        _ => matches!(error, Error::Syntax { .. }),
    }
}

#[test]
fn spec_examples_parse_under_strict() {
    let mut report = Report::default();
    let excluded_examples = excluded_spec_examples();
    let mut excluded = 0;
    let mut any_selected = false;
    for sub in ["provn/spec/prov-n", "provn/spec/prov-dm"] {
        for (name, text) in files_unchecked(&corpus_dir(sub), "provn") {
            any_selected = true;
            let result = provkit::provn::read_report(&text, Profile::Strict);
            if let Some(why) = reason(&excluded_examples, &name) {
                excluded += 1;
                match result {
                    Ok(_) => {
                        report.fail(&name, format!("listed as excluded ({why}) but accepted"));
                    }
                    Err(e) if !fails_as_listed(&name, &e) => {
                        report.fail(&name, format!("excluded as {why}, but failed with: {e}"));
                    }
                    Err(_) => {}
                }
                continue;
            }
            report.checked += 1;
            match result {
                Ok(r)
                    if r.document.records.is_empty()
                        && r.document.bundles.is_empty()
                        && r.ignored.is_empty() =>
                {
                    report.fail(&name, "read as an empty document");
                }
                Ok(_) => {}
                Err(e) => report.fail(&name, e),
            }
        }
    }
    require_a_match(any_selected);
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

/// The extensibility expressions the reader leaves out of corpus documents.
#[test]
fn corpus_extensibility_expressions_are_reported() {
    let mut any_selected = false;
    for example in ["prov-n-example-63.provn", "prov-n-example-64.provn"] {
        if !common::selected(example) {
            continue;
        }
        any_selected = true;
        let text = std::fs::read_to_string(corpus_dir("provn/spec/prov-n").join(example)).unwrap();
        let provkit::provn::ReadReport { ignored, .. } =
            provkit::provn::read_report(&text, Profile::Strict).unwrap();
        let names: Vec<&str> = ignored.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["dictExt:hadMembers"], "{example}");
    }

    if common::selected("prov-family.provn") {
        any_selected = true;
        let text =
            std::fs::read_to_string(corpus_dir("provn/provtoolbox").join("prov-family.provn"))
                .unwrap();
        let provkit::provn::ReadReport { ignored, .. } =
            provkit::provn::read_report(&text, Profile::Default).unwrap();
        assert_eq!(ignored.len(), 12);
        assert!(
            ignored
                .iter()
                .all(|i| i.name == "prov:derivedByInsertionFrom")
        );
    }
    require_a_match(any_selected);
}
