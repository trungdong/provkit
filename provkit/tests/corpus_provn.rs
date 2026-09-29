//! Writes every PROV-JSON corpus document as PROV-N.

#![cfg(all(feature = "json", feature = "provn"))]

mod common;

use common::{Report, corpus_dir, files};

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
