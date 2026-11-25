# Chapter 7: The command line

The `provkit-cli` crate is the binary `provkit`, and it owns everything
the library refuses to. The library reads `&str` and writes to
`impl std::io::Write` and never touches a file, and the CLI owns
files, format detection and exit codes. This chapter
reads the clap definitions that turn three structs and two enums into the
commands and their help, the function both
commands read their input through, the sniffer that tells PROV-JSON
from PROV-N by looking at the text, the `main` that turns a `Result`
into an exit code, and `compare`, which prints what one [document](../reference/glossary.md#document-prov) has
and the other lacks. It builds on chapter 5 for `json::read` and
`json::write`, on chapter 6 for `provn::read_report`, `provn::write`,
`provn::statement` and `provn::name`, on chapter 4 for the set-based
equality `compare` starts from and for `merged_bundles` and
`unified`, and on chapter 2 for the [namespace](../reference/glossary.md#namespace-and-prefix-prov) table each side of a
comparison renders against.

## What the code does

### provkit-cli/src/main.rs

The file declares two private modules, `compare` and `format`, and
holds the argument definitions, `main`, the shared reader and the
`convert` command. Above the span, `Cli` derives clap's `Parser` and
carries `#[command(name = "provkit", version, about)]`. `version`
with no value takes the crate's version and `about` with no value
takes the crate's description from `Cargo.toml`, so `provkit --help`
opens with "Command-line tool to convert and compare W3C PROV
documents". Its one field is the subcommand.

```rust,ignore
{{#include ../../../provkit-cli/src/main.rs:command}}
```

The derive of `Subcommand` makes `convert` and `compare` from the two
variants, lower-cased, and each variant carries the struct of its
arguments. The doc comment on a variant is its help. `provkit compare
-h` prints the first paragraph, without its full stop, and `provkit
compare --help` prints the whole comment. The help on `Convert`
gives the order in which `-f`, `-t`, the extension and the content
decide a format, and the help on `Compare` says what the listing is,
how a name with no declared [prefix](../reference/glossary.md#namespace-and-prefix-prov) is written, and what the three
exit codes mean. `ConvertArgs`, below the span, derives `Args`. Its
positional `input` is a `PathBuf`, and `#[arg(short, long)]` makes
`-o` and `--output` from the field name `output`. The two format
flags name their letters, `-f` for `--from` and `-t` for `--to`, and
are `Option<Format>` with `value_enum`, so clap lists `json` and
`provn` as the possible values and refuses any other spelling.
`CompareArgs` lives in `compare.rs`.

```rust,ignore
{{#include ../../../provkit-cli/src/main.rs:main}}
```

`main` returns `ExitCode`. `Cli::parse()` reads the process
arguments, and on a usage error or `--help` clap prints its own
message and exits before the function continues. The first `match`
dispatches on the subcommand. `convert` returns `Result<()>`, and
`map(|()| ExitCode::SUCCESS)` turns its success into a code, while
`compare::run` returns `Result<ExitCode>` because it has two
successful outcomes. The second `match` is the whole error policy of
the binary. An `Ok` code is returned as it is, and an `Err` is printed
on standard error as `provkit: ` followed by the error in its `{:#}`
form, the alternate display of an `anyhow::Error`, which writes the
error and every context wrapped around it on one line separated by
`: `, and the process exits 2. Between this span and the next,
`shown` names a path in a message, with `<stdin>` for `-`, and
`located` turns a library
[`Error`](../api/provkit/enum.Error.html) into an `anyhow::Error`
placed at `source:line:column: message` when the error has a
position. It destructures `Error::Syntax` and `Error::At` with `..`
for the fields it does not use, asks a
[`JsonError`](../api/provkit/struct.JsonError.html) for its line and
column, and gives every other error to `structure_position` before it
falls back to the source name in front of the error. A PROV-JSON
structure error carries its position at the end of its message, as
` at line L column C`, so `structure_position` walks down through any
number of `Error::InRecord` to the innermost error, and when that is an
`Error::JsonStructure` it moves the position to the front. The text
that names each record stays in front of the message and the
`PROV-JSON: ` label goes, so a doubly nested error prints as
`` bad.json:1:107: bundle `ex:b`: entity `ex:e`: duplicate field `ex:a` ``.
`split_position` finds the position by splitting at the last
` at line `, because a value quoted in the message can hold those words,
and it accepts the ending only when both numbers are plain digits. An
error without that ending, such as one inside a record that has no
position, takes that fallback, label included. `message_of` cuts
the same ending from a `JsonError` message.

```rust,ignore
{{#include ../../../provkit-cli/src/main.rs:read_document}}
```

Both commands read through `read_document`. It reads the whole text
first, standard input through `io::read_to_string` or the file through
`fs::read_to_string`, and `map_err` turns a failure into one message
naming the input. The message gains a hint when the error's kind is
`InvalidData` and `starts_with_utf16_mark`, below the span, finds a
UTF-16 byte-order mark; that function opens the file and reads two
bytes only after the UTF-8 read has failed. The format is then
resolved in a fixed order. `from` wins,
then `Format::from_path`, then `Format::sniff`, and `or_else` makes
each step lazy, so the text is sniffed only when the flag and the
extension decide nothing. `-` has no extension, so standard input goes
straight to sniffing. When all three give `None`
the error names the flag to use. The read itself goes through
`json::read` or
[`provn::read_report`](../api/provkit/provn/fn.read_report.html) under
`Profile::Default`, with every library error passing through
`located`, and a PROV-N read also hands the report's `ignored` list to
`warn_ignored`, which prints one warning on standard error naming the
count and the first expression's name, line and column, with the exit
code unchanged. `convert`, at the end of the file, resolves the output
format from `-t`, then the extension of `-o`, then PROV-N when the
output is standard output, and refuses a file with any other
extension. It reads the document, serialises it into a `Vec<u8>`
before any file is touched, so a library error never leaves a partial
file, and then writes the bytes through
`write_stdout` or `fs::write`. `write_stdout` locks standard output,
writes and flushes, and treats a broken pipe as a normal end, so
`provkit convert big.json | head` exits quietly.

### provkit-cli/src/format.rs

`Format` is `Json` or `Provn`. It derives clap's `ValueEnum`, which
is what `value_enum` on the flags needs, and the doc on each variant
is the text clap prints beside the possible values. `WS` is an
associated constant holding the grammar's four whitespace characters,
space, tab, carriage return and line feed, and no other Unicode space
separates PROV-N tokens. `from_path` reads the
extension, in any letter case through `eq_ignore_ascii_case`, and
returns `None` for any other extension and for `-`.

```rust,ignore
{{#include ../../../provkit-cli/src/format.rs:sniff}}
```

`sniff` first drops a leading byte-order mark, because both readers
drop one. The loop then skips what the grammar
calls trivia. It trims the four whitespace characters, and when `//`
follows it moves past the end of the line, when `/*` follows it moves
past the next `*/`, and otherwise it stops. `split_once` returns
`None` for an unterminated comment, and `map_or("", ..)` leaves
nothing to look at, so a text that is only a comment sniffs as
neither format. The decision is made on what is left. `{` is
PROV-JSON, and `document` is PROV-N when nothing, a whitespace
character or the start of a comment follows it, so `documentary` and
`document(` are neither. `skip_trivia` in the library's lexer from
chapter 6 does the same job, but it is private to the library, so this
loop mirrors it rather than calling it.

### provkit-cli/src/compare.rs

`CompareArgs`, above the span, holds the two positional documents, `-f` for the format of
both, and `--unified`, a `bool` flag that is true when present.

```rust,ignore
{{#include ../../../provkit-cli/src/compare.rs:compare_run}}
```

`run` refuses `-` on both sides through `bail!`, because standard
input can be read once, and reads each side through `read_document`
with the one `-f`. With `--unified` each document is replaced by its
[`unified`](../api/provkit/struct.Document.html#method.unified) form
from chapter 4, and `with_context` puts the file name in front of a
[unification](../reference/glossary.md#unification-prov) failure. Then `a == b` is the set-based equality of chapter 4,
so two documents in different formats, with different namespace
tables and different [record](../reference/glossary.md#record-provkit) order, compare equal, and the command
returns `ExitCode::SUCCESS` with nothing printed. Otherwise it builds
the whole report in a `String`, one `report_side` call per side with
the sides swapped, and writes it once, so a failure never leaves half
of it printed, and returns 1.

```rust,ignore
{{#include ../../../provkit-cli/src/compare.rs:only_in}}
```

`only_in` collects `theirs` into a `HashSet<&Record>`, borrowing the
records as chapter 4's equality did, and keeps each record of `mine`
that the set lacks. The second test in the `filter` is
`seen.insert(*r)`, which returns `false` for a record already
inserted, so a record repeated on one side is listed once. The
lifetime `'a` ties the returned references to `mine` alone, so
`theirs` may be a shorter-lived slice. `report_side`, below the span,
lists the top-level records first and then each [bundle](../reference/glossary.md#bundle-prov) under a
`bundle <id>:` heading. Bundles that share an identifier are grouped
through
[`merged_bundles`](../api/provkit/struct.Document.html#method.merged_bundles),
as equality groups them, and a bundle the other side lacks is listed
even when empty. Each scope renders against one
[`Namespaces`](../api/provkit/struct.Namespaces.html) table, a clone
of the document's for the top level and the bundle's with the
document's as fallback for a bundle, and `declare_minted` prints a
`prefix` line for every binding the table gained while rendering,
after the records of that scope. `statement_line` tries
[`provn::statement`](../api/provkit/provn/fn.statement.html) against a
clone of the table and assigns the clone back on success, so a
prefix minted by the failed attempt is discarded. When the failure is
[`Error::Unwritable`](../api/provkit/enum.Error.html#variant.Unwritable)
it builds the heading that
[Why can `compare` print a record PROV-N cannot write?](#why-can-compare-print-a-record-prov-n-cannot-write)
below describes, the keyword, the
identifier and the offending name as `<IRI>`, ending in `// not valid
PROV-N`, and any other error is returned.

## Why this shape

### Why does `compare` exit 0, 1 or 2?

The three codes are 0 when the documents are equal with nothing
printed, 1 when they differ and 2 on any error. The test
`compare_help_names_the_exit_codes` checks that `compare --help` names
all three, and `compare_read_error_exits_two` checks that a missing
file exits 2 with nothing on standard output and the file's name on
standard error. ProvToolbox exits 6 for a difference and 1 for a read
failure
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"`provconvert -compare` exits 6 on a difference"). `Result<ExitCode>`
lets `run` report success, a difference and an error apart. A
difference in a record PROV-N cannot write is still a difference,
exit code 1, and not a failure with exit code 2.

### Why `anyhow` in the binary and `thiserror` in the library?

The library's [`Error`](../api/provkit/enum.Error.html) is an enum
derived with `thiserror`, as chapter 4 showed, and its doc example
matches a variant by name with `..` for the fields it does not need.
`located` destructures `Syntax` and `At` the same way, because the
binary wants their line and column, binds the `JsonError` inside
`Json` whole and asks it for its position, and hands every other
variant to `structure_position`. Past that one
function the binary never gives an error to a caller, because `main`
prints it and exits, so the error only has to display well.
`anyhow::Error` holds any error with a chain of contexts,
`with_context` adds the file name in front of a library error in
`convert` and `run`, `anyhow!` makes an error from a message, and
`{:#}` prints the chain on one line.

### Why can `compare` print a record PROV-N cannot write?

Such a record, one whose namespace IRI holds a character `IRI_REF`
forbids, is printed as a heading made of the keyword, the identifier
and the first unwritable name as `<full IRI>`, ending in `// not valid
PROV-N`
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"`provkit compare` prints a one-sided record or bundle that PROV-N
cannot write"). A name that belongs to an
[attribute](../reference/glossary.md#attribute-prov) is printed inside
`[...]`, and a bundle heading reads `bundle <IRI>: // not valid
PROV-N`. The exit code stays 1, because PROV-JSON can hold such an
IRI. `provn::statement` still returns the
error, so the library's contract from chapter 6 is unchanged and the
heading is the binary's own rendering, which `statement_line` builds
from the name inside `Error::Unwritable`. The test
`compare_marks_records_prov_n_cannot_write` pins each form.

### Why is the listing not a PROV-N document?

Every side renders with its
own document's namespaces, and every line of one file, and of one
bundle, renders against one namespace table, so a minted `nsN` prefix
means one IRI in its scope. A `prefix nsN <IRI>` line is printed for
each minted prefix after the records of that scope, and applies to
the whole scope, the records printed before it included. The two
sides may bind one prefix
to two IRIs, and each side is printed as its own file would write it,
so a reader resolves the lines under one heading with that file's
declarations and the `prefix` lines beneath them. The tests
`compare_bundle_difference_uses_document_prefixes` and
`compare_reports_one_sided_records` pin the shape.

## The Rust here

- **clap's derive API**. `#[derive(Parser)]` on the top struct,
  `Subcommand` on the enum, `Args` on each argument struct and
  `ValueEnum` on `Format` generate the parser and its help; `#[command]`
  sets command attributes and `#[arg]` names the flag letters; a doc
  comment becomes the help text, the first paragraph for `-h` and all of
  it for `--help`.
  [clap derive reference](https://docs.rs/clap/latest/clap/_derive/index.html),
  [Doc comments](https://docs.rs/clap/latest/clap/_derive/index.html#doc-comments)
- **`anyhow::Result`, `with_context`, `anyhow!` and `bail!`**.
  `Result<T>` here is `Result<T, anyhow::Error>`, a type that holds any
  error; `with_context` wraps an error with a message built only on
  failure, `anyhow!` makes an error from a format string, and `bail!` in
  `compare::run` returns one at once.
  [anyhow](https://docs.rs/anyhow/latest/anyhow/),
  [Context](https://docs.rs/anyhow/latest/anyhow/trait.Context.html),
  [bail!](https://docs.rs/anyhow/latest/anyhow/macro.bail.html)
- **`{:#}` on an `anyhow::Error`**. The alternate display, which `main`
  uses, prints the error and every context around it on one line,
  joined by `: `, where `{}` prints the outermost context alone.
  [Display representations](https://docs.rs/anyhow/latest/anyhow/struct.Error.html#display-representations)
- **`ExitCode` from `main`**. Returning `std::process::ExitCode` sets
  the status without calling `process::exit`, so destructors run;
  `SUCCESS` and `from(2)` are the values `main` uses.
  [ExitCode](https://doc.rust-lang.org/std/process/struct.ExitCode.html)
- **`io::stderr()`**. `main` prints an error and `warn_ignored` a
  warning on standard error, so a message never mixes with the PROV-N
  or PROV-JSON on standard output. Each writes with chapter 6's
  `let _ = writeln!(..)`, because `eprintln!` panics when standard error
  is a pipe whose reader has gone, and a message nobody reads should
  leave the exit code as it is.
  [io::stderr](https://doc.rust-lang.org/std/io/fn.stderr.html)
- **`io::read_to_string`**. A free function that reads any `Read` to the
  end into a `String`, so standard input and `fs::read_to_string` give
  the same type to one `map_err` in `read_document`.
  [io::read_to_string](https://doc.rust-lang.org/std/io/fn.read_to_string.html)
- **`io::ErrorKind`**. `e.kind()` classifies an I/O error, `InvalidData`
  being the kind `fs::read_to_string` returns for text that is not
  UTF-8; `write_stdout` matches `BrokenPipe` with a `match` guard, as
  chapter 6's lexer matched on a condition.
  [ErrorKind](https://doc.rust-lang.org/std/io/enum.ErrorKind.html)
- **`Result::is_ok_and`**. The `Result` twin of chapter 2's `is_some_and`;
  the closure runs on the `Ok` value and an `Err` is `false`, so in
  `starts_with_utf16_mark` an unreadable file is simply not a UTF-16
  one. The `use std::io::Read as
  _` at the top of the file brings `read_exact` into scope without a
  name, as chapter 3's `fmt::Write as _` did.
  [Result::is_ok_and](https://doc.rust-lang.org/std/result/enum.Result.html#method.is_ok_and)
- **`Option::as_deref` and `Option::filter`**. `as_deref` turns
  `&Option<PathBuf>` into `Option<&Path>`, the `Option` twin of chapter 6's
  `Result::as_deref`, and `filter` drops the value when the closure says
  no, so `-o -` in `convert` becomes `None` and means standard output.
  [Option::as_deref](https://doc.rust-lang.org/std/option/enum.Option.html#method.as_deref),
  [Option::filter](https://doc.rust-lang.org/std/option/enum.Option.html#method.filter)
- **Destructuring an error variant with `..`**. `Error::Syntax { line,
  column, message, .. }` in `located` binds three fields by name and
  ignores the rest, which a `#[non_exhaustive]` variant from chapter 4 requires of code
  outside its crate.
  [Destructuring enums](https://doc.rust-lang.org/book/ch19-03-pattern-syntax.html#destructuring-enums),
  [Ignoring remaining parts with `..`](https://doc.rust-lang.org/book/ch19-03-pattern-syntax.html#ignoring-remaining-parts-of-a-value-with-)
- **A `[char; N]` array as a string pattern**. An array of `char`
  implements `Pattern`, so `trim_start_matches(Self::WS)` and
  `starts_with(Self::WS)` match any one of the four; the constant itself
  is an associated constant as chapter 4 introduced them.
  [Pattern](https://doc.rust-lang.org/std/str/pattern/trait.Pattern.html)
- **`eq_ignore_ascii_case`, `strip_prefix`, `trim_start_matches` and
  `split_once`**. `eq_ignore_ascii_case` compares without making a
  lower-cased copy, `trim_start_matches` drops every leading match, and
  `strip_prefix` and `split_once` return `None` when the pattern is
  absent, so the fallbacks in `sniff` are `unwrap_or` and `map_or`
  rather than `?`, while `from_path` uses `?` on its two `Option` steps.
  [str methods](https://doc.rust-lang.org/std/primitive.str.html#method.strip_prefix)
- **`HashSet::insert` inside a `filter`**. `insert` returns whether the
  value was new, so `seen.insert(*r)` in `only_in` both remembers a
  record and decides whether to keep it, and `collect` gathers the survivors into a
  `Vec`; the set borrows its records as chapter 4's did.
  [HashSet::insert](https://doc.rust-lang.org/std/collections/struct.HashSet.html#method.insert),
  [Processing items with iterators](https://doc.rust-lang.org/book/ch13-02-iterators.html)
- **A lifetime parameter on a function**. Chapters 2 and 6 put a lifetime
  on a struct and on a method; `only_in<'a>` names one on a free
  function so the returned `Vec<&'a Record>` is tied to `mine` and not
  to `theirs`, which elision could not express with two reference
  inputs.
  [Lifetime annotations in function signatures](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#lifetime-annotations-in-function-signatures)
- **Clone, try, assign back**. In `statement_line` the namespace table
  is cloned, the writer runs against the clone, and `*namespaces =
  trial` commits it only on success, so a failed line leaves the table
  as it was.
  [Clone](https://doc.rust-lang.org/std/clone/trait.Clone.html)
- **`while let` over a chain of boxed errors**. In `structure_position`
  `while let Error::InRecord { error, .. } = innermost` repeats while
  the pattern matches, and each pass rebinds `innermost` to the
  `Box<Error>` inside, which a reference to `Error` accepts by
  deref coercion, so the loop stops at the first error that is not an
  `InRecord`.
  [while let](https://doc.rust-lang.org/book/ch19-01-all-the-places-for-patterns.html#while-let-conditional-loops)

## Run this

```sh
cargo test -p provkit-cli compare_reports_one_sided_records
```

This runs the built binary on the two corpus files
`scruffy-generation1-S.json` and `scruffy-generation1-M.json` and
checks the exit code 1, a listing of `entity(ex:e1)` and
`activity(ex:a1)` under the heading for the second file on standard
output, and nothing on standard error. The crate's unit-test binary
reports zero and the `cli` integration binary reports the one.

```text
test result: ok. 1 passed
```

```sh
cargo test -p provkit-cli a_provn_error_is_printed_as_file_line_column
```

This writes a PROV-N file with a syntax error and one with an
undeclared prefix, converts each, and checks that standard error holds
one line of the form `provkit: <file>:<line>:<column>: <message>`
with exit code 2, and that the undeclared-prefix text read from
standard input names `<stdin>`.

```text
test result: ok. 1 passed
```

```sh
cargo test -p provkit-cli compare_help_names_the_exit_codes
```

This runs `compare --help` and checks that the text names all three
exit codes.

```text
test result: ok. 1 passed
```

## Try it

Predict the exit code of each run, and for the third run which fault
the message names and where it points. The page shows the installed
name `provkit`; in a checkout, `cargo run -q -p provkit-cli --`
followed by the same arguments does the same. The two files of the
second run are twins from the corpus, one [entity](../reference/glossary.md#entity-prov) `ex:e1` in each
format, and the third run writes a file with two faults, an
undeclared prefix and a missing `)`.

```sh
provkit convert corpus/json/entity1.json; echo $?
provkit compare corpus/json/entity1.json corpus/provn/provtoolbox-corpus/entity1.provn; echo $?
printf 'document\n  entity(ex:e\nendDocument\n' > /tmp/bad.provn; provkit convert /tmp/bad.provn; echo $?
```

<details><summary>Answer</summary>

The first run exits 0 and prints the document as PROV-N on standard
output, because `-t` and `-o` are absent and standard output defaults
to PROV-N.

```text
document
  prefix ex <http://example.org/>

  entity(ex:e1)
endDocument
0
```

The second run exits 0 and prints nothing. `read_document` reads one
file as PROV-JSON by its extension and the other as PROV-N, and
`a == b` holds because chapter 4's equality compares the records as a
set and ignores the format and the namespace tables.

```text
0
```

The third run exits 2 with one line on standard error. The message
names the undeclared prefix, not the missing `)`, and points at line
2, column 10, the start of `ex:e`. Chapter 6's parser resolves a name
as soon as it reads it, in `identifier_and_first_argument`, so
`resolve` returns `Error::UndeclaredPrefix` wrapped in `Error::At`
with that token's position before `remaining_arguments` reaches the
end of the line, and `located` prints the position in front of the
inner error's message.

```text
provkit: /tmp/bad.provn:2:10: undeclared prefix `ex`
2
```

</details>

## Idioms first seen here

- `clap's derive API`: https://docs.rs/clap/latest/clap/_derive/index.html
- `anyhow::Result, with_context, anyhow! and bail!`: https://docs.rs/anyhow/latest/anyhow/
- `{:#} on an anyhow::Error`: https://docs.rs/anyhow/latest/anyhow/struct.Error.html#display-representations
- `ExitCode from main`: https://doc.rust-lang.org/std/process/struct.ExitCode.html
- `io::stderr()`: https://doc.rust-lang.org/std/io/fn.stderr.html
- `io::read_to_string`: https://doc.rust-lang.org/std/io/fn.read_to_string.html
- `io::ErrorKind`: https://doc.rust-lang.org/std/io/enum.ErrorKind.html
- `Result::is_ok_and`: https://doc.rust-lang.org/std/result/enum.Result.html#method.is_ok_and
- `Option::as_deref and Option::filter`: https://doc.rust-lang.org/std/option/enum.Option.html#method.as_deref
- `destructuring an error variant with ..`: https://doc.rust-lang.org/book/ch19-03-pattern-syntax.html#destructuring-enums
- `a [char; N] array as a string pattern`: https://doc.rust-lang.org/std/str/pattern/trait.Pattern.html
- `eq_ignore_ascii_case, strip_prefix, trim_start_matches and split_once`: https://doc.rust-lang.org/std/primitive.str.html#method.strip_prefix
- `HashSet::insert inside a filter`: https://doc.rust-lang.org/std/collections/struct.HashSet.html#method.insert
- `a lifetime parameter on a function`: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#lifetime-annotations-in-function-signatures
- `clone, try, assign back`: https://doc.rust-lang.org/std/clone/trait.Clone.html
- `while let over a chain of boxed errors`: https://doc.rust-lang.org/book/ch19-01-all-the-places-for-patterns.html#while-let-conditional-loops
