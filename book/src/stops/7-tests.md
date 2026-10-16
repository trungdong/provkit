# Stop 7: Tests

The tests live in three places. Unit tests sit beside each module and
reach private items. The corpus tests under `provkit/tests` run every
file of a corpus shared with ProvPy, and every excluded file and known
difference is listed in a data file with a reason. The end-to-end
tests under `provkit-cli/tests` run the built binary. The repository's
`CLAUDE.md` states the layout in two sentences, unit tests beside each
module in a sibling `tests.rs` or an inline `mod tests`, and corpus
tests in `provkit/tests` with every excluded file and known difference
listed in `provkit/tests/data/*.tsv` with a reason. This stop reads
the helpers the unit tests share, the report
that collects failures across a corpus, the test that reads every
PROV-JSON file, the test that checks every probe against its row, the
three data files, the binary's tests and the manifest lines that
decide which tests ship. It builds on every stop before it, on stop 0
for the `test_util` module under `#[cfg(test)]` and the `[[test]]`
targets, on stop 1 for `mod tests;`, on stop 2 for `DateTime::parse`
and argument-position `impl Trait`, on stop 3 for the equality every
round trip ends in, on stop 4 for `json::read` and `json::to_string`,
on stop 5 for `provn::read_with` and the two profiles, and on stop 6
for the binary the end-to-end tests run.

## What the code does

### provkit/src/test_util.rs

Stop 0 showed `lib.rs` declaring this module as `pub(crate)` under
`#[cfg(test)]`, so it exists only while tests build and every unit
test in the crate can `use crate::test_util`. The larger modules load
a sibling `tests.rs` through the `mod tests;` line stop 1 introduced,
and the smaller ones hold `mod tests { ... }` at the end of their own
file, as stop 1 said of `qname.rs`. Either way the tests are a child
module of the code they test, so they see its private items. The
first half of the file is builders. `ex` and `prov` make a name in
`http://example.org/` or the PROV namespace, `entity`, `agent`,
`bundle` and `document` build one record, bundle or document under
`ex`, `dt` is one fixed `dateTime`, and `element` and `relation` go
through `Record::from_parts`. Each goes through `unwrap` on a
constructor that cannot fail for the inputs the tests pass, which is
the idiom of test code. `syntax` and `located` take an
[`Error`](../api/provkit/enum.Error.html) apart, returning the
position and the message of a `Syntax` or the position and the inner
error of an `At`, and `panic!` with the error in the message for any
other variant, so a test that expects a syntax error reads as one
line.

```rust,ignore
{{#include ../../../provkit/src/test_util.rs:broken}}
```

`Broken` is a unit struct, a type with no fields, and it implements
`std::io::Write`. Its `write` returns an error built by
`io::Error::other` on every call, and its `flush` succeeds. The
writers of stops 4 and 5 take `impl std::io::Write`, as the
`CLAUDE.md` rule says the library must, so a test hands them
`&mut Broken` and checks the error path without a file system.
`failing_writer_reports_io` in `json/write.rs` and
`io_failure_is_reported_as_io` in `provn/write/tests.rs` both assert
that the result is
[`Error::Io`](../api/provkit/enum.Error.html#variant.Io), the variant
stop 3 showed wrapping an `io::Error`.

```rust,ignore
{{#include ../../../provkit/src/test_util.rs:round_trip}}
```

`written`, above the span, serialises a document with
[`json::to_string`](../api/provkit/json/fn.to_string.html) and parses
the text again with `serde_json`, so a test gets both the text and a
`serde_json::Value` it can index into. `round_trip` calls `written`
and then asserts that
[`json::read`](../api/provkit/json/fn.read.html) on the text gives a
document equal to the one written. The third argument of `assert_eq!`
is a format message, and `"{text}"` puts the written JSON in the
failure, so a failing round trip shows what was produced. Both
functions carry `#[cfg(feature = "json")]`, because a build with the
`provn` feature alone has no `serde_json`. The tests in
`json/write.rs` and `json/read/tests.rs` use them.

### provkit/tests/common/mod.rs

Each file under `provkit/tests` is an integration test, which Cargo
compiles as its own crate against the public API of `provkit`. Each
begins with `mod common;`, which loads `common/mod.rs` into that
crate, so the helpers here are compiled once per test file. The file
opens with `#![allow(dead_code)]` and a comment saying why, since a
helper one test file uses is unused in another and the warning would
fail the build under `-D warnings`. `JSON_CORPUS_SIZE` is the number
of PROV-JSON documents in the corpus, and the tests that read the
whole set pass it to `finish`. `ex`, `dt`, `element`, `relation` and
`document` are the builders of `test_util` again, written out a
second time because an integration test cannot see a `pub(crate)`
module. `data_path` and `corpus_dir` build a path from
`env!("CARGO_MANIFEST_DIR")`, the directory holding
`provkit/Cargo.toml`, fixed into the binary at compile time, so
`cargo test` finds `tests/data` and `../corpus` from any working
directory. `tsv` reads a file into rows of tab-separated columns and
drops blank lines, and `pairs` demands two columns per row and panics
naming the file otherwise. `files` lists every file with one
extension in a directory, sorted by name, reads each, and keeps the
ones `selected` admits. `selected` reads `PROVKIT_ONLY` through
`only`, and when the variable is set it keeps a name that contains
the value as a substring, so `PROVKIT_ONLY=entity1` keeps
`entity1.provn` and `entity10.provn` both. `require_a_match` fails a
run whose filter left no file, so a mistyped filter cannot pass as an
empty run, and `files_unchecked` serves the one test that reads two
directories and checks the union itself.

```rust,ignore
{{#include ../../../provkit/tests/common/mod.rs:report}}
```

`Report` holds the failures and a count of cases checked. `fail`
records one failure as `name: why`, taking the reason as
`impl Display` so a caller passes a `&str`, a `String` or an error.
`read_json` reads a text as PROV-JSON and returns the document, or
records the failure and returns `None`, so a caller writes
`let Some(d) = report.read_json(..) else { continue }` and moves on
to the next file. It carries `#[cfg(feature = "json")]` because
`corpus_probes` builds under the `provn` feature alone. `round_trip`
takes the writer and the reader as two `impl FnOnce` arguments, so
one method serves both formats. `corpus_json.rs` passes the functions
`json::to_string` and `json::read`, and `corpus_twins.rs` passes
[`provn::to_string`](../api/provkit/provn/fn.to_string.html) and a
closure that calls `read_with` under `Profile::Default`. A write
failure, a re-read failure and a document that differs after the trip
are three distinct messages, and `back == *document` is the set-based
equality of stop 3, so record order and the namespace table do not
count. `finish` consumes the report. It asserts the checked count
against the expected one only when no filter is set, as its comment
says, and then asserts that there are no failures, with a message
that counts them and lists every one, so one bad file does not hide
the rest.

### provkit/tests/corpus_json.rs

The corpus is the git submodule `corpus/`, whose `.gitmodules` entry
points at the `prov-corpus` repository. `CLAUDE.md` says ProvPy
shares it, and that it is read-only here, so a corpus change goes to
that repository and arrives as a submodule bump. The sentence of
[CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md)
beginning "The tests read" names the tag the submodule is pinned at,
and `git submodule status` prints the same tag beside the commit.
Its layout is in `corpus/README.md`. `json/` holds the PROV-JSON
documents of the shared ProvToolbox corpus, `provn/spec/prov-n/` and
`provn/spec/prov-dm/` hold the examples of the two Recommendations
wrapped in a document, `provn/provtoolbox/` holds ProvToolbox's own
test documents, `provn/provtoolbox-corpus/` holds the PROV-N that
ProvToolbox wrote from the JSON corpus, one twin per JSON file of the
same name, `provn/probes/` holds the probes and `expected.tsv`, and
`constraints/` holds the PROV-CONSTRAINTS unification cases.

```rust,ignore
{{#include ../../../provkit/tests/corpus_json.rs:reads_all}}
```

`reads_all_398` is the shape of every corpus test. It lists the files
of `corpus/json`, counts each, reads it through `read_json`, and hands
the report `JSON_CORPUS_SIZE`, which is
the number its name carries. `round_trips_all_398` below it reads
each file and then calls `round_trip` with the two PROV-JSON
functions. `unification_over_the_corpus` calls
[`unified`](../api/provkit/struct.Document.html#method.unified) from
stop 3 on every document, expects the `scruffy-*` files to fail with a
`Unification` error on a formal, checks the ones that fail on a name
against a constant list and counts the ones that fail on a time, and
checks that every other document unifies, that unifying twice gives
the same result and that unification never adds a record. The
constraints test does the same over `corpus/constraints`, with one
constant list of the files whose failing formal is present on one
side only and another pairing each other failing file with its cause.
The last test in the file reads the first Rust block of both READMEs
with `include_str!` and asserts they are the same text, and its
comment says it lives in this
file because the root README is not in the packaged crate. The other
corpus files follow the same shape. `corpus_provn.rs` reads the
Recommendation examples under `Profile::Strict` and ProvToolbox's
documents under `Profile::Default`, and checks the extensibility
expressions the reader leaves out. `corpus_twins.rs` reads every twin
and compares it with its JSON file, and round-trips the JSON corpus
through PROV-N. `provn_round_trip.rs` generates names, values and
namespace IRIs from a character palette and checks that whatever the
PROV-N writer accepts reads back equal.

### provkit/tests/corpus_probes.rs

The module doc says what the test checks. Every probe in
`corpus/provn/probes` reads or fails as `expected.tsv` says, under
both profiles. `PROBE_COUNT` is the number of probes, and `HEADER`
names the five columns, `name`, `strict`, `default`, `rule` and
`provpy`. `Outcome` is `Accept` or `Reject`, parsed from a column or
taken from whether a read returned `Ok`. `expected`, above the span,
reads the table through `tsv`, asserts the header, and turns each row
into an `Expected` keyed by name. Its pattern
`([name, strict, default, rule] | [name, strict, default, rule, _])`
accepts a row of four or five columns, because the `provpy` column is
empty for a probe on which ProvPy agrees and a trailing empty column
may be absent, as the comment says. A row with another width, a
result that is not `accept` or `reject`, an empty `rule` column or a
name listed twice is recorded as a failure, so the table itself is
under test.

```rust,ignore
{{#include ../../../provkit/tests/corpus_probes.rs:probes}}
```

The test reads every `.provn` file in the directory, strips the
extension to get the probe's name, and looks the row up. A probe with
no row fails. For each of the two profiles it reads the text with
[`provn::read_with`](../api/provkit/provn/fn.read_with.html) from
stop 5, turns the result into an `Outcome`, and records a failure
naming the profile when it differs from the row. When no filter is
set it also checks the table the other way, so a row with no probe
file fails, and `finish` is given `PROBE_COUNT`. The `rule` column is
not read by the test beyond the check that it is not empty. It is for
the reader. `corpus/provn/probes/README.md` says it holds what decides
the row, a production number, a section, or a terminal the grammar
takes from another specification, and that a rule ending in
`decision` marks a row the Recommendation does not decide.

### provkit/tests/data

Three tab-separated files list every excluded file and known
difference. Each
row is a file name, a tab and a reason or a category, and the doc
comments on the functions that read them say the names and reasons
follow `corpus/provn/README.md`. The shortest is included whole.

```text
{{#include ../../../provkit/tests/data/excluded_provtoolbox_documents.tsv}}
```

`excluded_provtoolbox_documents.tsv` is read by
`provtoolbox_files_parse_under_default` in `corpus_provn.rs`, which
reads every file in `corpus/provn/provtoolbox` under
`Profile::Default`, fails when a listed file is accepted, and counts
only the files that are not listed. `excluded_spec_examples.tsv` lists
the Recommendation examples the grammar rejects or that are not
complete PROV-N, and `spec_examples_parse_under_strict` reads both
`spec` directories under `Profile::Strict`. A listed example that is
accepted fails, and so does one that fails for a reason other than
the one its row gives, which `fails_as_listed` checks. Example 61 must
fail with
[`Error::NoDefaultNamespace`](../api/provkit/enum.Error.html#variant.NoDefaultNamespace)
inside an `At`, because its reason is a bundle name with no default
namespace, and every other listed example must fail with
`Error::Syntax`. `known_differences.tsv` has a category in its second
column, `DynamicTimestamp` or `DatatypePalette`, and
`provtoolbox_corpus_equals_json_twins` in `corpus_twins.rs` reads it
once. Every twin must read, listed or not, since the match arm for a
read error comes first and the comment says only equality is waived.
A listed twin that reads equal to its JSON file fails the test, so a
row that is no longer needed cannot stay. When no filter is set the
test also checks the list both ways, each row naming a file that was
read and no file listed twice, and asserts the count of rows per
category against the literals in the test.

### provkit-cli/tests/cli.rs

The binary's tests run it as a process. `provkit` returns
`assert_cmd::Command::cargo_bin("provkit")`, the binary Cargo built
for this test run, and the helpers `convert`, `convert_stdin`,
`convert_entity1_to`, `compare` and `compare_unified` add the
arguments and call `assert()`, which runs the command and returns an
`Assert` for the test to chain checks on. A test then calls
`.success()` or `.code(..)` for the exit code and `.stdout(..)` and
`.stderr(..)` with a literal or a predicate from the `predicates`
crate, such as `predicate::str::starts_with("document\n")` or
`contains(..)` joined with `.and(..)`. The path arguments are
`impl AsRef<OsStr>`, so a test passes a `&str`, a `PathBuf` or a
reference to one alike. `scratch` makes a fresh directory named after
the test under `CARGO_TARGET_TMPDIR`, the directory Cargo gives
integration tests for files, and `write` puts a document in it.
`close_stdout_early` uses `std::process::Command` with
`env!("CARGO_BIN_EXE_provkit")` instead, because it closes the child's
standard output after eight bytes, as its doc says, and then reads
standard error and the exit code. Several tests pin the help text,
among them `help_states_the_precedence_rule` for the order
in which `-f`, `-t` and the extension decide a format and
`compare_help_names_the_exit_codes` for the three codes stop 6
described. The rest exercise one behaviour each, the sniffer on
standard input, a byte order mark, a positioned error, a closed pipe,
and the shape of a comparison listing.

### provkit/Cargo.toml

Stop 0 read the `[[test]]` targets and their `required-features`,
so a feature-subset build skips a target rather than failing to
compile it. The lines above those targets decide what the published
crate carries.

```toml
{{#include ../../../provkit/Cargo.toml:exclude}}
```

`exclude = ["tests/corpus_*.rs", "tests/data/"]` removes the corpus
tests and their data from the published crate, and its comment says
why. They read the corpus submodule, which the
`.crate` archive lacks, and `tests/common/` ships because
`provn_round_trip` declares it and must compile from the tarball.
`provkit-cli/Cargo.toml` excludes `tests/cli.rs` with a comment of the
same form, since the end-to-end tests read the corpus too.

## Why this shape

### Why a corpus shared with ProvPy?

`CLAUDE.md` describes `corpus/` as a git submodule shared with ProvPy,
and its conformance rules say not to deviate from ProvPy without a
CONFORMANCE.md row giving the reason and the issue number. The corpus
section of
[CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md)
opens with the paragraph beginning "The tests read", which names the
corpus and adds that ProvPy shares it, and the probes paragraph beginning
"`expected.tsv` gives the result" says both profiles must match for
every probe. The corpus's own README lists the two libraries that
consume it and notes that each keeps its own copy of the exclusion
list. The commit "Scaffold the workspace, CI and the corpus submodule"
adds the submodule without stating a reason. The sources establish
that the files are the same ones ProvPy reads and that the no-deviation
rule is checked against them; none states in so many words that the
sharing exists for that purpose.

### Why does `expected.tsv` carry a `rule` column?

`corpus/provn/probes/README.md` says each probe isolates one
production or rule of the Recommendation, and that the `rule` column
holds what decides the row, a production number such as `[53]`, a
section such as `3.7.4`, or a terminal the grammar leaves to another
specification. The commit "Test against the shared PROV-N probes"
lists an empty `rule` column among the things the test fails on, and
the span above shows that check in `expected`. The CONFORMANCE.md
probes paragraph beginning "`expected.tsv` gives the result" says both
profiles must match for every probe. Together they give each probe a
name, a production and an expected result, so a change that breaks a
production fails a probe that isolates it, with the production in its
row.

### Why a reason on every exclusion?

`CLAUDE.md` requires every excluded file and known difference to be
listed in `provkit/tests/data/*.tsv` with a reason. The commit "Move
the corpus exclusion lists to data files" says the three lists had
followed two conventions, Rust constants whose reasons duplicated the
corpus README and a TSV, and now follow one policy, a TSV in
`tests/data` with a name and a reason column read through the shared
`tsv` helper. The commit "Make each cited test able to fail on its
claim" adds that excluded spec examples must fail with the kind their
reason names and that a phantom or duplicate row in
`known_differences.tsv` fails, which is the `fails_as_listed` check
and the two-way check described above. The CONFORMANCE.md corpus
section records the exclusions in prose, under the headings "PROV-N
specification examples", "ProvToolbox files" and "ProvToolbox twins",
each naming the test that pins it. A row is therefore a claim the
test checks, that the file is rejected for the stated reason or reads
and differs from its twin, rather than a file the test does not look
at.

### Why test the packaged crate in CI?

`.github/workflows/ci.yml` runs `cargo package --workspace` after the
workspace tests and then `cargo test` with the manifest of the
packaged `provkit` under `target/package`. The commit "Ship a testable
crate and make the typos job pass" says why. The published crate had
excluded `tests/common/` while shipping `tests/provn_round_trip.rs`,
which declares `mod common;`, so `cargo test` on the tarball failed to
compile. `tests/common/` now ships, because it touches the corpus only
at run time, and CI runs the tests of the packaged copy so that what
ships still builds and passes. The comment on the `exclude` line in
`provkit/Cargo.toml` records the same rule.

## The Rust here

- **`#[test]` and `assert!` in test code**, first used in
  `provkit/tests/corpus_json.rs` at `reads_all_398`. `#[test]` marks a
  function the test harness runs; `assert!` panics when its condition
  is false and `assert_eq!` when its two sides differ, and both take
  a format message after the operands, as `Report::finish` and
  `round_trip` in `test_util` do, where the doctests of earlier stops
  called them without one.
  [How to write tests](https://doc.rust-lang.org/book/ch11-01-writing-tests.html)
- **`env!`**, first used in `provkit/tests/common/mod.rs` at
  `data_path`. It reads an environment variable at compile time and
  fails the build when the variable is unset. Cargo sets
  `CARGO_MANIFEST_DIR` for every crate, and for an integration test
  `CARGO_TARGET_TMPDIR` and, when the package has a binary,
  `CARGO_BIN_EXE_<name>`.
  [env!](https://doc.rust-lang.org/std/macro.env.html),
  [Environment variables Cargo sets for crates](https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates)
- **`impl FnOnce` arguments**, first used in
  `provkit/tests/common/mod.rs` at `Report::round_trip`. Stop 2 put
  `impl Into<String>` in argument position for a conversion; here the
  bound is a closure trait, `FnOnce` because each is called once, and
  a plain function such as `json::to_string` satisfies it as well as a
  closure.
  [The Fn traits](https://doc.rust-lang.org/book/ch13-01-closures.html#moving-captured-values-out-of-closures-and-the-fn-traits)
- **A test double implementing `std::io::Write`**, first used in
  `provkit/src/test_util.rs` at `Broken`. `write` and `flush` are the
  two required methods, and a type that fails in `write` exercises the
  error path of every writer that takes `impl Write`.
  [Write](https://doc.rust-lang.org/std/io/trait.Write.html)
- **`std::io::Error::other`**, first used in
  `provkit/src/test_util.rs` at `Broken::write`. It wraps any error or
  message in an `io::Error` of kind `Other`, so a test double needs no
  error type of its own.
  [Error::other](https://doc.rust-lang.org/std/io/struct.Error.html#method.other)
- **`unwrap` and `panic!` in test code**, first used in
  `provkit/src/test_util.rs` at `ex` and `syntax`. The library returns
  errors; a test calls `unwrap` where failure means the test is wrong
  and `panic!` with the unexpected value in the message, so the
  failure names what it got.
  [To panic! or not to panic!](https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html)
- **`#![allow(dead_code)]` on a shared test module**, first used in
  `provkit/tests/common/mod.rs` at the top of the file. The `#!` form
  is an inner attribute and applies to the whole module, where stop
  2's `#[allow(..)]` applied to one item, and the comment beside it
  gives the reason.
  [Attributes](https://doc.rust-lang.org/reference/attributes.html)
- **`assert_cmd::Command::cargo_bin` and `predicates`**, first used in
  `provkit-cli/tests/cli.rs` at `provkit`. `cargo_bin` finds the
  binary Cargo built for the test, `assert()` runs it and returns an
  `Assert`, and `predicates` supplies the matchers chained on standard
  output and error.
  [assert_cmd](https://docs.rs/assert_cmd/latest/assert_cmd/),
  [predicates](https://docs.rs/predicates/latest/predicates/)
- **`impl AsRef<OsStr>` arguments**, first used in
  `provkit-cli/tests/cli.rs` at `convert`. `AsRef<OsStr>` is the bound
  `Command::arg` takes, so a helper with the same bound accepts a
  `&str`, a `PathBuf` or a reference to one and passes it through.
  [AsRef](https://doc.rust-lang.org/std/convert/trait.AsRef.html)

## Run this

```sh
cargo test -p provkit --test corpus_json reads_all_398
```

This runs the one test over every file of `corpus/json` and reports a
failure for any file `json::read` refuses, with the count checked
against `JSON_CORPUS_SIZE`.

```text
test result: ok. 1 passed
```

```sh
PROVKIT_ONLY=entity1 cargo test -p provkit --test corpus_twins
```

This runs both tests of `corpus_twins.rs` on the files whose name
contains `entity1`, which are `entity1` and `entity10` in each format.
The twins test reads each PROV-N file and compares it with its JSON
twin, and `entity10.provn` is a `DynamicTimestamp` row, so it must
read and must differ. The round-trip test writes the two JSON files
as PROV-N and reads them back. The count check is skipped because the
filter is set.

```text
test result: ok. 2 passed
```

## Try it

Pick a row of `provkit/tests/data/known_differences.tsv` and read its
category. Run `corpus_twins` on that file alone and explain why it
passes although the twin differs. `PROVKIT_ONLY` matches a substring,
so pick a stem that names one file, or add `-- --nocapture` to see the
test's own report of which twins were equal and unequal.

```sh
PROVKIT_ONLY=<stem> cargo test -p provkit --test corpus_twins
```

<details><summary>Answer</summary>

The row `attr_activity0.provn` is a `DatatypePalette` difference.
`corpus/provn/README.md` says ProvToolbox's generator does not cover
three XSD datatypes that the JSON fixture carries, so the two files
hold different attribute lists.

```sh
PROVKIT_ONLY=attr_activity0 cargo test -p provkit --test corpus_twins
```

```text
test result: ok. 2 passed
```

The test passes because of the match in
`provtoolbox_corpus_equals_json_twins`. The file reads under
`Profile::Default`, so the outcome is `Ok(false)`, and the row makes
`known_as` a `Some`, so that arm pushes the file into `known` instead
of failing. The later assertion checks that no `DatatypePalette` twin
read as equal, which holds. With `-- --nocapture` the test's
`eprintln!` shows the file under its category.

```text
DatatypePalette: 0 equal, 1 unequal
  equal:
  unequal: attr_activity0.provn
```

Delete the row and the same run fails with "differs from its
PROV-JSON twin", since the arm for `(Ok(false), None)` records a
failure. Keep the row and make the two files equal, and the run fails
the other way, with "twins now read as equal".

</details>

## Idioms first seen here

- `#[test] and assert! in test code`: https://doc.rust-lang.org/book/ch11-01-writing-tests.html
- `env!`: https://doc.rust-lang.org/std/macro.env.html
- `impl FnOnce arguments`: https://doc.rust-lang.org/book/ch13-01-closures.html#moving-captured-values-out-of-closures-and-the-fn-traits
- `a test double implementing std::io::Write`: https://doc.rust-lang.org/std/io/trait.Write.html
- `std::io::Error::other`: https://doc.rust-lang.org/std/io/struct.Error.html#method.other
- `unwrap and panic! in test code`: https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html
- `#![allow(dead_code)] on a shared test module`: https://doc.rust-lang.org/reference/attributes.html
- `assert_cmd::Command::cargo_bin and predicates`: https://docs.rs/assert_cmd/latest/assert_cmd/
- `impl AsRef<OsStr> arguments`: https://doc.rust-lang.org/std/convert/trait.AsRef.html
