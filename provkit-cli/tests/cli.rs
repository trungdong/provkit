//! End-to-end tests for the `provkit` binary.

use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;

fn corpus(sub: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(sub)
}

fn provkit() -> Command {
    Command::cargo_bin("provkit").unwrap()
}

/// A fresh, empty directory named after the test, under the target directory.
fn scratch(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const PROVN: &str = "document\n  prefix ex <http://example.org/>\n  entity(ex:e1)\nendDocument\n";

#[test]
fn convert_json_to_provn_on_stdout_by_default() {
    provkit()
        .args(["convert"])
        .arg(corpus("json/entity1.json"))
        .assert()
        .success()
        .stdout(predicate::str::starts_with("document\n"))
        .stdout(predicate::str::contains("entity(ex:e1"))
        .stderr("");
}

#[test]
fn convert_to_file_by_extension_and_back() {
    let dir = scratch("convert_to_file_by_extension_and_back");
    let provn = dir.join("out.provn");
    provkit()
        .args(["convert"])
        .arg(corpus("json/entity1.json"))
        .arg("-o")
        .arg(&provn)
        .assert()
        .success()
        .stdout("")
        .stderr("");
    assert!(
        std::fs::read_to_string(&provn)
            .unwrap()
            .starts_with("document")
    );
    let json = dir.join("out.json");
    provkit()
        .args(["convert"])
        .arg(&provn)
        .arg("-o")
        .arg(&json)
        .assert()
        .success();
    let text = std::fs::read_to_string(&json).unwrap();
    assert!(text.trim_start().starts_with('{'));
    let original = std::fs::read_to_string(corpus("json/entity1.json")).unwrap();
    assert_eq!(
        provkit::json::read(&text).unwrap(),
        provkit::json::read(&original).unwrap()
    );
}

#[test]
fn stdin_is_sniffed_and_stdout_needs_a_flag_for_json() {
    let input = std::fs::read_to_string(corpus("json/entity1.json")).unwrap();
    provkit()
        .args(["convert", "-t", "json", "-"])
        .write_stdin(input.clone())
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{"))
        .stderr("");
    provkit()
        .args(["convert", "-"])
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("document"));
}

#[test]
fn dash_output_means_stdout() {
    provkit()
        .args(["convert", "-o", "-"])
        .arg(corpus("json/entity1.json"))
        .assert()
        .success()
        .stdout(predicate::str::starts_with("document"));
}

#[test]
fn to_flag_overrides_output_extension() {
    let dir = scratch("to_flag_overrides_output_extension");
    let out = dir.join("out.json");
    provkit()
        .args(["convert", "-t", "provn"])
        .arg(corpus("json/entity1.json"))
        .arg("-o")
        .arg(&out)
        .assert()
        .success();
    assert!(
        std::fs::read_to_string(&out)
            .unwrap()
            .starts_with("document")
    );
}

#[test]
fn sniffing_skips_comments_and_blank_lines() {
    provkit()
        .args(["convert", "-t", "json", "-"])
        .write_stdin(format!("\n// lead\n/* block */\n{PROVN}"))
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{"));
}

#[test]
fn from_flag_overrides_extension() {
    let dir = scratch("from_flag_overrides_extension");
    let lying = dir.join("really-provn.json");
    std::fs::write(&lying, PROVN).unwrap();
    provkit()
        .args(["convert", "-f", "provn", "-t", "json"])
        .arg(&lying)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{"));
    provkit()
        .args(["convert"])
        .arg(&lying)
        .assert()
        .code(2)
        .stderr(predicate::str::starts_with("provkit: "))
        .stderr(predicate::str::contains("really-provn.json"));
}

#[test]
fn unreadable_input_exits_2() {
    provkit()
        .args(["convert", "/nonexistent/file.json"])
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::starts_with(
            "provkit: /nonexistent/file.json",
        ));
    let dir = scratch("unreadable_input_exits_2");
    let junk = dir.join("junk.txt");
    std::fs::write(&junk, "hello").unwrap();
    provkit()
        .args(["convert"])
        .arg(&junk)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("junk.txt"))
        .stderr(predicate::str::contains("cannot tell the format"));
}

#[test]
fn library_errors_name_the_file() {
    let dir = scratch("library_errors_name_the_file");
    let bad = dir.join("bad.provn");
    std::fs::write(&bad, "document\n  entity(ex:e1)\nendDocument\n").unwrap();
    let assert = provkit().args(["convert"]).arg(&bad).assert().code(2);
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    let prefix = format!("provkit: {}: ", bad.display());
    assert!(stderr.starts_with(&prefix), "stderr was {stderr:?}");
    assert!(stderr.len() > prefix.len() + 1);
}

#[test]
fn stdin_errors_name_the_input_as_dash() {
    provkit()
        .args(["convert", "-"])
        .write_stdin("hello")
        .assert()
        .code(2)
        .stderr(predicate::str::starts_with("provkit: -: "));
    provkit()
        .args(["convert", "-f", "json", "-"])
        .write_stdin("{")
        .assert()
        .code(2)
        .stderr(predicate::str::starts_with("provkit: -: "));
}

const PRECEDENCE: &str = "taken from -f/-t when given, else from the file extension";

#[test]
fn help_states_the_precedence_rule() {
    provkit()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(PRECEDENCE));
    provkit()
        .args(["convert", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(PRECEDENCE));
}

#[test]
fn unknown_extension_falls_through_to_sniffing() {
    let dir = scratch("unknown_extension_falls_through_to_sniffing");
    let txt = dir.join("x.txt");
    std::fs::write(&txt, PROVN).unwrap();
    provkit()
        .args(["convert", "-t", "json"])
        .arg(&txt)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{"))
        .stderr("");
    let dat = dir.join("x.dat");
    std::fs::copy(corpus("json/entity1.json"), &dat).unwrap();
    provkit()
        .args(["convert"])
        .arg(&dat)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("document"))
        .stderr("");
}

#[test]
fn unknown_format_values_are_rejected() {
    for flag in ["-f", "-t"] {
        provkit()
            .args(["convert", flag, "xml", "-"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("invalid value 'xml'"));
    }
}

#[test]
fn failed_write_names_the_output() {
    let dir = scratch("failed_write_names_the_output");
    let out = dir.join("no-such-dir").join("out.provn");
    provkit()
        .args(["convert"])
        .arg(corpus("json/entity1.json"))
        .arg("-o")
        .arg(&out)
        .assert()
        .code(2)
        .stderr(predicate::str::starts_with(format!(
            "provkit: {}: ",
            out.display()
        )));
}
