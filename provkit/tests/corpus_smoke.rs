//! Smoke tests for corpus integration.

mod common;

#[test]
fn corpus_has_398_json_documents() {
    let files = common::files(&common::corpus_dir("json"), "json");
    assert_eq!(files.len(), 398);
}
