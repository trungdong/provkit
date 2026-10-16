# New to Rust

Read the stops in order. Each Rust idiom is named the first time it
appears, with a link to the Rust book, the reference, the standard
library or the crate that documents it. The table lists the idioms in
that order, so you can see what each stop asks of you before you start
it. Three chapters of the Rust book carry most of the weight, which are
ownership (chapter 4), enums and pattern matching (chapter 6), and
traits, generics and lifetimes (chapter 10). The generated
[idiom index](../reference/idioms.md) lists every idiom any commit
introduced, grouped by the document it links to. The
[glossary](../reference/glossary.md) explains the words that Rust and
PROV use differently.

| Idiom | First seen | Read |
|---|---|---|
| `modules and visibility` | [Stop 0](../stops/0-orientation.md) | [Rust book](https://doc.rust-lang.org/book/ch07-02-defining-modules-to-control-scope-and-privacy.html) |
| `pub use` | [Stop 0](../stops/0-orientation.md) | [Rust book](https://doc.rust-lang.org/book/ch07-04-bringing-paths-into-scope-with-the-use-keyword.html) |
| `#[cfg(feature)]` | [Stop 0](../stops/0-orientation.md) | [Rust reference](https://doc.rust-lang.org/reference/conditional-compilation.html) |
| `doc comments and doctests` | [Stop 0](../stops/0-orientation.md) | [rustdoc book](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html) |
| `include_str!` | [Stop 0](../stops/0-orientation.md) | [std](https://doc.rust-lang.org/std/macro.include_str.html) |
| `Cargo workspace and features` | [Stop 0](../stops/0-orientation.md) | [Cargo book](https://doc.rust-lang.org/cargo/reference/workspaces.html) |
| `ownership and borrowing` | [Stop 1](../stops/1-names.md) | [Rust book](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html) |
| `Arc<str>` | [Stop 1](../stops/1-names.md) | [std](https://doc.rust-lang.org/std/sync/struct.Arc.html) |
| `Box<str>` | [Stop 1](../stops/1-names.md) | [std](https://doc.rust-lang.org/std/boxed/struct.Box.html) |
| `#[derive(Clone)]` | [Stop 1](../stops/1-names.md) | [Rust book](https://doc.rust-lang.org/book/appendix-03-derivable-traits.html) |
| `hand-written PartialEq, Eq, Ord, PartialOrd and Hash` | [Stop 1](../stops/1-names.md) | [std](https://doc.rust-lang.org/std/hash/trait.Hash.html) |
| `impl Iterator + '_ and lifetime elision` | [Stop 1](../stops/1-names.md) | [Rust reference](https://doc.rust-lang.org/reference/lifetime-elision.html) |
| `iterator adaptors all, eq and cmp` | [Stop 1](../stops/1-names.md) | [std](https://doc.rust-lang.org/std/iter/trait.Iterator.html) |
| `matches!` | [Stop 1](../stops/1-names.md) | [std](https://doc.rust-lang.org/std/macro.matches.html) |
| `? on Option and Result` | [Stop 1](../stops/1-names.md) | [Rust book](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html) |
| `then_some and is_some_and` | [Stop 1](../stops/1-names.md) | [std](https://doc.rust-lang.org/std/option/enum.Option.html) |
| `LazyLock` | [Stop 1](../stops/1-names.md) | [std](https://doc.rust-lang.org/std/sync/struct.LazyLock.html) |
| `macro_rules! with $(...)* repetition` | [Stop 1](../stops/1-names.md) | [Rust reference](https://doc.rust-lang.org/reference/macros-by-example.html) |
| `#[doc = concat!(...)]` | [Stop 1](../stops/1-names.md) | [rustdoc book](https://doc.rust-lang.org/rustdoc/write-documentation/the-doc-attribute.html) |
| `IndexMap` | [Stop 1](../stops/1-names.md) | [docs.rs](https://docs.rs/indexmap) |
| `let ... else` | [Stop 1](../stops/1-names.md) | [Rust reference](https://doc.rust-lang.org/reference/statements.html#let-statements) |
| `entry().or_default() and partition_point` | [Stop 1](../stops/1-names.md) | [std](https://doc.rust-lang.org/std/primitive.slice.html#method.partition_point) |
| `#[must_use]` | [Stop 1](../stops/1-names.md) | [Rust reference](https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-must_use-attribute) |
| `Copy struct with a lifetime` | [Stop 1](../stops/1-names.md) | [Rust book](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html) |
| `mod tests; in a sibling file` | [Stop 1](../stops/1-names.md) | [Rust book](https://doc.rust-lang.org/book/ch11-03-test-organization.html) |
| `impl Into<String> and impl Into<Value> arguments` | [Stop 2](../stops/2-values.md) | [Rust book](https://doc.rust-lang.org/book/ch10-02-traits.html#traits-as-parameters) |
| `compile_fail doctests` | [Stop 2](../stops/2-values.md) | [rustdoc book](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html#attributes) |
| `enums with data in variants` | [Stop 2](../stops/2-values.md) | [Rust book](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html) |
| `FromStr and str::parse` | [Stop 2](../stops/2-values.md) | [std](https://doc.rust-lang.org/std/str/trait.FromStr.html) |
| `std::mem::discriminant` | [Stop 2](../stops/2-values.md) | [std](https://doc.rust-lang.org/std/mem/fn.discriminant.html) |
| `macro_rules! generating trait impls over a $t:ty list` | [Stop 2](../stops/2-values.md) | [Rust reference](https://doc.rust-lang.org/reference/macros-by-example.html) |
| `From and Into` | [Stop 2](../stops/2-values.md) | [std](https://doc.rust-lang.org/std/convert/trait.From.html) |
| `use std::fmt::Write as _` | [Stop 2](../stops/2-values.md) | [Rust reference](https://doc.rust-lang.org/reference/items/use-declarations.html#underscore-imports) |
| `#[allow(clippy::too_many_arguments)] with a reason` | [Stop 2](../stops/2-values.md) | [Clippy](https://clippy.rust-lang.org/master/index.html#too_many_arguments) |
| `and_then, map_err and ok_or_else chains` | [Stop 2](../stops/2-values.md) | [std](https://doc.rust-lang.org/std/result/enum.Result.html) |
| `i128::from widening` | [Stop 2](../stops/2-values.md) | [std](https://doc.rust-lang.org/std/primitive.i128.html) |
| `bool::then` | [Stop 2](../stops/2-values.md) | [std](https://doc.rust-lang.org/std/primitive.bool.html#method.then) |
| `IndexSet` | [Stop 2](../stops/2-values.md) | [docs.rs](https://docs.rs/indexmap/latest/indexmap/set/struct.IndexSet.html) |
| `Arc::make_mut and copy on write` | [Stop 2](../stops/2-values.md) | [std](https://doc.rust-lang.org/std/sync/struct.Arc.html#method.make_mut) |
| `move closures returning impl Iterator + 'a` | [Stop 2](../stops/2-values.md) | [Rust book](https://doc.rust-lang.org/book/ch13-01-closures.html) |
| `implementing Iterator, DoubleEndedIterator, ExactSizeIterator, IntoIterator, FromIterator and Extend` | [Stop 2](../stops/2-values.md) | [std](https://doc.rust-lang.org/std/iter/index.html) |
| `internal @rule arms and nested repetition in macro_rules!` | [Stop 3](../stops/3-records.md) | [Rust reference](https://doc.rust-lang.org/reference/macros-by-example.html#repetitions) |
| `a macro that defines macros` | [Stop 3](../stops/3-records.md) | [Rust reference](https://doc.rust-lang.org/reference/macros-by-example.html) |
| `#[macro_use] and textual macro scope` | [Stop 3](../stops/3-records.md) | [Rust reference](https://doc.rust-lang.org/reference/macros-by-example.html#the-macro_use-attribute) |
| `#[non_exhaustive]` | [Stop 3](../stops/3-records.md) | [Rust reference](https://doc.rust-lang.org/reference/attributes/type_system.html#the-non_exhaustive-attribute) |
| `associated constants` | [Stop 3](../stops/3-records.md) | [Rust reference](https://doc.rust-lang.org/reference/items/associated-items.html#associated-constants) |
| `owned and borrowed twins` | [Stop 3](../stops/3-records.md) | [Rust book](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html) |
| `a generic method bounded by a private trait` | [Stop 3](../stops/3-records.md) | [Rust book](https://doc.rust-lang.org/book/ch10-01-syntax.html#in-function-definitions) |
| `pub(super)` | [Stop 3](../stops/3-records.md) | [Rust reference](https://doc.rust-lang.org/reference/visibility-and-privacy.html#pubin-path-pubcrate-pubsuper-and-pubself) |
| `Cow` | [Stop 3](../stops/3-records.md) | [std](https://doc.rust-lang.org/std/borrow/enum.Cow.html) |
| `HashSet and HashMap with borrowed keys` | [Stop 3](../stops/3-records.md) | [std](https://doc.rust-lang.org/std/collections/struct.HashSet.html) |
| `chain and flat_map` | [Stop 3](../stops/3-records.md) | [std](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.flat_map) |
| `an impl block in another module` | [Stop 3](../stops/3-records.md) | [Rust reference](https://doc.rust-lang.org/reference/items/implementations.html#inherent-implementations) |
| `an enum as a state machine and std::mem::replace` | [Stop 3](../stops/3-records.md) | [std](https://doc.rust-lang.org/std/mem/fn.replace.html) |
| `debug_assert!` | [Stop 3](../stops/3-records.md) | [std](https://doc.rust-lang.org/std/macro.debug_assert.html) |
| `thiserror derives` | [Stop 3](../stops/3-records.md) | [docs.rs](https://docs.rs/thiserror/latest/thiserror/) |
| `a recursive type through Box` | [Stop 3](../stops/3-records.md) | [Rust book](https://doc.rust-lang.org/book/ch15-01-box.html#enabling-recursive-types-with-boxes) |
| `a Result type alias` | [Stop 3](../stops/3-records.md) | [Rust reference](https://doc.rust-lang.org/reference/items/type-aliases.html) |
| `serde's Deserialize and Visitor` | [Stop 4](../stops/4-json.md) | [docs.rs](https://docs.rs/serde/latest/serde/de/trait.Visitor.html) |
| `DeserializeSeed` | [Stop 4](../stops/4-json.md) | [docs.rs](https://docs.rs/serde/latest/serde/de/trait.DeserializeSeed.html) |
| `IgnoredAny` | [Stop 4](../stops/4-json.md) | [docs.rs](https://docs.rs/serde/latest/serde/de/struct.IgnoredAny.html) |
| `serde_json::value::RawValue` | [Stop 4](../stops/4-json.md) | [docs.rs](https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html) |
| `#[serde(untagged)]` | [Stop 4](../stops/4-json.md) | [serde](https://serde.rs/enum-representations.html) |
| `manual Serialize` | [Stop 4](../stops/4-json.md) | [serde](https://serde.rs/impl-serialize.html) |
| `PhantomData` | [Stop 4](../stops/4-json.md) | [std](https://doc.rust-lang.org/std/marker/struct.PhantomData.html) |
| `generic newtypes` | [Stop 4](../stops/4-json.md) | [Rust book](https://doc.rust-lang.org/book/ch20-02-advanced-traits.html#using-the-newtype-pattern-to-implement-external-traits-on-external-types) |
| `IndexMap Entry matched by variant` | [Stop 4](../stops/4-json.md) | [docs.rs](https://docs.rs/indexmap/latest/indexmap/map/enum.Entry.html) |
| `Vec::with_capacity` | [Stop 4](../stops/4-json.md) | [std](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.with_capacity) |
| `collect into Result<Vec<_>, _>` | [Stop 4](../stops/4-json.md) | [std](https://doc.rust-lang.org/std/iter/trait.FromIterator.html#impl-FromIterator%3CResult%3CA,+E%3E%3E-for-Result%3CV,+E%3E) |
| `&mut *out reborrowing` | [Stop 4](../stops/4-json.md) | [Rust reference](https://doc.rust-lang.org/reference/expressions/operator-expr.html#borrow-operators) |
| `String::from_utf8(..).expect(..) with a reason` | [Stop 4](../stops/4-json.md) | [std](https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8) |
| `Result<&QName, String> as either-or` | [Stop 4](../stops/4-json.md) | [std](https://doc.rust-lang.org/std/result/enum.Result.html) |
| `#[default] on an enum variant` | [Stop 5](../stops/5-provn.md) | [Rust reference](https://doc.rust-lang.org/reference/attributes/derive.html) |
| `references tied to the input, not to &self` | [Stop 5](../stops/5-provn.md) | [Rust book](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html) |
| `recursive descent, one method per production` | [Stop 5](../stops/5-provn.md) | [Rust book](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html) |
| `match guards` | [Stop 5](../stops/5-provn.md) | [Rust book](https://doc.rust-lang.org/book/ch19-03-pattern-syntax.html#extra-conditionals-with-match-guards) |
| `a closure taking &mut Self` | [Stop 5](../stops/5-provn.md) | [Rust book](https://doc.rust-lang.org/book/ch13-01-closures.html) |
| `a const depth limit on recursion` | [Stop 5](../stops/5-provn.md) | [Rust reference](https://doc.rust-lang.org/reference/items/constant-items.html) |
| `.map(drop)` | [Stop 5](../stops/5-provn.md) | [std](https://doc.rust-lang.org/std/mem/fn.drop.html) |
| `let chains` | [Stop 5](../stops/5-provn.md) | [Rust reference](https://doc.rust-lang.org/reference/expressions/if-expr.html#chains-of-conditions) |
| `let _ = writeln!(..)` | [Stop 5](../stops/5-provn.md) | [std](https://doc.rust-lang.org/std/macro.writeln.html) |
| `Result::as_deref with a slice pattern` | [Stop 5](../stops/5-provn.md) | [std](https://doc.rust-lang.org/std/result/enum.Result.html#method.as_deref) |
| `a scoped block to end a borrow` | [Stop 5](../stops/5-provn.md) | [Rust book](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html) |
| `clap's derive API` | [Stop 6](../stops/6-cli.md) | [docs.rs](https://docs.rs/clap/latest/clap/_derive/index.html) |
| `anyhow::Result, with_context, anyhow! and bail!` | [Stop 6](../stops/6-cli.md) | [docs.rs](https://docs.rs/anyhow/latest/anyhow/) |
| `{:#} on an anyhow::Error` | [Stop 6](../stops/6-cli.md) | [docs.rs](https://docs.rs/anyhow/latest/anyhow/struct.Error.html#display-representations) |
| `ExitCode from main` | [Stop 6](../stops/6-cli.md) | [std](https://doc.rust-lang.org/std/process/struct.ExitCode.html) |
| `eprintln!` | [Stop 6](../stops/6-cli.md) | [std](https://doc.rust-lang.org/std/macro.eprintln.html) |
| `io::read_to_string` | [Stop 6](../stops/6-cli.md) | [std](https://doc.rust-lang.org/std/io/fn.read_to_string.html) |
| `io::ErrorKind` | [Stop 6](../stops/6-cli.md) | [std](https://doc.rust-lang.org/std/io/enum.ErrorKind.html) |
| `Result::is_ok_and` | [Stop 6](../stops/6-cli.md) | [std](https://doc.rust-lang.org/std/result/enum.Result.html#method.is_ok_and) |
| `Option::as_deref and Option::filter` | [Stop 6](../stops/6-cli.md) | [std](https://doc.rust-lang.org/std/option/enum.Option.html#method.as_deref) |
| `destructuring an error variant with ..` | [Stop 6](../stops/6-cli.md) | [Rust book](https://doc.rust-lang.org/book/ch19-03-pattern-syntax.html#destructuring-enums) |
| `a [char; N] array as a string pattern` | [Stop 6](../stops/6-cli.md) | [std](https://doc.rust-lang.org/std/str/pattern/trait.Pattern.html) |
| `eq_ignore_ascii_case, strip_prefix, trim_start_matches and split_once` | [Stop 6](../stops/6-cli.md) | [std](https://doc.rust-lang.org/std/primitive.str.html#method.strip_prefix) |
| `HashSet::insert inside a filter` | [Stop 6](../stops/6-cli.md) | [std](https://doc.rust-lang.org/std/collections/struct.HashSet.html#method.insert) |
| `a lifetime parameter on a function` | [Stop 6](../stops/6-cli.md) | [Rust book](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#lifetime-annotations-in-function-signatures) |
| `clone, try, assign back` | [Stop 6](../stops/6-cli.md) | [std](https://doc.rust-lang.org/std/clone/trait.Clone.html) |
| `#[test], assert! and assert_eq!` | [Stop 7](../stops/7-tests.md) | [Rust book](https://doc.rust-lang.org/book/ch11-01-writing-tests.html) |
| `env!` | [Stop 7](../stops/7-tests.md) | [std](https://doc.rust-lang.org/std/macro.env.html) |
| `impl FnOnce arguments` | [Stop 7](../stops/7-tests.md) | [Rust book](https://doc.rust-lang.org/book/ch13-01-closures.html#moving-captured-values-out-of-closures-and-the-fn-traits) |
| `a test double implementing std::io::Write` | [Stop 7](../stops/7-tests.md) | [std](https://doc.rust-lang.org/std/io/trait.Write.html) |
| `std::io::Error::other` | [Stop 7](../stops/7-tests.md) | [std](https://doc.rust-lang.org/std/io/struct.Error.html#method.other) |
| `unwrap and panic! in test code` | [Stop 7](../stops/7-tests.md) | [Rust book](https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html) |
| `#![allow(dead_code)] on a shared test module` | [Stop 7](../stops/7-tests.md) | [Rust reference](https://doc.rust-lang.org/reference/attributes.html) |
| `assert_cmd::Command::cargo_bin and predicates` | [Stop 7](../stops/7-tests.md) | [docs.rs](https://docs.rs/assert_cmd/latest/assert_cmd/) |
| `impl AsRef<OsStr> arguments` | [Stop 7](../stops/7-tests.md) | [std](https://doc.rust-lang.org/std/convert/trait.AsRef.html) |
| `Cargo profiles` | [Stop 8](../stops/8-build.md) | [Cargo book](https://doc.rust-lang.org/cargo/reference/profiles.html) |
| `[workspace.lints] and clippy lint groups` | [Stop 8](../stops/8-build.md) | [Cargo book](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-lints-table) |
| `cargo deny` | [Stop 8](../stops/8-build.md) | [project page](https://embarkstudios.github.io/cargo-deny/) |
| `cargo about` | [Stop 8](../stops/8-build.md) | [project page](https://github.com/EmbarkStudios/cargo-about) |
| `typos` | [Stop 8](../stops/8-build.md) | [project page](https://github.com/crate-ci/typos) |
| `cargo package and the exclude field` | [Stop 8](../stops/8-build.md) | [Cargo book](https://doc.rust-lang.org/cargo/commands/cargo-package.html) |
| `cross-compilation targets` | [Stop 8](../stops/8-build.md) | [rustc book](https://doc.rust-lang.org/rustc/platform-support.html) |
| `cargo binstall and release archives` | [Stop 8](../stops/8-build.md) | [project page](https://github.com/cargo-bins/cargo-binstall) |
