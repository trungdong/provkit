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
