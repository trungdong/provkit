# Chapter 1: Orientation

provkit is a workspace of two crates. `provkit` is the library, which
holds the data model and the two formats, and `provkit-cli` is the
`provkit` binary that gives the library a command line. This chapter describes
the crate root of the library and the two manifests, nothing else. It
builds on no earlier chapter; every later chapter assumes the module list,
the feature gates and the public surface shown here.

## What the code does

### provkit/src/lib.rs

The file opens with the crate doc, the library's own introduction with
a runnable example. The rendered form is the
[front page of the API reference](../api/provkit/index.html); this
page does not restate it.

Further down comes the module list. The book shows these lines as
they stand in the file, so the block is not a program and is not
compiled.

```rust,ignore
{{#include ../../../provkit/src/lib.rs:modules}}
```

Every module in the span above is private except two. `json` and
`provn` are public, and each sits behind `#[cfg(feature = "...")]` so
it exists only when its feature is on. A further module, `test_util`, is declared below
this list as `pub(crate)` under `#[cfg(test)]`, so it is visible to
the whole crate and to nothing outside it, and only while tests build.

The private modules are reached through re-exports at the crate root.

```rust,ignore
{{#include ../../../provkit/src/lib.rs:exports}}
```

The data-model types all come from the root, so `use provkit::{Document,
Record, QName}` works without knowing which file defines each one. The
two public modules keep their readers and writers, and `provn` keeps
`Profile`, so those are spelled `json::read` and `provn::Profile`.
`well_known` is a module of names in the `prov:` and `xsd:` [namespaces](../reference/glossary.md#namespace-and-prefix-prov)
that the `pub use` line for `qname` brings to the root beside `QName`. The block below is a
doctest the book compiles.

```rust
# extern crate provkit;
use provkit::{Attributes, Document, Entity, QName, Record};

fn main() -> Result<(), provkit::Error> {
    let mut doc = Document::new();
    doc.records.push(Record::Entity(Entity {
        id: QName::new("http://example.org/", "e1")?,
        attributes: Attributes::new(),
    }));
    assert_eq!(doc.records.len(), 1);
    Ok(())
}
```

Two items above the module list matter for the build. `ReadmeDoctests`
is a struct whose only purpose is its doc attribute, which pulls in
`provkit/README.md` with `include_str!` so the README's fenced examples
compile and run under `cargo test --doc`. It exists only under
`#[cfg(doctest)]` and only when both features are on, so the README
cannot rot. The crate-level `#![cfg_attr(...)]` allows `dead_code` and
`unused_imports` when either feature is off, because a crate-private
helper that serves one format has no caller without it. In the default
build both lints stay on.

### provkit/Cargo.toml

The library manifest declares the two features.

```toml
{{#include ../../../provkit/Cargo.toml:features}}
```

`json` turns on the optional `serde` and `serde_json` dependencies and
the `serde` feature of `indexmap`; a build without it has neither crate
and builds `indexmap` without its `serde` feature. `provn` enables nothing, because the PROV-N parser and writer need
no extra crate. Both are on by default.

Each integration test target names the features it needs.

```toml
{{#include ../../../provkit/Cargo.toml:tests}}
```

Cargo skips a `[[test]]` target whose `required-features` are off
instead of failing to compile it. `cargo test -p provkit
--no-default-features --features provn` therefore builds the unit
tests, the doctests and the three `provn` targets, and leaves
`corpus_json` and `corpus_twins` out.

### Cargo.toml

The root manifest holds what both crates share.

```toml
{{#include ../../../Cargo.toml:workspace}}

{{#include ../../../Cargo.toml:workspace_dependencies}}
```

`[workspace.package]` sets the version, the 2024 edition, the minimum
Rust version `1.98`, the licence and the repository once; each crate's
manifest says `version.workspace = true` and so on to inherit them.
The version appears twice, once as the package version and once in the
`provkit` entry of `[workspace.dependencies]`, which is the version
`provkit-cli` depends on. A version change touches both lines.

```toml
{{#include ../../../Cargo.toml:lints}}
```

`unsafe_code = "forbid"` means no `unsafe` block anywhere in either
crate, and `forbid` cannot be overridden by an `allow` further down.
Clippy's `all` and `pedantic` groups both warn, and CI runs clippy with
`-D warnings`, so each is an error there. The manifest allows a
handful of pedantic lints that fight the domain, and its comment says
why for two of them. PROV names repeat their module, as `json::JsonValue`
does, so `module_name_repetitions` is off, and every fallible function
documents its `Error` in one place, so `missing_errors_doc` is off
too. `missing_panics_doc` and `must_use_candidate` are allowed with no
stated reason. Where one lint is wrong for a single item, the code
carries `#[allow(clippy::...)]`
with a comment giving the reason, for example `too_many_arguments` on
`DateTime::new`, which takes one argument per XSD component.

## Why this shape

### Why do readers take `&str` and writers take `impl Write`?

Readers take `&str` and writers take `&mut impl std::io::Write`, so
the library never touches files. Opening a path, sniffing a format from
an extension and reporting `file:line:column` are the command line's
job. A library that only sees strings and writers runs anywhere a
string can be made, and `.github/workflows/ci.yml` builds `provkit` for
`wasm32-unknown-unknown`, a target with no filesystem, with nothing to
exclude.

### Why two features?

`json` is the only feature with dependencies behind it, `serde` and
`serde_json`. A user who only exchanges PROV-N switches `json` off and
pays nothing for the serde crates; the `cargo check` under Run this
shows them absent from the build. The `indexmap/serde` entry sits in
the `json` list for the same reason, so a build without `json` depends
on neither `serde` nor `serde_core`, which an `indexmap` built with its
`serde` feature would pull in.

### Why does every data-model type sit at the crate root?

With every module private except `json` and `provn`, and `well_known`
re-exported from the private `qname`, each data-model type has exactly
one public path and shows once in the API reference, at the root.
`provn` keeps its own three types, `Profile`, `Ignored` and
`ReadReport`. A private module with a `pub use` re-export is what
gives an item that single path. The file layout inside `src/` is then
an internal matter, because no caller can name a private module.

## The Rust here

- **Modules and visibility**. `mod` declares a private module, `pub mod`
  a public one, and `pub(crate)` makes `test_util` visible to the whole
  crate and nowhere else.
  [Defining modules](https://doc.rust-lang.org/book/ch07-02-defining-modules-to-control-scope-and-privacy.html),
  [Paths in the module tree](https://doc.rust-lang.org/book/ch07-03-paths-for-referring-to-an-item-in-the-module-tree.html)
- **Re-exports with `pub use`**. Each `pub use` line in `lib.rs` brings
  a private module's items to the crate root under the same name, or a
  new one with `as`.
  [Bringing paths into scope with use](https://doc.rust-lang.org/book/ch07-04-bringing-paths-into-scope-with-the-use-keyword.html)
- **Conditional compilation**. `#[cfg(feature = "json")]` keeps the
  `json` module out of the build when the feature is off, and
  `#![cfg_attr(...)]` applies an attribute only when its condition
  holds.
  [Conditional compilation](https://doc.rust-lang.org/reference/conditional-compilation.html)
- **Doc comments and doctests**. `//!` documents the enclosing item and
  `///` the next one; a fenced Rust block in either is compiled and run
  by `cargo test --doc`, and a line starting `# ` compiles but does not
  render.
  [Publishing a crate](https://doc.rust-lang.org/book/ch14-02-publishing-to-crates-io.html),
  [Documentation tests](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html)
- **`include_str!`**. The macro reads a file at compile time into a
  `&'static str`; inside `#[doc = ...]` on `ReadmeDoctests` that text
  becomes the item's documentation, doctests included.
  [include_str](https://doc.rust-lang.org/std/macro.include_str.html)
- **Cargo workspaces and features**. A workspace builds several crates
  with one lock file, and `[workspace.package]` holds the metadata they
  share; a feature is a named switch that turns on code and optional
  dependencies.
  [Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html),
  [Features](https://doc.rust-lang.org/cargo/reference/features.html)

## Run this

```sh
cargo test -p provkit --doc
```

This runs the doctests of the library, the examples in the crate doc and
in `provkit/README.md` among them. Two groups report, the tests that run
and the `compile_fail` tests that must not build.

```text
test result: ok. 11 passed
test result: ok. 4 passed
```

```sh
cargo check -p provkit --no-default-features --features provn
```

This type-checks the library with `json` off. It exits 0 and no line of
its output names `serde` or `serde_json`, on a cold cache or a warm one.

## Try it

Build the library with only `provn` and call `provkit::json::read` from
a short program. Make a scratch crate next to the checkout, point it
at `provkit/` with `default-features = false`, and run `cargo build`.
What does the compiler say, and which line of `lib.rs` does it point
at?

<details><summary>Answer</summary>

Create the scratch crate beside the checkout, which sits in a directory
named `provkit`.

```sh
cargo new try-provn
cd try-provn
```

Add the dependency to the new `Cargo.toml`.

```toml
[package]
name = "try-provn"
version = "0.1.0"
edition = "2024"

[dependencies]
provkit = { path = "../provkit/provkit", default-features = false, features = ["provn"] }
```

Replace `src/main.rs` with a program that names the `json` module. The
book does not compile this block, because the book builds the library
with both features on and the program would succeed there.

```rust,ignore
fn main() {
    let doc = provkit::json::read("{}");
    println!("{}", doc.is_ok());
}
```

Then build.

```sh
cargo build
```

The build fails with E0433. The first lines of the error are these.

```text
error[E0433]: cannot find `json` in `provkit`
  --> src/main.rs:2:24
   |
 2 |     let doc = provkit::json::read("{}");
   |                        ^^^^ could not find `json` in `provkit`
```

Below them the compiler adds a note, "found an item that was configured
out", pointing at `pub mod json;` in `lib.rs` and the
`#[cfg(feature = "json")]` line above it, which it describes as "the
item is gated behind the `json` feature". In this build the module does
not exist, so no visibility rule is involved. Put `"json"` into the `features`
list, or drop `default-features = false`, and the same program builds.

</details>

## Idioms first seen here

- `modules and visibility`: https://doc.rust-lang.org/book/ch07-02-defining-modules-to-control-scope-and-privacy.html
- `pub use`: https://doc.rust-lang.org/book/ch07-04-bringing-paths-into-scope-with-the-use-keyword.html
- `#[cfg(feature)]`: https://doc.rust-lang.org/reference/conditional-compilation.html
- `doc comments and doctests`: https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html
- `include_str!`: https://doc.rust-lang.org/std/macro.include_str.html
- `Cargo workspace and features`: https://doc.rust-lang.org/cargo/reference/workspaces.html
