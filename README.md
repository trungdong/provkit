# provkit

W3C PROV data model in Rust, with PROV-JSON and PROV-N read and write, and a
`provkit` command-line tool to convert and compare documents.

Status: 0.1.0, the first release. The minimum supported Rust version is 1.98.

## Install

```sh
cargo install --locked provkit-cli
```

The binary is `provkit`. Prebuilt binaries are on the
[Releases](https://github.com/trungdong/provkit/releases) page.

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
```

Input format comes from `-f json|provn`, else the file extension, else the
content. The sniffer treats `document` followed by whitespace, a comment or
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
| 2 | A read or write error, an output format that cannot be told, or with `--unified` a unification failure |

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
share an identifier and kind, and fails where PROV-CONSTRAINTS 53 to 55 forbid
the sharing or where formal attributes conflict. `provn::read_with` takes a
`provn::Profile`. `Strict` accepts the whole PROV-N grammar and the MUST
statements of its prose, with the departures CONFORMANCE.md records, and
`Default` (the profile `provn::read` uses) also accepts what ProvToolbox and
ProvPy write. `provn::read_report` also lists the extensibility expressions
the reader left out. API documentation: `cargo doc --open -p provkit`.

## Conformance

[CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md) lists where provkit departs from the PROV
Recommendations, where it differs from ProvPy 3.2.2 (with the ProvPy issue
number) and its implementation limits, each with its reason and test.
[CHANGELOG.md](https://github.com/trungdong/provkit/blob/main/CHANGELOG.md) lists the releases.

## Tests and CI

The tests read the [prov-corpus](https://github.com/trungdong/prov-corpus)
submodule at `corpus/`, shared with ProvPy (`prov`, Python). Clone with
`--recurse-submodules`, or run `git submodule update --init`.

```sh
cargo test --workspace
```

Three checks run in CI only: the `wasm32-unknown-unknown` build, `cargo deny`
for supply-chain audits, and `typos` for spell checking. To run them locally,
install the tools with `cargo install cargo-deny typos-cli` and add the wasm32
target with `rustup target add wasm32-unknown-unknown`.

## Licence

MIT OR Apache-2.0, at your option. See [LICENSE-MIT](https://github.com/trungdong/provkit/blob/main/LICENSE-MIT) and
[LICENSE-APACHE](https://github.com/trungdong/provkit/blob/main/LICENSE-APACHE).
