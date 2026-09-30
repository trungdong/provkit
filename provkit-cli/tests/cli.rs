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
fn extensions_match_in_any_case() {
    let dir = scratch("extensions_match_in_any_case");
    let out = dir.join("out.JSON");
    provkit()
        .args(["convert"])
        .arg(corpus("json/entity1.json"))
        .arg("-o")
        .arg(&out)
        .assert()
        .success();
    assert!(
        std::fs::read_to_string(&out)
            .unwrap()
            .trim_start()
            .starts_with('{')
    );
    let provn = dir.join("in.PROVN");
    std::fs::write(&provn, PROVN).unwrap();
    provkit()
        .args(["convert", "-t", "json"])
        .arg(&provn)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{"));
}

#[test]
fn a_leading_byte_order_mark_is_ignored() {
    let dir = scratch("a_leading_byte_order_mark_is_ignored");
    let json_text = std::fs::read_to_string(corpus("json/entity1.json")).unwrap();
    let cases = [
        ("bom.json", format!("\u{FEFF}{json_text}")),
        ("bom.provn", format!("\u{FEFF}{PROVN}")),
        ("bom.txt", format!("\u{FEFF}{PROVN}")),
    ];
    for (name, text) in cases {
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        provkit()
            .args(["convert"])
            .arg(&path)
            .assert()
            .success()
            .stdout(predicate::str::contains("entity(ex:e1"))
            .stderr("");
    }
    for text in [format!("\u{FEFF}{json_text}"), format!("\u{FEFF}{PROVN}")] {
        provkit()
            .args(["convert", "-"])
            .write_stdin(text)
            .assert()
            .success()
            .stdout(predicate::str::contains("entity(ex:e1"))
            .stderr("");
    }
    // Only the first character is a byte-order mark; a second one is content.
    provkit()
        .args(["convert", "-"])
        .write_stdin(format!("\u{FEFF}\u{FEFF}{PROVN}"))
        .assert()
        .code(2);
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
fn positioned_errors_print_their_message_once() {
    let dir = scratch("positioned_errors_print_their_message_once");
    let bad = dir.join("bad.provn");
    std::fs::write(&bad, "document\n  entity(ex:e1)\nendDocument\n").unwrap();
    let assert = provkit().args(["convert"]).arg(&bad).assert().code(2);
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("line 2, column 10: undeclared prefix `ex`"),
        "stderr was {stderr:?}"
    );
    assert_eq!(
        stderr.matches("undeclared prefix `ex`").count(),
        1,
        "stderr was {stderr:?}"
    );
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

#[test]
fn compare_equal_across_formats() {
    provkit()
        .args(["compare"])
        .arg(corpus("json/entity1.json"))
        .arg(corpus("provn/provtoolbox-corpus/entity1.provn"))
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn compare_reports_one_sided_records() {
    let m = corpus("json/scruffy-generation1-M.json");
    provkit()
        .args(["compare"])
        .arg(corpus("json/scruffy-generation1-S.json"))
        .arg(&m)
        .assert()
        .code(1)
        .stdout(format!(
            "only in {}:\n  entity(ex:e1)\n  activity(ex:a1)\n",
            m.display()
        ))
        .stderr("");
}

#[test]
fn compare_unified_reports_conflicts_as_errors() {
    provkit()
        .args(["compare", "--unified"])
        .arg(corpus("json/scruffy-generation1-S.json"))
        .arg(corpus("json/scruffy-generation1-M.json"))
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "scruffy-generation1-S.json: cannot unify",
        ));
}

#[test]
fn compare_unified_merges_split_records() {
    let dir = scratch("compare_unified_merges_split_records");
    let a = dir.join("a.provn");
    let b = dir.join("b.provn");
    std::fs::write(
        &a,
        "document\n prefix ex <http://example.org/>\n wasGeneratedBy(ex:g; ex:e, -, -)\n wasGeneratedBy(ex:g; -, ex:a, -)\nendDocument\n",
    )
    .unwrap();
    std::fs::write(
        &b,
        "document\n prefix ex <http://example.org/>\n wasGeneratedBy(ex:g; ex:e, ex:a, -)\nendDocument\n",
    )
    .unwrap();
    provkit().args(["compare"]).arg(&a).arg(&b).assert().code(1);
    provkit()
        .args(["compare", "--unified"])
        .arg(&a)
        .arg(&b)
        .assert()
        .success();
}

#[test]
fn compare_bundle_difference_uses_document_prefixes() {
    let dir = scratch("compare_bundle_difference_uses_document_prefixes");
    let a = dir.join("a.provn");
    let b = dir.join("b.provn");
    std::fs::write(
        &a,
        "document\n prefix ex <http://example.org/>\n bundle ex:b1\n  entity(ex:e1)\n  entity(ex:e2)\n endBundle\nendDocument\n",
    )
    .unwrap();
    std::fs::write(
        &b,
        "document\n prefix ex <http://example.org/>\n bundle ex:b1\n  entity(ex:e1)\n endBundle\nendDocument\n",
    )
    .unwrap();
    provkit()
        .args(["compare"])
        .arg(&a)
        .arg(&b)
        .assert()
        .code(1)
        .stdout(
            predicate::str::contains("  bundle ex:b1:\n    entity(ex:e2)\n")
                .and(predicate::str::contains("ns1").not()),
        )
        .stderr("");
}

#[test]
fn compare_read_error_exits_two() {
    let dir = scratch("compare_read_error_exits_two");
    provkit()
        .args(["compare"])
        .arg(dir.join("missing.provn"))
        .arg(corpus("json/entity1.json"))
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::contains("missing.provn"));
}

#[test]
fn compare_help_names_the_exit_codes() {
    provkit()
        .args(["compare", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("0 when the documents are equal")
                .and(predicate::str::contains("1 when they differ"))
                .and(predicate::str::contains("2 on a read error")),
        );
}

#[test]
fn compare_empty_bundle_against_none_prints_only_the_heading() {
    let dir = scratch("compare_empty_bundle_against_none_prints_only_the_heading");
    let a = dir.join("a.provn");
    let b = dir.join("b.provn");
    std::fs::write(
        &a,
        "document\n prefix ex <http://example.org/>\n bundle ex:b\n endBundle\nendDocument\n",
    )
    .unwrap();
    std::fs::write(
        &b,
        "document\n prefix ex <http://example.org/>\nendDocument\n",
    )
    .unwrap();
    provkit()
        .args(["compare"])
        .arg(&a)
        .arg(&b)
        .assert()
        .code(1)
        .stdout(format!("only in {}:\n  bundle ex:b:\n", a.display()))
        .stderr("");
}

/// A name in a namespace whose IRI PROV-N cannot write. Every local part
/// is one PROV-N can write, because the readers accept no other.
#[test]
fn compare_marks_records_prov_n_cannot_write() {
    let dir = scratch("compare_marks_records_prov_n_cannot_write");
    let a = dir.join("a.json");
    let b = dir.join("b.json");
    std::fs::write(
        &a,
        r#"{"prefix": {"ex": "http://example.org/", "bad": "http://x.org/my ns/"},
            "entity": {"bad:entity": {}, "ex:e": {"bad:attr": "v"}, "ex:f": {"ex:a": {"$": "x", "type": "bad:type"}}},
            "used": {"_:u": {"prov:activity": "bad:activity"}},
            "bundle": {"bad:bundle": {"entity": {"ex:inner": {}}}}}"#,
    )
    .unwrap();
    std::fs::write(&b, r#"{"prefix": {"ex": "http://example.org/"}}"#).unwrap();
    provkit()
        .args(["compare"])
        .arg(&a)
        .arg(&b)
        .assert()
        .code(1)
        .stdout(format!(
            "only in {}:\n  \
             entity(<http://x.org/my ns/entity>) // not valid PROV-N\n  \
             entity(ex:e, [<http://x.org/my ns/attr>]) // not valid PROV-N\n  \
             entity(ex:f, [<http://x.org/my ns/type>]) // not valid PROV-N\n  \
             used(<http://x.org/my ns/activity>) // not valid PROV-N\n  \
             bundle <http://x.org/my ns/bundle>: // not valid PROV-N\n    \
             entity(ex:inner)\n",
            a.display()
        ))
        .stderr("");
}

#[test]
fn compare_declares_the_prefixes_it_mints() {
    let dir = scratch("minted");
    let a = dir.join("a.json");
    let b = dir.join("b.json");
    std::fs::write(
        &a,
        r#"{"prefix": {"default": "http://one.org/"}, "entity": {"12": {}},
        "bundle": {"b": {"prefix": {"default": "http://two.org/"}, "entity": {"34": {}}}}}"#,
    )
    .unwrap();
    std::fs::write(&b, r#"{"prefix": {"default": "http://one.org/"}}"#).unwrap();
    let out = provkit()
        .args(["compare"])
        .arg(&a)
        .arg(&b)
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let out = String::from_utf8(out).unwrap();
    assert!(out.contains("entity(ns1:12)"), "{out}");
    assert!(out.contains("prefix ns1 <http://one.org/>"), "{out}");
    assert!(
        out.contains("prefix ns1 <http://two.org/>")
            || out.contains("prefix ns2 <http://two.org/>"),
        "{out}"
    );
}

#[test]
fn compare_does_not_redeclare_a_declared_prefix() {
    let dir = scratch("compare_does_not_redeclare_a_declared_prefix");
    let a = dir.join("a.provn");
    let b = dir.join("b.provn");
    std::fs::write(&a, PROVN).unwrap();
    std::fs::write(
        &b,
        "document\n  prefix ex <http://example.org/>\nendDocument\n",
    )
    .unwrap();
    provkit()
        .args(["compare"])
        .arg(&a)
        .arg(&b)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("  prefix ").not());
}

#[test]
fn compare_with_unrecognisable_standard_input_offers_no_flag() {
    provkit()
        .args(["compare", "-", "-"])
        .write_stdin("hello")
        .assert()
        .code(2)
        .stderr("provkit: cannot tell the format of standard input\n");
    provkit()
        .args(["convert", "-"])
        .write_stdin("hello")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("use -f json or -f provn"));
}

#[test]
fn a_closed_output_pipe_ends_quietly() {
    use std::io::Read;
    use std::process::Stdio;

    // A document large enough to fill the pipe buffer before the reader closes.
    let dir = scratch("a_closed_output_pipe_ends_quietly");
    let big = dir.join("big.provn");
    let entities = "  entity(ex:e)\n".repeat(50_000);
    let text = format!("document\n  prefix ex <http://example.org/>\n{entities}endDocument\n");
    std::fs::write(&big, text).unwrap();

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_provkit"))
        .arg("convert")
        .arg(&big)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut first = [0u8; 8];
    stdout.read_exact(&mut first).unwrap();
    drop(stdout);
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    let status = child.wait().unwrap();
    assert_eq!(stderr, "");
    assert_eq!(status.code(), Some(0));
}

#[test]
fn an_output_file_with_an_unknown_extension_needs_a_format() {
    let dir = scratch("an_output_file_with_an_unknown_extension_needs_a_format");
    let out = dir.join("out.txt");
    provkit()
        .args(["convert"])
        .arg(corpus("json/entity1.json"))
        .arg("-o")
        .arg(&out)
        .assert()
        .code(2)
        .stderr(format!(
            "provkit: cannot tell the output format from '{}'; use -t json or -t provn\n",
            out.display()
        ));
    assert!(!out.exists());
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
fn ignored_extensibility_expressions_warn_once_on_standard_error() {
    let dir = scratch("ignored_extensibility_expressions_warn_once");
    let path = dir.join("ext.provn");
    std::fs::write(
        &path,
        "document\n  prefix ex <http://example.org/>\n  prefix dictExt <http://example.org/dict#>\n  entity(ex:e1)\n  dictExt:hadMembers(ex:e1, ex:a)\n  dictExt:hadMembers(ex:e1, ex:b)\nendDocument\n",
    )
    .unwrap();
    provkit()
        .args(["convert"])
        .arg(&path)
        .assert()
        .success()
        .stderr(format!(
            "provkit: warning: {}: ignored 2 extensibility expression(s), first dictExt:hadMembers at line 5, column 3\n",
            path.display()
        ));
    provkit()
        .args(["convert", "-"])
        .write_stdin(std::fs::read_to_string(&path).unwrap())
        .assert()
        .success()
        .stderr(predicate::str::starts_with(
            "provkit: warning: standard input: ignored 2",
        ));
    provkit()
        .args(["compare"])
        .arg(&path)
        .arg(&path)
        .assert()
        .success()
        .stdout("")
        .stderr(predicate::str::contains("ignored 2").count(2));
}

#[test]
fn a_word_starting_with_document_is_not_provn() {
    provkit()
        .args(["convert", "-"])
        .write_stdin("documentary\n")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot tell the format"));
}
