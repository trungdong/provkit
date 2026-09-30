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

/// The 28 `scruffy-*` documents (14 pairs) each hold two records under one
/// identifier that disagree on a formal. Each fails to unify. The reason names
/// `prov:time` in 20 of them (generation, usage, invalidation and the first
/// two start and end cases) and a `prov:` name attribute in the other 8.
#[test]
fn unification_over_the_corpus() {
    let mut report = Report::default();
    let (mut on_time, mut on_name) = (0, 0);
    for (name, text) in files(&corpus_dir("json"), "json") {
        report.checked += 1;
        let Some(d) = report.read_json(&name, &text) else {
            continue;
        };
        let scruffy = name.starts_with("scruffy-");
        match d.unified() {
            Err(provkit::Error::Unification { reason, .. }) if scruffy => {
                let formal = reason
                    .strip_prefix("prov:")
                    .and_then(|rest| rest.split(' ').next());
                match formal {
                    Some("time") => on_time += 1,
                    Some(f) if FormalAttr::from_local_name(f).is_some_and(|a| !a.is_time()) => {
                        on_name += 1;
                    }
                    _ => report.fail(&name, format!("unexpected conflict: {reason}")),
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
    if (on_time, on_name) != (20, 8) {
        report.fail(
            "scruffy-*",
            format!("{on_time} conflicts on a time and {on_name} on a name, expected 20 and 8"),
        );
    }
    report.finish(JSON_CORPUS_SIZE);
}
