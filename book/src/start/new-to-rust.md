# New to Rust

Read the chapters in order. Each Rust idiom is named the first time it
appears, with a link to the Rust book, the reference, the standard
library or the crate that documents it. The table lists the idioms in
that order, so you can see what each chapter asks of you before you start
it. Three chapters of the Rust book cover most of what this book's chapters ask:
ownership (chapter 4), enums and pattern matching (chapter 6), and
traits, generics and lifetimes (chapter 10). The generated
[idiom index](../reference/idioms.md) lists every idiom any commit
introduced, grouped by the document it links to. The
[glossary](../reference/glossary.md) explains the words that Rust and
PROV use differently.

| Idiom | First seen | Read |
|---|---|---|
| `modules and visibility` | [Chapter 1](../chapters/1-orientation.md) | [Rust book](https://doc.rust-lang.org/book/ch07-02-defining-modules-to-control-scope-and-privacy.html) |
| `pub use` | [Chapter 1](../chapters/1-orientation.md) | [Rust book](https://doc.rust-lang.org/book/ch07-04-bringing-paths-into-scope-with-the-use-keyword.html) |
| `#[cfg(feature)]` | [Chapter 1](../chapters/1-orientation.md) | [Rust reference](https://doc.rust-lang.org/reference/conditional-compilation.html) |
| `doc comments and doctests` | [Chapter 1](../chapters/1-orientation.md) | [rustdoc book](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html) |
| `include_str!` | [Chapter 1](../chapters/1-orientation.md) | [std](https://doc.rust-lang.org/std/macro.include_str.html) |
| `Cargo workspace and features` | [Chapter 1](../chapters/1-orientation.md) | [Cargo book](https://doc.rust-lang.org/cargo/reference/workspaces.html) |
| `ownership and borrowing` | [Chapter 2](../chapters/2-names.md) | [Rust book](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html) |
| `Arc<str>` | [Chapter 2](../chapters/2-names.md) | [std](https://doc.rust-lang.org/std/sync/struct.Arc.html) |
| `Box<str>` | [Chapter 2](../chapters/2-names.md) | [std](https://doc.rust-lang.org/std/boxed/struct.Box.html) |
| `#[derive(Clone)]` | [Chapter 2](../chapters/2-names.md) | [Rust book](https://doc.rust-lang.org/book/appendix-03-derivable-traits.html) |
| `hand-written PartialEq, Eq, Ord, PartialOrd and Hash` | [Chapter 2](../chapters/2-names.md) | [std](https://doc.rust-lang.org/std/hash/trait.Hash.html) |
| `impl Iterator + '_ and lifetime elision` | [Chapter 2](../chapters/2-names.md) | [Rust reference](https://doc.rust-lang.org/reference/lifetime-elision.html) |
| `iterator adaptors all, eq and cmp` | [Chapter 2](../chapters/2-names.md) | [std](https://doc.rust-lang.org/std/iter/trait.Iterator.html) |
| `matches!` | [Chapter 2](../chapters/2-names.md) | [std](https://doc.rust-lang.org/std/macro.matches.html) |
| `? on Option and Result` | [Chapter 2](../chapters/2-names.md) | [Rust book](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html) |
| `then_some and is_some_and` | [Chapter 2](../chapters/2-names.md) | [std](https://doc.rust-lang.org/std/option/enum.Option.html) |
| `LazyLock` | [Chapter 2](../chapters/2-names.md) | [std](https://doc.rust-lang.org/std/sync/struct.LazyLock.html) |
| `macro_rules! with $(...)* repetition` | [Chapter 2](../chapters/2-names.md) | [Rust reference](https://doc.rust-lang.org/reference/macros-by-example.html) |
| `#[doc = concat!(...)]` | [Chapter 2](../chapters/2-names.md) | [rustdoc book](https://doc.rust-lang.org/rustdoc/write-documentation/the-doc-attribute.html) |
| `IndexMap` | [Chapter 2](../chapters/2-names.md) | [docs.rs](https://docs.rs/indexmap) |
| `let ... else` | [Chapter 2](../chapters/2-names.md) | [Rust reference](https://doc.rust-lang.org/reference/statements.html#let-statements) |
| `entry().or_default() and partition_point` | [Chapter 2](../chapters/2-names.md) | [std](https://doc.rust-lang.org/std/primitive.slice.html#method.partition_point) |
| `#[must_use]` | [Chapter 2](../chapters/2-names.md) | [Rust reference](https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-must_use-attribute) |
| `Copy struct with a lifetime` | [Chapter 2](../chapters/2-names.md) | [Rust book](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html) |
| `mod tests; in a sibling file` | [Chapter 2](../chapters/2-names.md) | [Rust book](https://doc.rust-lang.org/book/ch11-03-test-organization.html) |
| `impl Into<String> and impl Into<Value> arguments` | [Chapter 3](../chapters/3-values.md) | [Rust book](https://doc.rust-lang.org/book/ch10-02-traits.html#traits-as-parameters) |
| `compile_fail doctests` | [Chapter 3](../chapters/3-values.md) | [rustdoc book](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html#attributes) |
| `enums with data in variants` | [Chapter 3](../chapters/3-values.md) | [Rust book](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html) |
| `FromStr and str::parse` | [Chapter 3](../chapters/3-values.md) | [std](https://doc.rust-lang.org/std/str/trait.FromStr.html) |
| `std::mem::discriminant` | [Chapter 3](../chapters/3-values.md) | [std](https://doc.rust-lang.org/std/mem/fn.discriminant.html) |
| `macro_rules! generating trait impls over a $t:ty list` | [Chapter 3](../chapters/3-values.md) | [Rust reference](https://doc.rust-lang.org/reference/macros-by-example.html) |
| `From and Into` | [Chapter 3](../chapters/3-values.md) | [std](https://doc.rust-lang.org/std/convert/trait.From.html) |
| `use std::fmt::Write as _` | [Chapter 3](../chapters/3-values.md) | [Rust reference](https://doc.rust-lang.org/reference/items/use-declarations.html#underscore-imports) |
| `#[allow(clippy::too_many_arguments)] with a comment` | [Chapter 3](../chapters/3-values.md) | [Clippy](https://clippy.rust-lang.org/master/index.html#too_many_arguments) |
| `and_then, map_err and ok_or_else chains` | [Chapter 3](../chapters/3-values.md) | [std](https://doc.rust-lang.org/std/result/enum.Result.html) |
| `i128::from widening` | [Chapter 3](../chapters/3-values.md) | [std](https://doc.rust-lang.org/std/primitive.i128.html) |
| `bool::then` | [Chapter 3](../chapters/3-values.md) | [std](https://doc.rust-lang.org/std/primitive.bool.html#method.then) |
| `IndexSet` | [Chapter 3](../chapters/3-values.md) | [docs.rs](https://docs.rs/indexmap/latest/indexmap/set/struct.IndexSet.html) |
| `Arc::make_mut and copy on write` | [Chapter 3](../chapters/3-values.md) | [std](https://doc.rust-lang.org/std/sync/struct.Arc.html#method.make_mut) |
| `move closures returning impl Iterator + 'a` | [Chapter 3](../chapters/3-values.md) | [Rust book](https://doc.rust-lang.org/book/ch13-01-closures.html) |
| `implementing Iterator, DoubleEndedIterator, ExactSizeIterator, IntoIterator, FromIterator and Extend` | [Chapter 3](../chapters/3-values.md) | [std](https://doc.rust-lang.org/std/iter/index.html) |
| `internal @rule arms and nested repetition in macro_rules!` | [Chapter 4](../chapters/4-records.md) | [Rust reference](https://doc.rust-lang.org/reference/macros-by-example.html#repetitions) |
| `a macro that defines macros` | [Chapter 4](../chapters/4-records.md) | [Rust reference](https://doc.rust-lang.org/reference/macros-by-example.html) |
| `#[macro_use] and textual macro scope` | [Chapter 4](../chapters/4-records.md) | [Rust reference](https://doc.rust-lang.org/reference/macros-by-example.html#the-macro_use-attribute) |
| `#[non_exhaustive]` | [Chapter 4](../chapters/4-records.md) | [Rust reference](https://doc.rust-lang.org/reference/attributes/type_system.html#the-non_exhaustive-attribute) |
| `associated constants` | [Chapter 4](../chapters/4-records.md) | [Rust reference](https://doc.rust-lang.org/reference/items/associated-items.html#associated-constants) |
| `owned and borrowed twins` | [Chapter 4](../chapters/4-records.md) | [Rust book](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html) |
| `a generic method bounded by a private trait` | [Chapter 4](../chapters/4-records.md) | [Rust book](https://doc.rust-lang.org/book/ch10-01-syntax.html#in-function-definitions) |
| `pub(super)` | [Chapter 4](../chapters/4-records.md) | [Rust reference](https://doc.rust-lang.org/reference/visibility-and-privacy.html#pubin-path-pubcrate-pubsuper-and-pubself) |
| `Cow` | [Chapter 4](../chapters/4-records.md) | [std](https://doc.rust-lang.org/std/borrow/enum.Cow.html) |
| `HashSet and HashMap with borrowed keys` | [Chapter 4](../chapters/4-records.md) | [std](https://doc.rust-lang.org/std/collections/struct.HashSet.html) |
| `chain and flat_map` | [Chapter 4](../chapters/4-records.md) | [std](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.flat_map) |
| `an impl block in another module` | [Chapter 4](../chapters/4-records.md) | [Rust reference](https://doc.rust-lang.org/reference/items/implementations.html#inherent-implementations) |
| `an enum as a state machine and std::mem::replace` | [Chapter 4](../chapters/4-records.md) | [std](https://doc.rust-lang.org/std/mem/fn.replace.html) |
| `debug_assert!` | [Chapter 4](../chapters/4-records.md) | [std](https://doc.rust-lang.org/std/macro.debug_assert.html) |
| `thiserror derives` | [Chapter 4](../chapters/4-records.md) | [docs.rs](https://docs.rs/thiserror/latest/thiserror/) |
| `a recursive type through Box` | [Chapter 4](../chapters/4-records.md) | [Rust book](https://doc.rust-lang.org/book/ch15-01-box.html#enabling-recursive-types-with-boxes) |
| `a Result type alias` | [Chapter 4](../chapters/4-records.md) | [Rust reference](https://doc.rust-lang.org/reference/items/type-aliases.html) |
| `serde's Deserialize and Visitor` | [Chapter 5](../chapters/5-json.md) | [docs.rs](https://docs.rs/serde/latest/serde/de/trait.Visitor.html) |
| `DeserializeSeed` | [Chapter 5](../chapters/5-json.md) | [docs.rs](https://docs.rs/serde/latest/serde/de/trait.DeserializeSeed.html) |
| `IgnoredAny` | [Chapter 5](../chapters/5-json.md) | [docs.rs](https://docs.rs/serde/latest/serde/de/struct.IgnoredAny.html) |
| `serde_json::value::RawValue` | [Chapter 5](../chapters/5-json.md) | [docs.rs](https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html) |
| `#[serde(untagged)]` | [Chapter 5](../chapters/5-json.md) | [serde](https://serde.rs/enum-representations.html) |
| `manual Serialize` | [Chapter 5](../chapters/5-json.md) | [serde](https://serde.rs/impl-serialize.html) |
| `PhantomData` | [Chapter 5](../chapters/5-json.md) | [std](https://doc.rust-lang.org/std/marker/struct.PhantomData.html) |
| `generic newtypes` | [Chapter 5](../chapters/5-json.md) | [Rust book](https://doc.rust-lang.org/book/ch20-02-advanced-traits.html#using-the-newtype-pattern-to-implement-external-traits-on-external-types) |
| `IndexMap Entry matched by variant` | [Chapter 5](../chapters/5-json.md) | [docs.rs](https://docs.rs/indexmap/latest/indexmap/map/enum.Entry.html) |
| `Vec::with_capacity` | [Chapter 5](../chapters/5-json.md) | [std](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.with_capacity) |
| `collect into Result<Vec<_>, _>` | [Chapter 5](../chapters/5-json.md) | [std](https://doc.rust-lang.org/std/iter/trait.FromIterator.html#impl-FromIterator%3CResult%3CT,+E%3E%3E-for-Result%3CV,+E%3E) |
| `&mut *out reborrowing` | [Chapter 5](../chapters/5-json.md) | [Rust reference](https://doc.rust-lang.org/reference/expressions/operator-expr.html#borrow-operators) |
| `String::from_utf8(..).expect(..) with a reason` | [Chapter 5](../chapters/5-json.md) | [std](https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8) |
| `Result<&QName, String> as either-or` | [Chapter 5](../chapters/5-json.md) | [std](https://doc.rust-lang.org/std/result/enum.Result.html) |
| `#[default] on an enum variant` | [Chapter 6](../chapters/6-provn.md) | [Rust reference](https://doc.rust-lang.org/reference/attributes/derive.html) |
| `references tied to the input, not to &self` | [Chapter 6](../chapters/6-provn.md) | [Rust book](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html) |
| `recursive descent, one method per production` | [Chapter 6](../chapters/6-provn.md) | [Rust book](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html) |
| `match guards` | [Chapter 6](../chapters/6-provn.md) | [Rust book](https://doc.rust-lang.org/book/ch19-03-pattern-syntax.html#extra-conditionals-with-match-guards) |
| `a closure taking &mut Self` | [Chapter 6](../chapters/6-provn.md) | [Rust book](https://doc.rust-lang.org/book/ch13-01-closures.html) |
| `a const depth limit on recursion` | [Chapter 6](../chapters/6-provn.md) | [Rust reference](https://doc.rust-lang.org/reference/items/constant-items.html) |
| `.map(drop)` | [Chapter 6](../chapters/6-provn.md) | [std](https://doc.rust-lang.org/std/mem/fn.drop.html) |
| `let chains` | [Chapter 6](../chapters/6-provn.md) | [Rust reference](https://doc.rust-lang.org/reference/expressions/if-expr.html#chains-of-conditions) |
| `let _ = writeln!(..)` | [Chapter 6](../chapters/6-provn.md) | [std](https://doc.rust-lang.org/std/macro.writeln.html) |
| `Result::as_deref with a slice pattern` | [Chapter 6](../chapters/6-provn.md) | [std](https://doc.rust-lang.org/std/result/enum.Result.html#method.as_deref) |
| `a scoped block around a borrow` | [Chapter 6](../chapters/6-provn.md) | [Rust book](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html) |
| `clap's derive API` | [Chapter 7](../chapters/7-cli.md) | [docs.rs](https://docs.rs/clap/latest/clap/_derive/index.html) |
| `anyhow::Result, with_context, anyhow! and bail!` | [Chapter 7](../chapters/7-cli.md) | [docs.rs](https://docs.rs/anyhow/latest/anyhow/) |
| `{:#} on an anyhow::Error` | [Chapter 7](../chapters/7-cli.md) | [docs.rs](https://docs.rs/anyhow/latest/anyhow/struct.Error.html#display-representations) |
| `ExitCode from main` | [Chapter 7](../chapters/7-cli.md) | [std](https://doc.rust-lang.org/std/process/struct.ExitCode.html) |
| `eprintln!` | [Chapter 7](../chapters/7-cli.md) | [std](https://doc.rust-lang.org/std/macro.eprintln.html) |
| `io::read_to_string` | [Chapter 7](../chapters/7-cli.md) | [std](https://doc.rust-lang.org/std/io/fn.read_to_string.html) |
| `io::ErrorKind` | [Chapter 7](../chapters/7-cli.md) | [std](https://doc.rust-lang.org/std/io/enum.ErrorKind.html) |
| `Result::is_ok_and` | [Chapter 7](../chapters/7-cli.md) | [std](https://doc.rust-lang.org/std/result/enum.Result.html#method.is_ok_and) |
| `Option::as_deref and Option::filter` | [Chapter 7](../chapters/7-cli.md) | [std](https://doc.rust-lang.org/std/option/enum.Option.html#method.as_deref) |
| `destructuring an error variant with ..` | [Chapter 7](../chapters/7-cli.md) | [Rust book](https://doc.rust-lang.org/book/ch19-03-pattern-syntax.html#destructuring-enums) |
| `a [char; N] array as a string pattern` | [Chapter 7](../chapters/7-cli.md) | [std](https://doc.rust-lang.org/std/str/pattern/trait.Pattern.html) |
| `eq_ignore_ascii_case, strip_prefix, trim_start_matches and split_once` | [Chapter 7](../chapters/7-cli.md) | [std](https://doc.rust-lang.org/std/primitive.str.html#method.strip_prefix) |
| `HashSet::insert inside a filter` | [Chapter 7](../chapters/7-cli.md) | [std](https://doc.rust-lang.org/std/collections/struct.HashSet.html#method.insert) |
| `a lifetime parameter on a function` | [Chapter 7](../chapters/7-cli.md) | [Rust book](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#lifetime-annotations-in-function-signatures) |
| `clone, try, assign back` | [Chapter 7](../chapters/7-cli.md) | [std](https://doc.rust-lang.org/std/clone/trait.Clone.html) |
| `#[test] and assert! in test code` | [Chapter 8](../chapters/8-tests.md) | [Rust book](https://doc.rust-lang.org/book/ch11-01-writing-tests.html) |
| `env!` | [Chapter 8](../chapters/8-tests.md) | [std](https://doc.rust-lang.org/std/macro.env.html) |
| `impl FnOnce arguments` | [Chapter 8](../chapters/8-tests.md) | [Rust book](https://doc.rust-lang.org/book/ch13-01-closures.html#moving-captured-values-out-of-closures-and-the-fn-traits) |
| `a test double implementing std::io::Write` | [Chapter 8](../chapters/8-tests.md) | [std](https://doc.rust-lang.org/std/io/trait.Write.html) |
| `std::io::Error::other` | [Chapter 8](../chapters/8-tests.md) | [std](https://doc.rust-lang.org/std/io/struct.Error.html#method.other) |
| `unwrap and panic! in test code` | [Chapter 8](../chapters/8-tests.md) | [Rust book](https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html) |
| `#![allow(dead_code)] on a shared test module` | [Chapter 8](../chapters/8-tests.md) | [Rust reference](https://doc.rust-lang.org/reference/attributes.html) |
| `assert_cmd::Command::cargo_bin and predicates` | [Chapter 8](../chapters/8-tests.md) | [docs.rs](https://docs.rs/assert_cmd/latest/assert_cmd/) |
| `impl AsRef<OsStr> arguments` | [Chapter 8](../chapters/8-tests.md) | [std](https://doc.rust-lang.org/std/convert/trait.AsRef.html) |
| `Cargo profiles` | [Chapter 9](../chapters/9-build.md) | [Cargo book](https://doc.rust-lang.org/cargo/reference/profiles.html) |
| `[workspace.lints] and clippy lint groups` | [Chapter 9](../chapters/9-build.md) | [Cargo book](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-lints-table) |
| `cargo deny` | [Chapter 9](../chapters/9-build.md) | [project page](https://embarkstudios.github.io/cargo-deny/) |
| `cargo about` | [Chapter 9](../chapters/9-build.md) | [project page](https://github.com/EmbarkStudios/cargo-about) |
| `typos` | [Chapter 9](../chapters/9-build.md) | [project page](https://github.com/crate-ci/typos) |
| `cargo package and the exclude field` | [Chapter 9](../chapters/9-build.md) | [Cargo book](https://doc.rust-lang.org/cargo/commands/cargo-package.html) |
| `cross-compilation targets` | [Chapter 9](../chapters/9-build.md) | [rustc book](https://doc.rust-lang.org/rustc/platform-support.html) |
| `cargo binstall and release archives` | [Chapter 9](../chapters/9-build.md) | [project page](https://github.com/cargo-bins/cargo-binstall) |
