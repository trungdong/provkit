//! Reads every PROV-JSON corpus document.

#![cfg(feature = "json")]

mod common;

use common::{Report, corpus_dir, files};

#[test]
fn reads_all_398() {
    let mut report = Report::default();
    for (name, text) in files(&corpus_dir("json"), "json") {
        report.checked += 1;
        if let Err(e) = provkit::json::read(&text) {
            report.fail(&name, e);
        }
    }
    report.finish(398);
}

#[test]
fn round_trips_all_398() {
    let mut report = Report::default();
    for (name, text) in files(&corpus_dir("json"), "json") {
        report.checked += 1;
        let first = match provkit::json::read(&text) {
            Ok(d) => d,
            Err(e) => {
                report.fail(&name, format!("read: {e}"));
                continue;
            }
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
    report.finish(398);
}
