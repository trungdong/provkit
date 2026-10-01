//! End-to-end tests for the `provkit` binary.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use assert_cmd::assert::Assert;
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

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

/// `a.{ext}` and `b.{ext}` in a fresh scratch directory.
fn two_docs(test: &str, ext: &str, a: &str, b: &str) -> (PathBuf, PathBuf) {
    let dir = scratch(test);
    (
        write(&dir, &format!("a.{ext}"), a),
        write(&dir, &format!("b.{ext}"), b),
    )
}

fn stdout(assert: &Assert) -> String {
    String::from_utf8(assert.get_output().stdout.clone()).unwrap()
}

/// `provkit convert {flags} {input}`.
fn convert(flags: &[&str], input: impl AsRef<OsStr>) -> Assert {
    provkit().arg("convert").args(flags).arg(input).assert()
}

/// `provkit convert {flags} -`, reading `input` from standard input.
fn convert_stdin(flags: &[&str], input: impl Into<Vec<u8>>) -> Assert {
    provkit()
        .arg("convert")
        .args(flags)
        .arg("-")
        .write_stdin(input)
        .assert()
}

/// `provkit convert {flags} json/entity1.json -o {out}`.
fn convert_entity1_to(flags: &[&str], out: &Path) -> Assert {
    provkit()
        .arg("convert")
        .args(flags)
        .arg(corpus("json/entity1.json"))
        .arg("-o")
        .arg(out)
        .assert()
}

fn compare(a: impl AsRef<OsStr>, b: impl AsRef<OsStr>) -> Assert {
    provkit().arg("compare").arg(a).arg(b).assert()
}

fn compare_unified(a: impl AsRef<OsStr>, b: impl AsRef<OsStr>) -> Assert {
    provkit()
        .args(["compare", "--unified"])
        .arg(a)
        .arg(b)
        .assert()
}

/// Runs `provkit {command} {files}`, reads 8 bytes of standard output, closes
/// it, and returns standard error and the exit code.
fn close_stdout_early(command: &str, files: &[&Path]) -> (String, Option<i32>) {
    use std::io::Read;
    use std::process::Stdio;

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_provkit"))
        .arg(command)
        .args(files)
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
    (stderr, child.wait().unwrap().code())
}

const PROVN: &str = "document\n  prefix ex <http://example.org/>\n  entity(ex:e1)\nendDocument\n";

#[test]
fn convert_json_to_provn_on_stdout_by_default() {
    convert(&[], corpus("json/entity1.json"))
        .success()
        .stdout(predicate::str::starts_with("document\n"))
        .stdout(predicate::str::contains("entity(ex:e1"))
        .stderr("");
}

#[test]
fn convert_to_file_by_extension_and_back() {
    let dir = scratch("convert_to_file_by_extension_and_back");
    let provn = dir.join("out.provn");
    convert_entity1_to(&[], &provn)
        .success()
        .stdout("")
        .stderr("");
    assert!(read(&provn).starts_with("document"));
    let json = dir.join("out.json");
    provkit()
        .args(["convert"])
        .arg(&provn)
        .arg("-o")
        .arg(&json)
        .assert()
        .success();
    let text = read(&json);
    assert!(text.trim_start().starts_with('{'));
    let original = read(&corpus("json/entity1.json"));
    assert_eq!(
        provkit::json::read(&text).unwrap(),
        provkit::json::read(&original).unwrap()
    );
}

#[test]
fn stdin_is_sniffed_and_stdout_needs_a_flag_for_json() {
    let input = read(&corpus("json/entity1.json"));
    convert_stdin(&["-t", "json"], input.clone())
        .success()
        .stdout(predicate::str::starts_with("{"))
        .stderr("");
    convert_stdin(&[], input)
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
    convert_entity1_to(&[], &out).success();
    assert!(read(&out).trim_start().starts_with('{'));
    let provn = write(&dir, "in.PROVN", PROVN);
    convert(&["-t", "json"], &provn)
        .success()
        .stdout(predicate::str::starts_with("{"));

    // Content that sniffs as PROV-JSON is still read as PROV-N: the
    // upper-case extension decides.
    let json_looking = write(&dir, "json-looking.PROVN", "{}");
    convert(&[], &json_looking)
        .code(2)
        .stderr(predicate::str::starts_with(format!(
            "provkit: {}:1:1: ",
            json_looking.display()
        )));
}

#[test]
fn a_leading_byte_order_mark_is_ignored() {
    let dir = scratch("a_leading_byte_order_mark_is_ignored");
    let json_text = read(&corpus("json/entity1.json"));
    let cases = [
        ("bom.json", format!("\u{FEFF}{json_text}")),
        ("bom.provn", format!("\u{FEFF}{PROVN}")),
        ("bom.txt", format!("\u{FEFF}{PROVN}")),
    ];
    for (name, text) in cases {
        convert(&[], write(&dir, name, &text))
            .success()
            .stdout(predicate::str::contains("entity(ex:e1"))
            .stderr("");
    }
    for text in [format!("\u{FEFF}{json_text}"), format!("\u{FEFF}{PROVN}")] {
        convert_stdin(&[], text)
            .success()
            .stdout(predicate::str::contains("entity(ex:e1"))
            .stderr("");
    }
    // Only the first character is a byte-order mark; a second one is content.
    convert_stdin(&[], format!("\u{FEFF}\u{FEFF}{PROVN}")).code(2);
}

#[test]
fn to_flag_overrides_output_extension() {
    let dir = scratch("to_flag_overrides_output_extension");
    let out = dir.join("out.json");
    convert_entity1_to(&["-t", "provn"], &out).success();
    assert!(read(&out).starts_with("document"));
}

#[test]
fn sniffing_skips_comments_and_blank_lines() {
    convert_stdin(&["-t", "json"], format!("\n// lead\n/* block */\n{PROVN}"))
        .success()
        .stdout(predicate::str::starts_with("{"));
}

#[test]
fn from_flag_overrides_extension() {
    let dir = scratch("from_flag_overrides_extension");
    let lying = write(&dir, "really-provn.json", PROVN);
    convert(&["-f", "provn", "-t", "json"], &lying)
        .success()
        .stdout(predicate::str::starts_with("{"));
    convert(&[], &lying)
        .code(2)
        .stderr(predicate::str::starts_with("provkit: "))
        .stderr(predicate::str::contains("really-provn.json"));
}

#[test]
fn unreadable_input_exits_2() {
    convert(&[], "/nonexistent/file.json")
        .code(2)
        .stdout("")
        .stderr(predicate::str::starts_with(
            "provkit: /nonexistent/file.json",
        ));
    let dir = scratch("unreadable_input_exits_2");
    convert(&[], write(&dir, "junk.txt", "hello"))
        .code(2)
        .stderr(predicate::str::contains("junk.txt"))
        .stderr(predicate::str::contains("cannot tell the format"));
}

/// A library error names the file and prints its positioned message once.
#[test]
fn positioned_errors_print_their_message_once() {
    let dir = scratch("positioned_errors_print_their_message_once");
    let bad = write(
        &dir,
        "bad.provn",
        "document\n  entity(ex:e1)\nendDocument\n",
    );
    convert(&[], &bad).code(2).stderr(format!(
        "provkit: {}:2:10: undeclared prefix `ex`\n",
        bad.display()
    ));
}

#[test]
fn stdin_errors_name_the_input_as_stdin() {
    convert_stdin(&["-f", "json"], "{")
        .code(2)
        .stderr(predicate::str::starts_with("provkit: <stdin>: "));
}

const PRECEDENCE: &str = "taken from -f/-t when given, else from the file extension";

#[test]
fn help_states_the_precedence_rule() {
    provkit()
        .args(["convert", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(PRECEDENCE));
}

#[test]
fn unknown_extension_falls_through_to_sniffing() {
    let dir = scratch("unknown_extension_falls_through_to_sniffing");
    convert(&["-t", "json"], write(&dir, "x.txt", PROVN))
        .success()
        .stdout(predicate::str::starts_with("{"))
        .stderr("");
    let dat = dir.join("x.dat");
    std::fs::copy(corpus("json/entity1.json"), &dat).unwrap();
    convert(&[], &dat)
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
            .code(2)
            .stderr(predicate::str::contains("invalid value 'xml'"));
    }
}

#[test]
fn failed_write_names_the_output() {
    let dir = scratch("failed_write_names_the_output");
    let out = dir.join("no-such-dir").join("out.provn");
    convert_entity1_to(&[], &out)
        .code(2)
        .stderr(predicate::str::starts_with(format!(
            "provkit: {}: ",
            out.display()
        )));
}

#[test]
fn compare_equal_across_formats() {
    compare(
        corpus("json/entity1.json"),
        corpus("provn/provtoolbox-corpus/entity1.provn"),
    )
    .success()
    .stdout("")
    .stderr("");
}

#[test]
fn compare_reports_one_sided_records() {
    let m = corpus("json/scruffy-generation1-M.json");
    compare(corpus("json/scruffy-generation1-S.json"), &m)
        .code(1)
        .stdout(format!(
            "only in {}:\n  entity(ex:e1)\n  activity(ex:a1)\n",
            m.display()
        ))
        .stderr("");
}

#[test]
fn compare_unified_reports_conflicts_as_errors() {
    compare_unified(
        corpus("json/scruffy-generation1-S.json"),
        corpus("json/scruffy-generation1-M.json"),
    )
    .code(2)
    .stderr(predicate::str::contains(
        "scruffy-generation1-S.json: cannot unify",
    ));
}

#[test]
fn compare_unified_merges_split_records() {
    let (a, b) = two_docs(
        "compare_unified_merges_split_records",
        "provn",
        "document\n prefix ex <http://example.org/>\n wasGeneratedBy(ex:g; ex:e, -, -)\n wasGeneratedBy(ex:g; -, ex:a, -)\nendDocument\n",
        "document\n prefix ex <http://example.org/>\n wasGeneratedBy(ex:g; ex:e, ex:a, -)\nendDocument\n",
    );
    compare(&a, &b).code(1);
    compare_unified(&a, &b).success();
}

#[test]
fn compare_bundle_difference_uses_document_prefixes() {
    let (a, b) = two_docs(
        "compare_bundle_difference_uses_document_prefixes",
        "provn",
        "document\n prefix ex <http://example.org/>\n bundle ex:b1\n  entity(ex:e1)\n  entity(ex:e2)\n endBundle\nendDocument\n",
        "document\n prefix ex <http://example.org/>\n bundle ex:b1\n  entity(ex:e1)\n endBundle\nendDocument\n",
    );
    compare(&a, &b)
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
    compare(dir.join("missing.provn"), corpus("json/entity1.json"))
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
                .and(predicate::str::contains("2 on any error")),
        );
}

#[test]
fn compare_empty_bundle_against_none_prints_only_the_heading() {
    let (a, b) = two_docs(
        "compare_empty_bundle_against_none_prints_only_the_heading",
        "provn",
        "document\n prefix ex <http://example.org/>\n bundle ex:b\n endBundle\nendDocument\n",
        "document\n prefix ex <http://example.org/>\nendDocument\n",
    );
    compare(&a, &b)
        .code(1)
        .stdout(format!("only in {}:\n  bundle ex:b:\n", a.display()))
        .stderr("");
}

/// A name in a namespace whose IRI PROV-N cannot write. Every local part
/// is one PROV-N can write, because the readers accept no other.
#[test]
fn compare_marks_records_prov_n_cannot_write() {
    let (a, b) = two_docs(
        "compare_marks_records_prov_n_cannot_write",
        "json",
        r#"{"prefix": {"ex": "http://example.org/", "bad": "http://x.org/my ns/"},
            "entity": {"bad:entity": {}, "ex:e": {"bad:attr": "v"}, "ex:f": {"ex:a": {"$": "x", "type": "bad:type"}}},
            "used": {"_:u": {"prov:activity": "bad:activity"}},
            "bundle": {"bad:bundle": {"entity": {"ex:inner": {}}}}}"#,
        r#"{"prefix": {"ex": "http://example.org/"}}"#,
    );
    compare(&a, &b)
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
    let (a, b) = two_docs(
        "minted",
        "json",
        r#"{"prefix": {"default": "http://one.org/"}, "entity": {"12": {}},
        "bundle": {"b": {"prefix": {"default": "http://two.org/"}, "entity": {"34": {}}}}}"#,
        r#"{"prefix": {"default": "http://one.org/"}}"#,
    );
    let out = stdout(&compare(&a, &b).code(1));
    assert!(out.contains("entity(ns1:12)"), "{out}");
    assert!(out.contains("prefix ns1 <http://one.org/>"), "{out}");
    assert!(out.contains("    prefix ns1 <http://two.org/>"), "{out}");
}

#[test]
fn compare_mints_the_next_free_prefix() {
    // A bare name that starts with digits cannot be written without a
    // prefix, and `ns1` is already bound to another namespace.
    let (a, b) = two_docs(
        "compare_mints_the_next_free_prefix",
        "json",
        r#"{"prefix": {"default": "http://one.org/", "ns1": "http://taken.org/"},
        "entity": {"12": {}},
        "bundle": {"b": {"prefix": {"default": "http://two.org/"}, "entity": {"34": {}}}}}"#,
        "{}",
    );
    let out = stdout(&compare(&a, &b).code(1));
    assert_eq!(
        out.lines().filter(|l| l.contains("prefix ns")).count(),
        2,
        "{out}"
    );
    assert!(out.contains("  entity(ns2:12)"), "{out}");
    assert!(out.contains("  prefix ns2 <http://one.org/>"), "{out}");
    assert!(out.contains("    entity(ns2:34)"), "{out}");
    assert!(out.contains("    prefix ns2 <http://two.org/>"), "{out}");
}

#[test]
fn compare_does_not_redeclare_a_declared_prefix() {
    let (a, b) = two_docs(
        "compare_does_not_redeclare_a_declared_prefix",
        "provn",
        PROVN,
        "document\n  prefix ex <http://example.org/>\nendDocument\n",
    );
    compare(&a, &b)
        .code(1)
        .stdout(predicate::str::contains("  prefix ").not());
}

#[test]
fn compare_with_unrecognisable_standard_input_offers_no_flag() {
    provkit()
        .args(["compare", "-"])
        .arg(corpus("json/activity1.json"))
        .write_stdin("hello")
        .assert()
        .code(2)
        .stderr("provkit: cannot tell the format of standard input: it starts with neither `{` nor `document`\n");
    convert_stdin(&[], "hello")
        .code(2)
        .stderr("provkit: cannot tell the format of standard input; use -f json or -f provn\n");
}

#[test]
fn compare_reads_standard_input_once() {
    provkit()
        .args(["compare", "-", "-"])
        .write_stdin(read(&corpus("json/activity1.json")))
        .assert()
        .code(2)
        .stderr("provkit: standard input can be read once; give one document as a file\n");
}

#[test]
fn a_provn_error_is_printed_as_file_line_column() {
    let dir = scratch("a_provn_error_is_printed_as_file_line_column");
    let bad = write(
        &dir,
        "bad.provn",
        "document\nprefix ex <http://example.org/>\nentity(ex:a ex:b)\nendDocument\n",
    );
    convert(&[], &bad).code(2).stderr(format!(
        "provkit: {}:3:13: expected ',' or ')', found 'ex:b'\n",
        bad.display()
    ));
    let undeclared = write(
        &dir,
        "undeclared.provn",
        "document\nentity(zz:a)\nendDocument\n",
    );
    convert(&[], &undeclared).code(2).stderr(format!(
        "provkit: {}:2:8: undeclared prefix `zz`\n",
        undeclared.display()
    ));
    convert_stdin(&["-f", "provn"], "document\nentity(zz:a)\nendDocument\n")
        .code(2)
        .stderr("provkit: <stdin>:2:8: undeclared prefix `zz`\n");
}

#[test]
fn a_document_that_cannot_be_written_names_the_input() {
    let dir = scratch("a_document_that_cannot_be_written_names_the_input");
    let input = write(
        &dir,
        "in.json",
        r#"{"prefix": {"ex": "http://x.org/my ns/"}, "entity": {"ex:e": {}}}"#,
    );
    let output = dir.join("out.provn");
    provkit()
        .arg("convert")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .assert()
        .code(2)
        .stderr(predicate::str::starts_with(format!(
            "provkit: {}: PROV-N cannot write",
            input.display()
        )));
    assert!(!output.exists());
}

#[test]
fn only_grammar_whitespace_is_skipped_when_sniffing() {
    for lead in ["\u{2003}", "\u{c}", "\u{a0}"] {
        convert_stdin(&[], format!("{lead}document\nendDocument\n"))
            .code(2)
            .stderr(predicate::str::contains("cannot tell the format"));
    }
    convert_stdin(&[], " \t\r\ndocument\nendDocument\n").success();
}

#[test]
fn compare_names_standard_input_as_errors_do() {
    let s = read(&corpus("json/scruffy-generation1-S.json"));
    let m = read(&corpus("json/scruffy-generation1-M.json"));
    // The heading of a one-sided record.
    provkit()
        .args(["compare"])
        .arg(corpus("json/scruffy-generation1-S.json"))
        .arg("-")
        .write_stdin(m)
        .assert()
        .code(1)
        .stdout("only in <stdin>:\n  entity(ex:e1)\n  activity(ex:a1)\n")
        .stderr("");
    // The context of a `--unified` error, on either side.
    for args in [["-", "json/activity1.json"], ["json/activity1.json", "-"]] {
        let mut command = provkit();
        command.args(["compare", "--unified"]);
        for arg in args {
            if arg == "-" {
                command.arg(arg);
            } else {
                command.arg(corpus(arg));
            }
        }
        command
            .write_stdin(s.clone())
            .assert()
            .code(2)
            .stderr(predicate::str::starts_with(
                "provkit: <stdin>: cannot unify",
            ));
    }
}

#[test]
fn compare_unified_reports_a_conflict_in_the_second_file() {
    compare_unified(
        corpus("json/activity1.json"),
        corpus("json/scruffy-generation1-S.json"),
    )
    .code(2)
    .stderr(predicate::str::contains(
        "scruffy-generation1-S.json: cannot unify",
    ));
}

#[test]
fn the_top_level_help_is_about_the_tool_and_compare_names_standard_input() {
    let long = stdout(&provkit().arg("--help").assert().success());
    let short = stdout(&provkit().arg("-h").assert().success());
    assert_eq!(long.lines().next(), short.lines().next());
    assert!(!long.contains("-f/-t"), "{long}");
    provkit()
        .args(["compare", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("or - for standard input"))
        .stdout(predicate::str::contains("Only one of the two may be -"));
}

#[test]
fn compare_lists_a_repeated_record_once_and_uses_a_bundle_s_own_prefix() {
    let (a, b) = two_docs(
        "compare_lists_a_repeated_record_once",
        "provn",
        "document\n prefix ex <http://example.org/>\n entity(ex:e)\n entity(ex:e)\n \
         bundle ex:b\n  prefix in <http://inner.org/>\n  entity(in:x)\n endBundle\nendDocument\n",
        "document\nendDocument\n",
    );
    compare(&a, &b).code(1).stdout(format!(
        "only in {}:\n  entity(ex:e)\n  bundle ex:b:\n    entity(in:x)\n",
        a.display()
    ));
}

#[test]
fn a_closed_output_pipe_ends_quietly() {
    // A document large enough to fill the pipe buffer before the reader closes.
    let dir = scratch("a_closed_output_pipe_ends_quietly");
    let entities = "  entity(ex:e)\n".repeat(50_000);
    let big = write(
        &dir,
        "big.provn",
        &format!("document\n  prefix ex <http://example.org/>\n{entities}endDocument\n"),
    );
    let (stderr, code) = close_stdout_early("convert", &[&big]);
    assert_eq!(stderr, "");
    assert_eq!(code, Some(0));
}

#[test]
fn compare_keeps_its_exit_code_when_the_output_pipe_closes() {
    use std::fmt::Write as _;

    // One side holds 50 000 distinct entities, so the report fills the pipe
    // buffer before the reader closes it.
    let dir = scratch("compare_keeps_its_exit_code_when_the_output_pipe_closes");
    let entities = (0..50_000).fold(String::new(), |mut out, i| {
        writeln!(out, "  entity(ex:e{i})").unwrap();
        out
    });
    let big = write(
        &dir,
        "big.provn",
        &format!("document\n  prefix ex <http://example.org/>\n{entities}endDocument\n"),
    );
    let empty = write(&dir, "empty.provn", "document\nendDocument\n");
    let (stderr, code) = close_stdout_early("compare", &[&big, &empty]);
    assert_eq!(stderr, "");
    // The documents differ, so the closed pipe leaves the exit code at 1.
    assert_eq!(code, Some(1));
}

#[test]
fn an_output_file_with_an_unknown_extension_needs_a_format() {
    let dir = scratch("an_output_file_with_an_unknown_extension_needs_a_format");
    let out = dir.join("out.txt");
    convert_entity1_to(&[], &out).code(2).stderr(format!(
        "provkit: cannot tell the output format from '{}'; use -t json or -t provn\n",
        out.display()
    ));
    assert!(!out.exists());
    convert_entity1_to(&["-t", "provn"], &out).success();
    assert!(read(&out).starts_with("document"));
}

#[test]
fn ignored_extensibility_expressions_warn_once_on_standard_error() {
    let dir = scratch("ignored_extensibility_expressions_warn_once");
    let path = write(
        &dir,
        "ext.provn",
        "document\n  prefix ex <http://example.org/>\n  prefix dictExt <http://example.org/dict#>\n  entity(ex:e1)\n  dictExt:hadMembers(ex:e1, ex:a)\n  dictExt:hadMembers(ex:e1, ex:b)\nendDocument\n",
    );
    convert(&[], &path).success().stderr(format!(
        "provkit: warning: {}: ignored 2 extensibility expression(s), first dictExt:hadMembers at line 5, column 3\n",
        path.display()
    ));
    convert_stdin(&[], read(&path))
        .success()
        .stderr(predicate::str::starts_with(
            "provkit: warning: <stdin>: ignored 2",
        ));
    compare(&path, &path)
        .success()
        .stdout("")
        .stderr(predicate::str::contains("ignored 2").count(2));
}

#[test]
fn a_word_starting_with_document_is_not_provn() {
    convert_stdin(&[], "documentary\n")
        .code(2)
        .stderr(predicate::str::contains("cannot tell the format"));
}
