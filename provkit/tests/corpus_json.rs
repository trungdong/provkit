//! Reads every PROV-JSON corpus document.

mod common;

use common::{JSON_CORPUS_SIZE, Report, corpus_dir, files};
use provkit::FormalAttr;

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
        let Some(first) = report.read_json(&name, &text) else {
            continue;
        };
        let written = match provkit::json::to_string(&first) {
            Ok(s) => s,
            Err(e) => {
                report.fail(&name, format!("write: {e}"));
                continue;
            }
        };
        match provkit::json::read(&written) {
            Ok(second) if second == first => {}
            Ok(_) => report.fail(&name, "documents differ after round trip"),
            Err(e) => report.fail(&name, format!("re-read: {e}")),
        }
    }
    report.finish(JSON_CORPUS_SIZE);
}

/// The time formals a unification conflict may name.
const TIME_FORMALS: [&str; 3] = ["prov:time", "prov:startTime", "prov:endTime"];

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
/// identifier that disagree on a formal. Each fails to unify. The reason names
/// `prov:time` in 20 of them (generation, usage, invalidation and the first
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
            Err(provkit::Error::Unification { reason, .. }) if scruffy => {
                let formal = reason.split(' ').next().unwrap_or_default();
                let local = formal.strip_prefix("prov:").unwrap_or_default();
                if TIME_FORMALS.contains(&formal) {
                    on_time.push(name.clone());
                } else if FormalAttr::from_local_name(local).is_some_and(|a| !a.is_time()) {
                    on_name.push(name.clone());
                } else {
                    report.fail(&name, format!("unexpected conflict: {reason}"));
                }
            }
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
    if std::env::var("PROVKIT_ONLY").is_err() {
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
