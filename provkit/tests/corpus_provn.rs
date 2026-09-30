//! PROV-N corpus: the Recommendation examples, `ProvToolbox`'s documents and
//! writer output, and a PROV-N round trip of the PROV-JSON corpus.

mod common;

use common::{Report, corpus_dir, files};
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
        "'%% <http://example.org/type>' uses a bare IRI as a datatype; production [53] requires datatype to be a qualifiedName",
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

/// `ProvPy` 3.2.1's `KNOWN_DIFFERENCES`: twins not asserted equal.
const KNOWN_DIFFERENCES: &[(&str, Difference)] = &[
    ("activity3.provn", Difference::DynamicTimestamp),
    ("activity5.provn", Difference::DynamicTimestamp),
    ("activity6.provn", Difference::DynamicTimestamp),
    ("activity7.provn", Difference::DynamicTimestamp),
    ("activity8.provn", Difference::DynamicTimestamp),
    ("activity9.provn", Difference::DynamicTimestamp),
    ("agent6.provn", Difference::DynamicTimestamp),
    ("agent7.provn", Difference::DynamicTimestamp),
    ("agent8.provn", Difference::DynamicTimestamp),
    ("association7.provn", Difference::DynamicTimestamp),
    ("association9.provn", Difference::DynamicTimestamp),
    (
        "attr_association_one_role_attr27.provn",
        Difference::DynamicTimestamp,
    ),
    (
        "attr_association_one_role_attr32.provn",
        Difference::DynamicTimestamp,
    ),
    ("attr_entity_one_attr27.provn", Difference::DynamicTimestamp),
    ("attr_entity_one_attr32.provn", Difference::DynamicTimestamp),
    (
        "attr_entity_one_location_attr27.provn",
        Difference::DynamicTimestamp,
    ),
    (
        "attr_entity_one_location_attr32.provn",
        Difference::DynamicTimestamp,
    ),
    (
        "attr_entity_one_other_attr27.provn",
        Difference::DynamicTimestamp,
    ),
    (
        "attr_entity_one_other_attr32.provn",
        Difference::DynamicTimestamp,
    ),
    (
        "attr_entity_one_value_attr27.provn",
        Difference::DynamicTimestamp,
    ),
    (
        "attr_entity_one_value_attr32.provn",
        Difference::DynamicTimestamp,
    ),
    ("attribution7.provn", Difference::DynamicTimestamp),
    ("attribution8.provn", Difference::DynamicTimestamp),
    ("communication6.provn", Difference::DynamicTimestamp),
    ("communication7.provn", Difference::DynamicTimestamp),
    ("delegation7.provn", Difference::DynamicTimestamp),
    ("delegation8.provn", Difference::DynamicTimestamp),
    ("derivation8.provn", Difference::DynamicTimestamp),
    ("derivation9.provn", Difference::DynamicTimestamp),
    ("end10.provn", Difference::DynamicTimestamp),
    ("end7.provn", Difference::DynamicTimestamp),
    ("end8.provn", Difference::DynamicTimestamp),
    ("entity10.provn", Difference::DynamicTimestamp),
    ("entity5.provn", Difference::DynamicTimestamp),
    ("entity6.provn", Difference::DynamicTimestamp),
    ("entity7.provn", Difference::DynamicTimestamp),
    ("entity8.provn", Difference::DynamicTimestamp),
    ("entity9.provn", Difference::DynamicTimestamp),
    ("generation4.provn", Difference::DynamicTimestamp),
    ("generation5.provn", Difference::DynamicTimestamp),
    ("generation7.provn", Difference::DynamicTimestamp),
    ("influence6.provn", Difference::DynamicTimestamp),
    ("influence7.provn", Difference::DynamicTimestamp),
    ("invalidation4.provn", Difference::DynamicTimestamp),
    ("invalidation5.provn", Difference::DynamicTimestamp),
    ("invalidation7.provn", Difference::DynamicTimestamp),
    ("scruffy-end1-M.provn", Difference::DynamicTimestamp),
    ("scruffy-end1-S.provn", Difference::DynamicTimestamp),
    ("scruffy-end2-M.provn", Difference::DynamicTimestamp),
    ("scruffy-end2-S.provn", Difference::DynamicTimestamp),
    ("scruffy-end3-M.provn", Difference::DynamicTimestamp),
    ("scruffy-end3-S.provn", Difference::DynamicTimestamp),
    ("scruffy-end4-M.provn", Difference::DynamicTimestamp),
    ("scruffy-end4-S.provn", Difference::DynamicTimestamp),
    ("scruffy-generation1-M.provn", Difference::DynamicTimestamp),
    ("scruffy-generation1-S.provn", Difference::DynamicTimestamp),
    ("scruffy-generation2-M.provn", Difference::DynamicTimestamp),
    ("scruffy-generation2-S.provn", Difference::DynamicTimestamp),
    (
        "scruffy-invalidation1-M.provn",
        Difference::DynamicTimestamp,
    ),
    (
        "scruffy-invalidation1-S.provn",
        Difference::DynamicTimestamp,
    ),
    (
        "scruffy-invalidation2-M.provn",
        Difference::DynamicTimestamp,
    ),
    (
        "scruffy-invalidation2-S.provn",
        Difference::DynamicTimestamp,
    ),
    ("scruffy-start1-M.provn", Difference::DynamicTimestamp),
    ("scruffy-start1-S.provn", Difference::DynamicTimestamp),
    ("scruffy-start2-M.provn", Difference::DynamicTimestamp),
    ("scruffy-start2-S.provn", Difference::DynamicTimestamp),
    ("scruffy-start3-M.provn", Difference::DynamicTimestamp),
    ("scruffy-start3-S.provn", Difference::DynamicTimestamp),
    ("scruffy-start4-M.provn", Difference::DynamicTimestamp),
    ("scruffy-start4-S.provn", Difference::DynamicTimestamp),
    ("scruffy-usage1-M.provn", Difference::DynamicTimestamp),
    ("scruffy-usage1-S.provn", Difference::DynamicTimestamp),
    ("scruffy-usage2-M.provn", Difference::DynamicTimestamp),
    ("scruffy-usage2-S.provn", Difference::DynamicTimestamp),
    ("start10.provn", Difference::DynamicTimestamp),
    ("start7.provn", Difference::DynamicTimestamp),
    ("start8.provn", Difference::DynamicTimestamp),
    ("usage4.provn", Difference::DynamicTimestamp),
    ("usage5.provn", Difference::DynamicTimestamp),
    ("usage7.provn", Difference::DynamicTimestamp),
    ("attr_activity0.provn", Difference::DatatypePalette),
    ("attr_agent0.provn", Difference::DatatypePalette),
    ("attr_association0.provn", Difference::DatatypePalette),
    (
        "attr_association_one_role_attr29.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_association_one_role_attr30.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_association_one_role_attr31.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_association_one_role_attr33.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_association_one_role_attr34.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_association_one_role_attr35.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_association_one_role_attr36.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_association_one_role_attr37.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_association_one_role_attr38.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_association_one_role_attr39.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_association_one_role_attr40.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_association_one_role_attr41.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_association_one_role_attr42.provn",
        Difference::DatatypePalette,
    ),
    ("attr_attribution0.provn", Difference::DatatypePalette),
    ("attr_communication0.provn", Difference::DatatypePalette),
    ("attr_delegation0.provn", Difference::DatatypePalette),
    ("attr_derivation0.provn", Difference::DatatypePalette),
    ("attr_end0.provn", Difference::DatatypePalette),
    ("attr_entity0.provn", Difference::DatatypePalette),
    ("attr_entity_one_attr29.provn", Difference::DatatypePalette),
    ("attr_entity_one_attr30.provn", Difference::DatatypePalette),
    ("attr_entity_one_attr31.provn", Difference::DatatypePalette),
    ("attr_entity_one_attr33.provn", Difference::DatatypePalette),
    ("attr_entity_one_attr34.provn", Difference::DatatypePalette),
    ("attr_entity_one_attr35.provn", Difference::DatatypePalette),
    ("attr_entity_one_attr36.provn", Difference::DatatypePalette),
    ("attr_entity_one_attr37.provn", Difference::DatatypePalette),
    ("attr_entity_one_attr38.provn", Difference::DatatypePalette),
    ("attr_entity_one_attr39.provn", Difference::DatatypePalette),
    ("attr_entity_one_attr40.provn", Difference::DatatypePalette),
    ("attr_entity_one_attr41.provn", Difference::DatatypePalette),
    ("attr_entity_one_attr42.provn", Difference::DatatypePalette),
    (
        "attr_entity_one_location_attr29.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_location_attr30.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_location_attr31.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_location_attr33.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_location_attr34.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_location_attr35.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_location_attr36.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_location_attr37.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_location_attr38.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_location_attr39.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_location_attr40.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_location_attr41.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_location_attr42.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_other_attr29.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_other_attr30.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_other_attr31.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_other_attr33.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_other_attr34.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_other_attr35.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_other_attr36.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_other_attr37.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_other_attr38.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_other_attr39.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_other_attr40.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_other_attr41.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_other_attr42.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_value_attr29.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_value_attr30.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_value_attr31.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_value_attr33.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_value_attr34.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_value_attr35.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_value_attr36.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_value_attr37.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_value_attr38.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_value_attr39.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_value_attr40.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_value_attr41.provn",
        Difference::DatatypePalette,
    ),
    (
        "attr_entity_one_value_attr42.provn",
        Difference::DatatypePalette,
    ),
    ("attr_generation0.provn", Difference::DatatypePalette),
    ("attr_influence0.provn", Difference::DatatypePalette),
    ("attr_invalidation0.provn", Difference::DatatypePalette),
    ("attr_start0.provn", Difference::DatatypePalette),
    ("attr_usage0.provn", Difference::DatatypePalette),
];

#[test]
fn writes_all_398() {
    let mut report = Report::default();
    for (name, text) in files(&corpus_dir("json"), "json") {
        report.checked += 1;
        let document = match provkit::json::read(&text) {
            Ok(d) => d,
            Err(e) => {
                report.fail(&name, format!("read: {e}"));
                continue;
            }
        };
        match provkit::provn::to_string(&document) {
            Ok(s) if s.starts_with("document\n") && s.ends_with("endDocument\n") => {}
            Ok(_) => report.fail(&name, "output is not wrapped in document/endDocument"),
            Err(e) => report.fail(&name, format!("write: {e}")),
        }
    }
    report.finish(398);
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
    let mut known = Vec::new();
    let mut seen_known = 0;
    for (name, text) in files(&corpus_dir("provn/provtoolbox-corpus"), "provn") {
        let outcome = json_twin(&name).and_then(|expected| {
            provkit::provn::read_with(&text, Profile::Default)
                .map(|d| d == expected)
                .map_err(|e| e.to_string())
        });
        report.checked += 1;
        let known_as = KNOWN_DIFFERENCES.iter().find(|(n, _)| *n == name);
        seen_known += usize::from(known_as.is_some());
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
        assert_eq!(seen_known, KNOWN_DIFFERENCES.len());
    }
    report.finish(388);
}

#[test]
fn json_corpus_round_trips_through_provn() {
    let mut report = Report::default();
    for (name, text) in files(&corpus_dir("json"), "json") {
        report.checked += 1;
        let document = match provkit::json::read(&text) {
            Ok(d) => d,
            Err(e) => {
                report.fail(&name, format!("read: {e}"));
                continue;
            }
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
    report.finish(398);
}
