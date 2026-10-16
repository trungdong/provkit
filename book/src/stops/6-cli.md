# Stop 6: The command line

The `provkit-cli` crate is the binary `provkit`, and it owns everything
the library refuses to. The repository's `CLAUDE.md`, the short guide
a contributor reads first, draws the line. The library reads `&str`
and writes to `impl std::io::Write` and never touches a file, and the
CLI owns files, format detection and exit codes. This stop
reads the clap definitions that turn two structs and an enum into the
commands and their help, the function both
commands read their input through, the sniffer that tells PROV-JSON
from PROV-N by looking at the text, the `main` that turns a `Result`
into an exit code, and `compare`, which prints what one document has
and the other lacks. It builds on stop 4 for `json::read` and
`json::write`, on stop 5 for `provn::read_report`, `provn::write`,
`provn::statement` and `provn::name`, on stop 3 for the set-based
equality `compare` starts from and for `merged_bundles` and
`unified`, and on stop 1 for the namespace table each side of a
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
compare --help` prints the whole comment. The text on `Convert`
states the order in which `-f`, `-t`, the extension and the content
decide a format, and the text on `Compare` says what the listing is,
how a name with no declared prefix is written, and what the three
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
column, and passes any other error through with the source name in
front. `message_of` cuts the ` at line L column C` a `JsonError`
message ends with, so the position is stated once.

```rust,ignore
{{#include ../../../provkit-cli/src/main.rs:read_document}}
```

Both commands read through `read_document`. It reads the whole text
first, standard input through `io::read_to_string` or the file through
`fs::read_to_string`, and `map_err` turns a failure into one message
naming the input. The message gains a hint when the error's kind is
`InvalidData` and `starts_with_utf16_mark`, below the span, finds a
UTF-16 byte order mark; that function opens the file and reads two
bytes only after the UTF-8 read has failed, as its doc says. The
format is then resolved in the order the help states. `from` wins,
then `Format::from_path`, then `Format::sniff`, and `or_else` makes
each step lazy, so the text is sniffed only when the flag and the
extension decide nothing. `-` has no extension, so standard input goes
straight to sniffing, as the comment says. When all three give `None`
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
file, as its comment says, and then writes the bytes through
`write_stdout` or `fs::write`. `write_stdout` locks standard output,
writes and flushes, and treats a broken pipe as a normal end, so
`provkit convert big.json | head` exits quietly.

### provkit-cli/src/format.rs

`Format` is `Json` or `Provn`. It derives clap's `ValueEnum`, which
is what `value_enum` on the flags needs, and the doc on each variant
is the text clap prints beside the possible values. `WS` is an
associated constant holding the grammar's four whitespace characters,
space, tab, carriage return and line feed, and its doc says no other
Unicode space separates PROV-N tokens. `from_path` reads the
extension, in any letter case through `eq_ignore_ascii_case`, and
returns `None` for any other extension and for `-`.

```rust,ignore
{{#include ../../../provkit-cli/src/format.rs:sniff}}
```

`sniff` first drops a leading byte order mark, because both readers
drop one, as the comment says. The loop then skips what the grammar
calls trivia. It trims the four whitespace characters, and when `//`
follows it moves past the end of the line, when `/*` follows it moves
past the next `*/`, and otherwise it stops. `split_once` returns
`None` for an unterminated comment, and `map_or("", ..)` leaves
nothing to look at, so a text that is only a comment sniffs as
neither format. The decision is made on what is left. `{` is
PROV-JSON, and `document` is PROV-N when nothing, a whitespace
character or the start of a comment follows it, so `documentary` and
`document(` are neither. The second comment names `skip_trivia` in
the library's lexer from stop 5, which is private to the library, so
this loop mirrors it rather than calling it.

### provkit-cli/src/compare.rs

The module doc says what the command prints. `CompareArgs`, above
the span, holds the two positional documents, `-f` for the format of
both, and `--unified`, a `bool` flag that is true when present.

```rust,ignore
{{#include ../../../provkit-cli/src/compare.rs:compare_run}}
```

`run` refuses `-` on both sides through `bail!`, because standard
input can be read once, and reads each side through `read_document`
with the one `-f`. With `--unified` each document is replaced by its
[`unified`](../api/provkit/struct.Document.html#method.unified) form
from stop 3, and `with_context` puts the file name in front of a
unification failure. Then `a == b` is the set-based equality of stop 3,
so two documents in different formats, with different namespace
tables and different record order, compare equal, and the command
returns `ExitCode::SUCCESS` with nothing printed. Otherwise it builds
the whole report in a `String`, one `report_side` call per side with
the sides swapped, and writes it once, so a failure never leaves half
of it printed, as the comment says, and returns 1.

```rust,ignore
{{#include ../../../provkit-cli/src/compare.rs:only_in}}
```

`only_in` collects `theirs` into a `HashSet<&Record>`, borrowing the
records as stop 3's equality did, and keeps each record of `mine`
that the set lacks. The second test in the `filter` is
`seen.insert(*r)`, which returns `false` for a record already
inserted, so a record repeated on one side is listed once. The
lifetime `'a` ties the returned references to `mine` alone, so
`theirs` may be a shorter-lived slice. `report_side`, below the span,
lists the top-level records first and then each bundle under a
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
prefix minted for a line that fails is not kept. When the failure is
[`Error::Unwritable`](../api/provkit/enum.Error.html#variant.Unwritable)
it builds the heading the limit row below describes, the keyword, the
identifier and the offending name as `<IRI>`, ending in `// not valid
PROV-N`, and any other error is returned.

## Why this shape

### Why does `compare` exit 0, 1 or 2?

The doc on `Compare` in the first span states the three codes, 0 when
the documents are equal with nothing printed, 1 when they differ and 2
on any error, and the README's exit code table says the same with the
errors spelled out. The test `compare_help_names_the_exit_codes`
checks that `compare --help` names all three, and
`compare_read_error_exits_two` checks that a missing file exits 2
with nothing on standard output and the file's name on standard
error. The row of
[CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md)
beginning "`provconvert -compare` exits 6 on a difference" sets the
three codes against ProvToolbox's 6 for a difference and 1 for a read
failure, and leaves its Note column empty. The commit "Add
provkit compare" says that `Result<ExitCode>` lets one function
report success, a difference and an error apart, which is the shape
of `run`, and the limit row quoted two cards below gives the reason
for keeping 1 and 2 apart, that a difference is still a difference
and not a failure. No tracked source says why these three values
were chosen.

### Why `anyhow` in the binary and `thiserror` in the library?

The library's [`Error`](../api/provkit/enum.Error.html) is an enum
derived with `thiserror`, as stop 3 showed, and its doc example
matches a variant by name with `..` for the fields it does not need.
`located` does the same with three variants, because the binary wants
the line and column out of a `Syntax` or `At` and the position out of
a `Json`, and passes every other variant through. Past that one
function the binary never gives an error to a caller, because `main`
prints it and exits, so the error only has to display well.
`anyhow::Error` holds any error with a chain of contexts,
`with_context` adds the file name in front of a library error in
`convert` and `run`, `anyhow!` makes an error from a message, and
`{:#}` prints the chain on one line. The commit "Add provkit convert
with pandoc-style format resolution" notes that `Context` wraps an
error with the file name and that `{e:#}` prints the whole chain, and
the dependency arrived with the workspace scaffold. Neither commit
states a reason for using one crate in the library and the other in
the binary; the code shows the division and the sources describe what
each crate does.

### Why can `compare` print a record PROV-N cannot write?

The limit row of
[CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md)
beginning "`provkit compare` prints a one-sided record or bundle that
PROV-N cannot write" says such a record, one whose namespace IRI
holds a character `IRI_REF` forbids, is printed as a heading made of
the keyword, the identifier and the first unwritable name as
`<full IRI>`, ending in `// not valid PROV-N`, with a name that
belongs to an attribute inside `[...]`, that a bundle heading reads
`bundle <IRI>: // not valid PROV-N`, and that the exit code stays 1.
The reason it gives is that PROV-JSON can hold such an IRI, and a
difference in such a record is still a difference and not a failure
with exit code 2. The row also says `provn::statement` still returns
the error, so the library's contract from stop 5 is unchanged and the
heading is the binary's own rendering, which `statement_line` builds
from the name inside `Error::Unwritable`. The test
`compare_marks_records_prov_n_cannot_write` pins each form.

### Why is the listing not a PROV-N document?

The doc on `Compare` says the listing is not a PROV-N document, and
that its names resolve with the declarations of the file they came
from. The commit "Add provkit compare" says every side renders with
its own document's namespaces, and the commit "Make compare output
resolvable and fix the CLI messages" says every line of one file, and
of one bundle, renders against one namespace table, so a minted `nsN`
prefix means one IRI in its scope, and that a `prefix nsN <IRI>` line
is printed for each minted prefix after the records of that scope.
The README adds that such a line applies to the whole scope, the
records printed before it included. The two sides may bind one prefix
to two IRIs, and each side is printed as its own file would write it,
so a reader resolves the lines under one heading with that file's
declarations and the `prefix` lines beneath them. The tests
`compare_bundle_difference_uses_document_prefixes` and
`compare_reports_one_sided_records` pin the shape.

## The Rust here

- **clap's derive API**, first used in `provkit-cli/src/main.rs` at
  `Cli`. `#[derive(Parser)]` on the top struct, `Subcommand` on the
  enum, `Args` on each argument struct and `ValueEnum` on `Format`
  generate the parser and its help; `#[command]` sets command
  attributes and `#[arg]` names the flag letters; a doc comment
  becomes the help text, the first paragraph for `-h` and all of it
  for `--help`.
  [clap derive reference](https://docs.rs/clap/latest/clap/_derive/index.html),
  [Doc comments](https://docs.rs/clap/latest/clap/_derive/index.html#doc-comments)
- **`anyhow::Result`, `with_context`, `anyhow!` and `bail!`**, first
  used in `provkit-cli/src/main.rs` at `read_document`. `Result<T>`
  here is `Result<T, anyhow::Error>`, a type that holds any error;
  `with_context` wraps an error with a message built only on failure,
  `anyhow!` makes an error from a format string, and `bail!` in
  `compare::run` returns one at once.
  [anyhow](https://docs.rs/anyhow/latest/anyhow/),
  [Context](https://docs.rs/anyhow/latest/anyhow/trait.Context.html),
  [bail!](https://docs.rs/anyhow/latest/anyhow/macro.bail.html)
- **`{:#}` on an `anyhow::Error`**, first used in
  `provkit-cli/src/main.rs` at `main`. The alternate display prints
  the error and every context around it on one line, joined by `: `,
  where `{}` prints the outermost context alone.
  [Display representations](https://docs.rs/anyhow/latest/anyhow/struct.Error.html#display-representations)
- **`ExitCode` from `main`**, first used in `provkit-cli/src/main.rs`
  at `main`. Returning `std::process::ExitCode` sets the status
  without calling `process::exit`, so destructors run; `SUCCESS` and
  `from(2)` are the values used here.
  [ExitCode](https://doc.rust-lang.org/std/process/struct.ExitCode.html)
- **`eprintln!`**, first used in `provkit-cli/src/main.rs` at `main`.
  It writes to standard error, so a message never mixes with the
  PROV-N or PROV-JSON on standard output.
  [eprintln!](https://doc.rust-lang.org/std/macro.eprintln.html)
- **`io::read_to_string`**, first used in `provkit-cli/src/main.rs` at
  `read_document`. A free function that reads any `Read` to the end
  into a `String`, so standard input and `fs::read_to_string` give the
  same type to one `map_err`.
  [io::read_to_string](https://doc.rust-lang.org/std/io/fn.read_to_string.html)
- **`io::ErrorKind`**, first used in `provkit-cli/src/main.rs` at
  `read_document`. `e.kind()` classifies an I/O error, `InvalidData`
  being the kind `fs::read_to_string` returns for text that is not
  UTF-8; `write_stdout` matches `BrokenPipe` with a `match` guard, as
  stop 5's lexer matched on a condition.
  [ErrorKind](https://doc.rust-lang.org/std/io/enum.ErrorKind.html)
- **`Result::is_ok_and`**, first used in `provkit-cli/src/main.rs` at
  `starts_with_utf16_mark`. The `Result` twin of stop 1's
  `is_some_and`; the closure runs on the `Ok` value and an `Err` is
  `false`, so an unreadable file is simply not a UTF-16 one. The
  `use std::io::Read as _` at the top of the file brings `read_exact`
  into scope without a name, as stop 2's `fmt::Write as _` did.
  [Result::is_ok_and](https://doc.rust-lang.org/std/result/enum.Result.html#method.is_ok_and)
- **`Option::as_deref` and `Option::filter`**, first used in
  `provkit-cli/src/main.rs` at `convert`. `as_deref` turns
  `&Option<PathBuf>` into `Option<&Path>`, the `Option` twin of stop
  5's `Result::as_deref`, and `filter` drops the value when the
  closure says no, so `-o -` becomes `None` and means standard output.
  [Option::as_deref](https://doc.rust-lang.org/std/option/enum.Option.html#method.as_deref),
  [Option::filter](https://doc.rust-lang.org/std/option/enum.Option.html#method.filter)
- **Destructuring an error variant with `..`**, first used in
  `provkit-cli/src/main.rs` at `located`. `Error::Syntax { line,
  column, message, .. }` binds three fields by name and ignores the
  rest, which a `#[non_exhaustive]` variant from stop 3 requires of
  code outside its crate.
  [Destructuring enums](https://doc.rust-lang.org/book/ch19-03-pattern-syntax.html#destructuring-enums),
  [Ignoring remaining parts with `..`](https://doc.rust-lang.org/book/ch19-03-pattern-syntax.html#ignoring-remaining-parts-of-a-value-with-)
- **A `[char; N]` array as a string pattern**, first used in
  `provkit-cli/src/format.rs` at `Format::WS`. An array of `char`
  implements `Pattern`, so `trim_start_matches(Self::WS)` and
  `starts_with(Self::WS)` match any one of the four; the constant
  itself is an associated constant as stop 3 introduced them.
  [Pattern](https://doc.rust-lang.org/std/str/pattern/trait.Pattern.html)
- **`eq_ignore_ascii_case`, `strip_prefix`, `trim_start_matches` and
  `split_once`**, first used in `provkit-cli/src/format.rs` at
  `from_path` and `sniff`. `eq_ignore_ascii_case` compares without
  making a lower-cased copy, `trim_start_matches` drops every leading
  match, and `strip_prefix` and `split_once` return `None` when the
  pattern is absent, so the sniffer's fallbacks are `unwrap_or` and
  `map_or` rather than `?`.
  [str methods](https://doc.rust-lang.org/std/primitive.str.html#method.strip_prefix)
- **`HashSet::insert` inside a `filter`**, first used in
  `provkit-cli/src/compare.rs` at `only_in`. `insert` returns whether
  the value was new, so `seen.insert(*r)` both remembers a record and
  decides whether to keep it, and `collect` gathers the survivors into
  a `Vec`; the set borrows its records as stop 3's did.
  [HashSet::insert](https://doc.rust-lang.org/std/collections/struct.HashSet.html#method.insert),
  [Processing items with iterators](https://doc.rust-lang.org/book/ch13-02-iterators.html)
- **A lifetime parameter on a function**, first used in
  `provkit-cli/src/compare.rs` at `only_in`. Stops 1 and 5 put a
  lifetime on a struct and on a method; `only_in<'a>` names one on a
  free function so the returned `Vec<&'a Record>` is tied to `mine`
  and not to `theirs`, which elision could not express with two
  reference inputs.
  [Lifetime annotations in function signatures](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#lifetime-annotations-in-function-signatures)
- **Clone, try, assign back**, first used in
  `provkit-cli/src/compare.rs` at `statement_line`, below the span.
  The namespace table is cloned, the writer runs against the clone,
  and `*namespaces = trial` commits it only on success, so a failed
  line leaves the table as it was.
  [Clone](https://doc.rust-lang.org/std/clone/trait.Clone.html)

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
second run are twins from the corpus, one entity `ex:e1` in each
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
`a == b` holds because stop 3's equality compares the records as a
set and ignores the format and the namespace tables.

```text
0
```

The third run exits 2 with one line on standard error. The message
names the undeclared prefix, not the missing `)`, and points at line
2, column 10, the start of `ex:e`. Stop 5's parser resolves a name
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
- `eprintln!`: https://doc.rust-lang.org/std/macro.eprintln.html
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
