//! Reads every PROV-JSON corpus document.

mod common;

use common::{JSON_CORPUS_SIZE, Report, corpus_dir, files, only};

#[test]
fn reads_all_398() {
    let mut report = Report::default();
    for (name, text) in files(&corpus_dir("json"), "json") {
        report.checked += 1;
        report.read_json(&name, &text);
    }
    report.finish(JSON_CORPUS_SIZE);
}

#[test]
fn round_trips_all_398() {
    let mut report = Report::default();
    for (name, text) in files(&corpus_dir("json"), "json") {
        report.checked += 1;
        if let Some(first) = report.read_json(&name, &text) {
            report.round_trip(&name, &first, provkit::json::to_string, provkit::json::read);
        }
    }
    report.finish(JSON_CORPUS_SIZE);
}

/// The eight `scruffy-*` documents whose conflict is on a name, not a time.
const NAME_CONFLICTS: [&str; 8] = [
    "scruffy-end3-M.json",
    "scruffy-end3-S.json",
    "scruffy-end4-M.json",
    "scruffy-end4-S.json",
    "scruffy-start3-M.json",
    "scruffy-start3-S.json",
    "scruffy-start4-M.json",
    "scruffy-start4-S.json",
];

/// The 28 `scruffy-*` documents (14 pairs) each hold two records under one
/// identifier that disagree on a formal. Each fails to unify. The cause names
/// a time formal in 20 of them (generation, usage, invalidation and the first
/// two start and end cases) and a `prov:` name attribute in the other 8.
#[test]
fn unification_over_the_corpus() {
    let mut report = Report::default();
    let (mut on_time, mut on_name) = (Vec::new(), Vec::new());
    for (name, text) in files(&corpus_dir("json"), "json") {
        report.checked += 1;
        let Some(d) = report.read_json(&name, &text) else {
            continue;
        };
        let scruffy = name.starts_with("scruffy-");
        match d.unified() {
            Err(provkit::Error::Unification { cause, .. }) if scruffy => match cause {
                provkit::UnificationCause::Formal { attr, .. } if attr.is_time() => {
                    on_time.push(name.clone());
                }
                provkit::UnificationCause::Formal { .. } => on_name.push(name.clone()),
                other => report.fail(&name, format!("unexpected conflict: {other}")),
            },
            Err(e) if scruffy => {
                report.fail(&name, format!("expected Error::Unification, got: {e}"));
            }
            Err(e) => report.fail(&name, format!("unexpected unification error: {e}")),
            Ok(_) if scruffy => report.fail(&name, "expected a unification conflict"),
            Ok(u) => {
                match u.unified() {
                    Ok(again) if again == u => {}
                    Ok(_) => report.fail(&name, "unified() is not idempotent"),
                    Err(e) => report.fail(&name, format!("second unified(): {e}")),
                }
                if u.all_records().count() > d.all_records().count() {
                    report.fail(&name, "unified() grew the document");
                }
            }
        }
    }
    // The filter narrows the set, so the lists are checked only without it.
    if only().is_none() {
        if on_name != NAME_CONFLICTS {
            report.fail(
                "scruffy-*",
                format!("conflicts on a name: {on_name:?}, expected {NAME_CONFLICTS:?}"),
            );
        }
        if on_time.len() != 20 {
            report.fail(
                "scruffy-*",
                format!("{} conflicts on a time, expected 20", on_time.len()),
            );
        }
    }
    report.finish(JSON_CORPUS_SIZE);
}

/// Number of PROV-JSON documents in `corpus/constraints/`.
const CONSTRAINTS_CORPUS_SIZE: usize = 171;

/// The six files on which the `ProvToolbox` validator expects a failed merge:
/// an association with and without a plan, a derivation with and without
/// an activity, under one identifier.
const NONE_POSITION_FAILURES: [&str; 6] = [
    "association-fail4.json",
    "association-fail5.json",
    "derivation-fail1.json",
    "derivation-fail2.json",
    "derivation-fail3.json",
    "derivation-fail4.json",
];

/// Every other constraints file that fails `unified()`, with its cause: the
/// formal two present values disagree on, or `kinds` for a clash of kinds
/// under one identifier. All other constraints files unify.
const OTHER_FAILURES: [(&str, &str); 33] = [
    ("association-fail1.json", "prov:agent"),
    ("association-fail2.json", "prov:activity"),
    ("association-fail3.json", "prov:plan"),
    ("delegation-fail1.json", "prov:responsible"),
    ("delegation-fail2.json", "prov:delegate"),
    ("delegation-fail3.json", "prov:activity"),
    ("end-fail1.json", "prov:activity"),
    ("end-fail2.json", "prov:trigger"),
    ("end-fail3.json", "prov:ender"),
    ("end-fail5.json", "prov:time"),
    ("generation-fail2.json", "prov:entity"),
    ("generation-fail3.json", "prov:activity"),
    ("generation-fail4.json", "prov:time"),
    ("invalidation-fail2.json", "prov:entity"),
    ("invalidation-fail3.json", "prov:activity"),
    ("invalidation-fail4.json", "prov:time"),
    ("start-fail1.json", "prov:activity"),
    ("start-fail2.json", "prov:trigger"),
    ("start-fail3.json", "prov:starter"),
    ("start-fail5.json", "prov:time"),
    ("start-fail6.json", "prov:time"),
    ("start-fail8.json", "prov:time"),
    ("type-f1-FAIL-c50-c55.json", "kinds"),
    ("type-f3-FAIL-c54.json", "kinds"),
    ("type-f4-FAIL-c53.json", "kinds"),
    ("unification-activity-f1-FAIL-c22.json", "prov:startTime"),
    ("unification-entity-activity-f1-FAIL-c55.json", "kinds"),
    ("unification-generation-f1-FAIL-c23.json", "prov:entity"),
    ("unification-object-relation-f1-FAIL-c54.json", "kinds"),
    ("unification-relation-kind-f1-FAIL-c53.json", "kinds"),
    ("usage-fail2.json", "prov:activity"),
    ("usage-fail3.json", "prov:entity"),
    ("usage-fail4.json", "prov:time"),
];

#[test]
fn constraints_corpus_fails_to_unify_where_a_none_position_differs() {
    let mut report = Report::default();
    let mut none_failures = Vec::new();
    let mut others = Vec::new();
    for (name, text) in files(&corpus_dir("constraints"), "json") {
        report.checked += 1;
        let Some(d) = report.read_json(&name, &text) else {
            continue;
        };
        match d.unified() {
            Ok(_) => {}
            Err(provkit::Error::Unification { cause, .. }) => match cause {
                provkit::UnificationCause::Formal {
                    attr,
                    first,
                    second,
                    ..
                } if first.is_none() != second.is_none() => {
                    assert!(
                        matches!(
                            attr,
                            provkit::FormalAttr::Plan | provkit::FormalAttr::Activity
                        ),
                        "{name}: {attr}"
                    );
                    none_failures.push(name);
                }
                provkit::UnificationCause::Formal { attr, .. } => {
                    others.push((name, attr.to_string()));
                }
                provkit::UnificationCause::Kinds { .. } => others.push((name, "kinds".to_owned())),
                other => report.fail(&name, format!("unexpected conflict: {other}")),
            },
            Err(e) => report.fail(&name, format!("expected Error::Unification, got: {e}")),
        }
    }
    // The filter narrows the set, so the lists are checked only without it.
    if only().is_none() {
        if none_failures != NONE_POSITION_FAILURES {
            report.fail(
                "constraints",
                format!(
                    "none-position failures {none_failures:?}, expected {NONE_POSITION_FAILURES:?}"
                ),
            );
        }
        let expected: Vec<(String, String)> = OTHER_FAILURES
            .iter()
            .map(|&(n, c)| (n.to_owned(), c.to_owned()))
            .collect();
        if others != expected {
            report.fail(
                "constraints",
                format!("other failures {others:?}, expected {expected:?}"),
            );
        }
    }
    report.finish(CONSTRAINTS_CORPUS_SIZE);
}

/// The text of the first Rust code block in `markdown`.
fn first_rust_block(markdown: &str) -> &str {
    let (_, after) = markdown.split_once("```rust\n").expect("a Rust code block");
    after.split_once("```").expect("a closing fence").0
}

#[test]
fn both_readmes_carry_the_same_example() {
    // The root README is not in the packaged crate, so this test lives in a
    // file the package leaves out.
    let root = include_str!("../../README.md");
    let crate_readme = include_str!("../README.md");
    assert_eq!(first_rust_block(root), first_rust_block(crate_readme));
}
