# provkit

A Rust workspace with two crates. `provkit` is the library: the W3C PROV
data model, readers and writers for PROV-JSON and PROV-N, set-based
document equality, and unification of records that share an identifier.
`provkit-cli` is the binary `provkit`, which converts between the formats
and compares documents. `corpus/` is a git submodule shared with ProvPy.

## Where things are recorded

- `README.md` and `provkit/README.md`: how to use the CLI and the library.
- `CHANGELOG.md`: every user-visible change, in Keep a Changelog form.
- `CONFORMANCE.md`: every departure from the Recommendations, every
  difference from ProvPy and ProvToolbox, and every implementation limit.
  Each row names the test that pins it.
- `RELEASING.md`: the release steps. The owner runs them.
- `book/`: the contributor guide, built with mdBook.
- The rustdoc, built with `cargo doc --workspace --no-deps`.

## Conformance

- The grammar decides. Cite the PROV-N production, or the PROV-DM,
  PROV-CONSTRAINTS or PROV-JSON section, in the comment and in the
  CONFORMANCE.md row.
- Do not deviate from ProvPy 3.3.0 without a CONFORMANCE.md row that
  gives the reason and the issue number on trungdong/prov.
- A change to what a reader accepts or a writer emits updates
  CONFORMANCE.md and the test its row cites in the same commit.
- When the grammar or a Recommendation leaves a question open, raise it
  before writing code.

## Code

- The library reads `&str` and writes to `impl std::io::Write`. It never
  touches a file. The CLI owns files, format detection and exit codes.
- Features `json` and `provn` are both on by default, and the library
  must build and test with either one alone.
- Unit tests sit in `tests.rs` beside each module. Corpus tests live in
  `provkit/tests`, with every excluded file and known difference listed
  in `provkit/tests/data/*.tsv` with a reason.
- `corpus/` is read-only here. A corpus change goes to trungdong/prov-corpus
  and arrives as a submodule bump.

## Checks before a commit

    cargo fmt --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace --all-features
    RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps
    cargo deny check
    typos

Accepted spellings go in `_typos.toml` with a comment saying why.

## Commits

- The subject is one imperative sentence saying what changed. The body
  says why, in prose, and names the row, section or issue it rests on.
- Every message ends with a `Rust notes:` trailer listing the idioms the
  change introduces, each with a link to the Rust book, the reference or
  the crate docs, or `Rust notes: none.`
- No attribution lines.

## The book

- A change that touches code between `ANCHOR` comments, changes
  behaviour a stop page describes, or introduces an idiom updates the
  matching page under `book/src` in the same commit.
- `book/build.sh` builds the book and runs its doctests. CI runs the same.
- The idiom index is generated from the `Rust notes:` trailers.

## Release steps

- Pushing `main`, tagging, publishing and anything in `RELEASING.md` are
  the owner's steps. Do not run them unless asked.
