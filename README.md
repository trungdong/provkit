# provkit

W3C PROV data model in Rust, with PROV-JSON and PROV-N read and write, and a
`provkit` command-line tool to convert and compare documents.

Status: the first release. The minimum supported Rust version is 1.98.

## Install

```sh
cargo install --locked provkit-cli
cargo binstall provkit-cli
```

The second command fetches a release binary. The binary is `provkit`.
Prebuilt binaries are on the
[Releases](https://github.com/trungdong/provkit/releases) page. Each release
carries one archive per target, with a digest file named after the archive
(`provkit-<target>.sha256`) and a build-provenance attestation:

- `provkit-x86_64-apple-darwin.tar.gz`
- `provkit-aarch64-apple-darwin.tar.gz`
- `provkit-x86_64-unknown-linux-gnu.tar.gz`
- `provkit-x86_64-unknown-linux-musl.tar.gz`
- `provkit-aarch64-unknown-linux-gnu.tar.gz`
- `provkit-x86_64-pc-windows-msvc.zip`

Each archive holds the binary and `THIRD-PARTY.md`, the licence notices of
the crates it links.

## Command line

```sh
# PROV-JSON to PROV-N on standard output (the default output format)
provkit convert doc.json

# PROV-N to PROV-JSON, by the output extension (.json or .provn)
provkit convert doc.provn -o doc.json

# Standard input to standard output, with the output format named
provkit convert -t json - < doc.provn

# Print the records found on one side only
provkit compare a.json b.provn

# Merge records that share an identifier on each side first
provkit compare --unified a.json b.provn

# Name the format of both sides
provkit compare -f json a.txt b.txt
```

Input format comes from `-f json|provn` (on `compare`, for both sides), else
the file extension, else the content. The sniffer treats `document` followed by whitespace, a comment or
the end of input as PROV-N, and `{` as PROV-JSON. A comment glued to the
keyword is part of the name in PROV-N, so the reader then rejects it; the
departures in CONFORMANCE.md say why. Output format comes from `-t`, else
the `-o` extension (`.json` or `.provn`, in any letter case). PROV-N is the
default on standard output only, so `-o out.txt` needs `-t json` or
`-t provn`. `-` means standard input or output.

`compare` declares the `nsN` prefixes it mints for names with no declared
prefix, with a `prefix nsN <IRI>` line after the records of each scope. Such a
line applies to the whole scope, including the records printed before it. A
PROV-N file with extensibility expressions prints one warning on standard
error and the exit code does not change. A closed output pipe ends the program
quietly.

| Exit code | Meaning |
|---|---|
| 0 | `convert` succeeded, or `compare` found the documents equal |
| 1 | `compare` found a difference |
| 2 | Any error: a usage error, a file that cannot be read or written, a format that cannot be told, a syntax error, a document the output format cannot hold, or with `--unified` a unification failure |

An error with a position, in either format, is printed as `file:line:column: message`. `compare` takes `-` for standard input on one side.

## Library

```rust
use provkit::{json, provn, Attributes, Entity, QName, Record};

fn main() -> Result<(), provkit::Error> {
    let text = r#"{"prefix": {"ex": "http://example.org/"}, "entity": {"ex:e1": {}}}"#;
    let mut doc = json::read(text)?;
    doc.records.push(Record::Entity(Entity {
        id: QName::new("http://example.org/", "e2")?,
        attributes: Attributes::new(),
    }));
    let out = provn::to_string(&doc)?;
    assert!(out.contains("entity(ex:e2)"));
    Ok(())
}
```

Document equality is set-based. `Document::unified()` merges records that
share an identifier and kind, keeps each record once, and fails where
PROV-CONSTRAINTS 53 to 55 forbid the sharing, where formal attributes
conflict, or where an absent value in one of the four non-expandable positions
of PROV-CONSTRAINTS Table 3 meets a present one. `provn::read_with` takes a
`provn::Profile`. `Strict` accepts the whole PROV-N grammar and the MUST
statements of its prose, with the departures
[CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md) records, and `Default` (the profile
`provn::read` uses) adds the extensions ProvPy and ProvToolbox write, with the
exceptions that file lists. `provn::read_report` returns a `ReadReport` with the document and
the expressions left out. API documentation: `cargo doc --open -p provkit`.

## Conformance

[CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md) lists where provkit departs
from the PROV Recommendations, where it differs from ProvPy 3.2.2 (with the
ProvPy issue number) and from ProvToolbox 2.0.4, and its implementation limits,
each with its reason and test.
[CHANGELOG.md](https://github.com/trungdong/provkit/blob/main/CHANGELOG.md) lists the releases.

## Tests and CI

The tests read the [prov-corpus](https://github.com/trungdong/prov-corpus)
submodule at `corpus/`, shared with ProvPy (`prov`, Python). Clone with
`--recurse-submodules`, or run `git submodule update --init`.

The corpus is a git submodule holding examples extracted and modified from the
PROV-N and PROV-DM Recommendations, under the
[W3C Document Licence](https://www.w3.org/copyright/document-license/).

> Copyright © 2012-2013 W3C® (MIT, ERCIM, Keio, Beihang). This software or
> document includes material copied from or derived from PROV-N: The
> Provenance Notation, W3C Recommendation 30 April 2013,
> <https://www.w3.org/TR/2013/REC-prov-n-20130430/>.

> Copyright © 2011-2013 W3C® (MIT, ERCIM, Keio, Beihang). This software or
> document includes material copied from or derived from PROV-DM: The PROV
> Data Model, W3C Recommendation 30 April 2013,
> <https://www.w3.org/TR/2013/REC-prov-dm-20130430/>.

The published crates do not include the corpus.

```sh
cargo test --workspace
```

CI runs the tests on Linux and Windows, and the remaining checks on Linux.
Releases follow [RELEASING.md](https://github.com/trungdong/provkit/blob/main/RELEASING.md).

Three checks run in CI only: the `wasm32-unknown-unknown` build, `cargo deny`
for supply-chain audits, and `typos` for spell checking. To run them locally,
install the tools with `cargo install cargo-deny typos-cli` and add the wasm32
target with `rustup target add wasm32-unknown-unknown`.

## Licence

MIT OR Apache-2.0, at your option. See [LICENSE-MIT](https://github.com/trungdong/provkit/blob/main/LICENSE-MIT) and
[LICENSE-APACHE](https://github.com/trungdong/provkit/blob/main/LICENSE-APACHE).
