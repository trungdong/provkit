# Chapter 9: Build and release

The last chapter is everything that runs before and after the code: the
lint set, the checks CI runs on a push to `main` and on every pull
request, the licence check and the workflow that turns a tag into a
release. None of it is Rust, and all
of it decides what a contributor's change must pass and what a user
downloads. This chapter reads the release profile in the root manifest,
the clippy and rustfmt settings, the configuration of `cargo deny`,
`typos` and `cargo about`, and the CI and release workflows, each
file included whole. It builds on chapter 1 for the manifests and the
lint table, which that chapter shows and this one does not repeat, and on
chapter 8 for the `exclude` line and the `[[test]]` targets that the
packaged-crate and feature-subset steps of CI exercise.

## What the code does

### Cargo.toml

The root manifest ends with the release profile.

```toml
{{#include ../../../Cargo.toml:release_profile}}
```

A Cargo profile is a named set of compiler settings, and `release` is
the one `cargo build --release` and `cargo install` use. `lto =
"thin"` lets the optimiser work across
crate boundaries, `codegen-units = 1` hands each crate to the optimiser
as one unit instead of splitting it for parallel compilation, and
`strip = true` drops the symbols from the binary. The three trade
build time for a smaller, faster binary. The `dev` profile that
`cargo build` and `cargo test` use is not changed. The lint table sits
above the profile in the same file, and
[chapter 1](1-orientation.md#cargotoml) reads it. Each crate manifest
carries `[lints]` with `workspace = true`, so both crates build under
that one table.

### clippy.toml and rustfmt.toml

Both tools read a file at the workspace root, and each file holds one
setting beyond the defaults.

```toml
{{#include ../../../clippy.toml}}
```

`large-error-threshold` is the size in bytes above which clippy's
`result_large_err` lint warns about the `Err` type of a returned
`Result`. The [`Error`](../api/provkit/enum.Error.html) enum of chapter 4
is larger than clippy's default because the `Unification` variant
carries two `Option<Formal>` beside the identifier, and boxing that
rare variant was judged worse than the extra bytes on every `Result`,
so the threshold moved instead.

```toml
{{#include ../../../rustfmt.toml}}
```

`style_edition = "2024"` selects the 2024 formatting rules, which
match the edition the workspace sets.

### deny.toml

```toml
{{#include ../../../deny.toml}}
```

`cargo deny check` reads `Cargo.lock` and runs the checks the file
names. `[advisories]` compares every crate in the lock file with the
RustSec advisory database. With `version = 2` an advisory for a
vulnerability is an error, and a crate whose version has been yanked
from crates.io is a warning, because the file does not set `yanked`.
`[licenses]` fails the check for any crate whose licence is not in
`allow`. The entries are the licences the dependencies use, and
nothing else. `[bans]` with
`multiple-versions = "warn"` reports two versions of one crate in the
lock file without failing. `[sources]` denies a crate that comes from
a registry other than crates.io or from a git repository. The `deny`
job in `ci.yml` runs the same check through the `cargo-deny-action`,
with no arguments, so the file above is its whole configuration.

### _typos.toml

```toml
{{#include ../../../_typos.toml}}
```

`typos` reads every file in the repository that is not ignored and
compares each word with its list of common misspellings. The `[files]`
table excludes the corpus, because the submodule is shared with ProvPy
and its text is not this repository's to spell. Each entry under
`[default.extend-words]` maps a word to itself, which tells `typos`
that the spelling is intended, and the entries come in groups by where
the words occur. The British spellings are the first group. The others
are an identifier from the PROV-N name grammar and the strings the
tests need, a value an ill-formed `xsd:int` keeps, a fragment of an
escaped word in the lexer tests and a misspelt keyword that a test
expects to be a syntax error.

### about.toml and about.hbs

```toml
{{#include ../../../about.toml}}
```

`cargo about` walks the dependency graph, gathers the licence of every
crate, and renders a template with the result.
`ignore-dev-dependencies` and `ignore-build-dependencies` leave out
the crates that only the tests or the build scripts use, so the output
names only crates the binary links. `accepted` is the same list as
`allow` in `deny.toml`, and `targets` is the list of the release
matrix in `release.yml`, so the graph is resolved for the platforms
the binaries are built for and nothing else.

````hbs
{{#include ../../../about.hbs}}
````

The template is Handlebars. It opens by naming the binaries. The first
loop writes one line per licence with the number of crates under it,
and the second writes a section per licence with the crates that use
it and the licence text. The triple braces around `text` write the
text unescaped; double braces would escape it, and quotes in the
licence text would show as HTML entities. `release.yml` runs `cargo
about generate about.hbs -o THIRD-PARTY.md` in each matrix job and
passes the file to the upload step through `include`, so every
release archive carries it.

### .github/workflows/ci.yml

```yaml
{{#include ../../../.github/workflows/ci.yml}}
```

The workflow runs on a push to `main` and on every pull request, with
the token's permissions reduced to reading the repository. The `test`
job runs once per entry in its matrix, on the latest Ubuntu and the
latest Windows runner. On Windows the first step turns off CRLF
conversion before the checkout, because the tests read LF text. The
checkout brings the corpus submodule, because the corpus tests of
chapter 8 read it. The toolchain step installs the version its
`toolchain` line names, together with `rustfmt`, `clippy` and the
`wasm32-unknown-unknown` target. Every action is pinned to a commit,
with the tag that commit carries in a trailing comment.

Of the cargo commands, only `cargo test --workspace --all-features`
runs on both runners. Every other one carries
`if: matrix.os == 'ubuntu-latest'`, so only the workspace tests run on
Windows. The Ubuntu steps are `cargo fmt --check`, clippy over every target and
feature with warnings as errors, the workspace tests, `cargo package
--workspace` followed by `cargo test` on the packaged copy of
`provkit` under `target/package`, which chapter 8's last card covers,
the tests of `provkit` under `provn` alone and under `json` alone,
`cargo check --tests` under no default features, `provn` alone and
`json` alone with `RUSTFLAGS` set to deny warnings, `cargo doc` with
`RUSTDOCFLAGS` set the same way, so a broken intra-doc link fails the
job, and a build of the library for `wasm32-unknown-unknown`. The
`cargo pkgid` pipeline in the packaged-crate step reads the version
from Cargo rather than from a literal, so the step does not change at
a release.

`clippy-stable` runs the same clippy command on the `stable`
toolchain, so the next toolchain's lints show before the pin moves,
and `continue-on-error: true` means it never blocks. `deny` and
`typos` each check out the repository and run the tool's
own action with its defaults, so `deny.toml` and `_typos.toml` are the
whole of their configuration. `book` checks out the full history,
because the idiom index at the end of this book is generated from
every commit message, installs the same toolchain and a pinned
`mdbook` binary through `install-action`, and runs `book/build.sh`,
the script that builds the pages you are reading and runs their
doctests, so a snippet whose anchor vanished or a doctest that no
longer compiles fails the run.

### .github/workflows/release.yml

```yaml
{{#include ../../../.github/workflows/release.yml}}
```

The workflow runs when a tag matching `v*` is pushed, with permission
to write to the repository's contents. `check-version` reads `cargo metadata`, picks the version of
`provkit-cli` out of the JSON with a line of Python, and tests that
`v` followed by it equals the tag name in `GITHUB_REF_NAME`. A tag
that does not match the manifest fails here and nothing else runs.
`create-release` needs that job and makes the GitHub release through
`create-gh-release-action`, with `changelog: CHANGELOG.md`, so the
release body is the changelog entry for that version. `binaries`
needs both. It runs once per entry in its matrix, a target paired
with the runner that builds it, and asks for further permissions,
`id-token` and `attestations`, which the attestation step uses. The
steps check out with the submodule, install the toolchain at the
version `ci.yml` pins, install `cargo-about` through
`install-action`, write `THIRD-PARTY.md`, and hand
`upload-rust-binary-action` the binary name, the target, the manifest
of `provkit-cli`, the notices file to include, `checksum: sha256` for
a digest file beside the archive, and `locked: true` so the build
uses `Cargo.lock` as committed. The last step runs
`attest-build-provenance` on the archive the upload step reports in
its `tar` or `zip` output; the `.sha256` files are not attested.

[RELEASING.md](https://github.com/trungdong/provkit/blob/main/RELEASING.md)
is the human side of the same steps. A release is a tag `v<version>`
on `main`, the version sits in two places in the root manifest, and
the workflow checks the tag against the version of `provkit-cli`. The
release steps set the changelog date, run the checks CI runs and a
clean `cargo package --workspace`, commit, tag with `git tag -a`, wait
for the workflow, publish `provkit` and then `provkit-cli` to
crates.io, and install from both sources to check. Two recovery paths
follow, for a workflow that fails after the tag and for `provkit`
published without `provkit-cli`, and the places the toolchain pin
appears are listed last, because they move together.

## Why this shape

### Why is the toolchain pinned, and why is every action pinned by commit?

The toolchain is pinned so that a new lint in a later toolchain does
not fail a push that changed no code. The pin is raised together with
`rust-version` and the pin in `release.yml`, so the binaries are built
with the toolchain CI tests, and a higher minimum is a minor release.
The `clippy-stable` job is the other half of the arrangement. It shows
the next batch of lints before the pinned toolchain moves and cannot
fail the run, so the lints a new toolchain brings are visible on every
push and fail nothing until the pin is raised on purpose. A commit pin
names one revision of an action, and the trailing comment keeps the
tag readable beside the hash.

### Why test the feature subsets?

A user who only exchanges PROV-N switches `json` off and pays nothing
for the serde crates, as chapter 1's "Why two features?" card says, and
the subset steps in `ci.yml` build what that user gets. The
crate-level example builds to an empty `main` when a feature is off,
so the feature-subset test runs compile, and the `cargo check --tests`
steps under `RUSTFLAGS=-D warnings` cover no default features, `provn`
alone and `json` alone. Chapter 1 read the pieces of the library these
steps exercise, the
`required-features` on each `[[test]]` target, so a corpus test that
needs a feature is skipped rather than failed, and the crate-level
`#![cfg_attr(...)]` that allows dead code when a feature is off, so
a helper with no caller in that build does not fail under `-D
warnings`.

### Why build for wasm32?

The library takes strings and writers so that it never touches files,
as chapter 1's first card says, and `ci.yml` builds it for
`wasm32-unknown-unknown`, a target with no filesystem, with nothing to
exclude. The step compiles the library and every dependency its
default features pull in for a target with no operating system
beneath it, so a dependency that needs one may not compile there.
The target is installed in the toolchain step with `targets:
wasm32-unknown-unknown`. It is one of the checks that run in CI only,
and `rustup target add wasm32-unknown-unknown` adds the target
locally.

### Why `cargo deny` and `cargo about` both?

The two tools check the dependencies at two moments.
`cargo deny check` runs on every push to `main` and every pull
request, and fails on a crate whose licence is outside `allow`, so a
dependency under another licence fails CI. `cargo about` runs in each
release job and writes the notices for the crates that passed. Each
archive carries `THIRD-PARTY.md`, generated from `about.toml` and
`about.hbs`; the configuration skips dev and build dependencies so
the file lists only crates the binary links, and it accepts only those
licences. `accepted` in `about.toml` and `allow` in `deny.toml` are
the same
names, so a licence that `cargo deny` admits is one the notices can
render, and a new dependency under another licence is a change to
both files.

### Why must the repository be public before the tag?

GitHub's artifact attestations fail in a private repository on a free
personal account, so the repository must be public before the tag is
pushed
([RELEASING.md](https://github.com/trungdong/provkit/blob/main/RELEASING.md)).
The attestation is the last step of each `binaries` job in
`release.yml`,
`attest-build-provenance` with the `id-token` and `attestations`
permissions, so a failure there is a failure of the release workflow
after the tag exists. The recovery for that case is to fix the cause
on `main`, delete the tag locally and remotely, delete a release
created without assets, and tag again; nothing has been published to
crates.io, so the version number can stay.

## The Rust here

- **Cargo profiles**. A profile is a named set of compiler settings;
  `lto`, `codegen-units` and `strip` are the settings
  `[profile.release]` changes from the defaults, and `dev` is the profile `cargo build` and `cargo
  test` use.
  [Profiles](https://doc.rust-lang.org/cargo/reference/profiles.html),
  [lto](https://doc.rust-lang.org/cargo/reference/profiles.html#lto),
  [codegen-units](https://doc.rust-lang.org/cargo/reference/profiles.html#codegen-units),
  [strip](https://doc.rust-lang.org/cargo/reference/profiles.html#strip)
- **`[workspace.lints]` and clippy lint groups**. A lint table in the
  workspace manifest, which chapter 1 shows, applies to every crate whose
  own manifest says `[lints] workspace = true`. `all` and `pedantic` are groups of clippy
  lints, and `priority = -1` on a group lets a single lint named after
  it override the group's level.
  [The lints table](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-lints-table),
  [Clippy lints](https://doc.rust-lang.org/clippy/lints.html)
- **`cargo deny`**. A Cargo subcommand that checks the lock file for
  advisories, licences, duplicate versions and sources, configured by
  `deny.toml` at the workspace root.
  [cargo-deny](https://embarkstudios.github.io/cargo-deny/)
- **`cargo about`**. A Cargo subcommand that gathers the licence of
  every crate in the graph and renders a Handlebars template, here
  `about.hbs`, with the result.
  [cargo-about](https://github.com/EmbarkStudios/cargo-about)
- **`typos`**. A spell checker for source trees that flags common
  misspellings and takes its exclusions and accepted words from
  `_typos.toml`.
  [typos](https://github.com/crate-ci/typos)
- **`cargo package` and the `exclude` field**. `cargo package` builds
  the `.crate` archive crates.io receives and verifies that it builds;
  `exclude` in `provkit/Cargo.toml` takes gitignore-style globs and
  removes matching files from the archive.
  [cargo package](https://doc.rust-lang.org/cargo/commands/cargo-package.html),
  [The exclude and include fields](https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields)
- **Cross-compilation targets**. A target triple names the platform the
  compiler emits code for; the standard library for a target other than
  the host is installed with `rustup target add`, and `--target` on a
  Cargo command selects it, as the wasm32 step in `ci.yml` does.
  [Platform support](https://doc.rust-lang.org/rustc/platform-support.html)
- **`cargo binstall` and release archives**. The
  `[package.metadata.binstall]` table in `provkit-cli/Cargo.toml` tells
  `cargo binstall` where a release archive for a version and target
  lives and where the binary sits inside it, with names that match the
  ones the release workflow uploads.
  [cargo-binstall](https://github.com/cargo-bins/cargo-binstall)

## Run this

```sh
cargo deny check
typos
```

The first command runs the checks of `deny.toml` over
`Cargo.lock` and ends with one line naming each.

```text
advisories ok, bans ok, licenses ok, sources ok
```

The second reads every file in the repository that is not ignored,
the corpus excluded, and prints nothing when it finds no misspelling,
exiting 0. These two and the `wasm32-unknown-unknown` build are the
checks that run in CI only, and `cargo build -p provkit --target
wasm32-unknown-unknown` also passes once
`rustup target add wasm32-unknown-unknown` has run.

## Try it

`_typos.toml` accepts a misspelt PROV-N keyword that one test needs.
Write a file under `/tmp` containing that keyword and run `typos` on
it with no configuration beside it. Then copy `_typos.toml` into the
file's directory and run the same command again. The commands below
run from the repository root, where `_typos.toml` is. Which line
accepts the word, and why does the second run read the copy?

```sh
mkdir -p /tmp/typos-try
printf 'entty(ex:e1)\n' > /tmp/typos-try/probe.provn
typos /tmp/typos-try/probe.provn
cp _typos.toml /tmp/typos-try/
typos /tmp/typos-try/probe.provn
```

<details><summary>Answer</summary>

The first run reports the misspelling and exits 2, the code `typos`
uses when it finds one.

```text
error: `entty` should be `entry`, `entity`
  ╭▸ /tmp/typos-try/probe.provn:1:1
  │
1 │ entty(ex:e1)
  ╰╴━━━━━
```

The second run prints nothing and exits 0. The line that accepts the
word is `entty = "entty"` under `[default.extend-words]`, a misspelt
keyword that must be a syntax error in the extension tests. `typos`
looks for a `_typos.toml` in the
directory of each file it checks and in the directories above, so the
copy beside `probe.provn` applies to it, as the file at the workspace
root applies to every file in the repository. Delete the copy, or the
line from it, and the first output returns.

</details>

## Idioms first seen here

- `Cargo profiles`: https://doc.rust-lang.org/cargo/reference/profiles.html
- `[workspace.lints] and clippy lint groups`: https://doc.rust-lang.org/cargo/reference/workspaces.html#the-lints-table
- `cargo deny`: https://embarkstudios.github.io/cargo-deny/
- `cargo about`: https://github.com/EmbarkStudios/cargo-about
- `typos`: https://github.com/crate-ci/typos
- `cargo package and the exclude field`: https://doc.rust-lang.org/cargo/commands/cargo-package.html
- `cross-compilation targets`: https://doc.rust-lang.org/rustc/platform-support.html
- `cargo binstall and release archives`: https://github.com/cargo-bins/cargo-binstall
