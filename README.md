# provkit

W3C PROV data model in Rust, with PROV-JSON and PROV-N read and write, and a
`provkit` command-line tool to convert and compare documents.

Status: 0.1.0, the first release.

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

# PROV-N to PROV-JSON, by the output extension
provkit convert doc.provn -o doc.json

# Standard input to standard output, with the output format named
provkit convert -t json - < doc.provn

# Print the records found on one side only
provkit compare a.json b.provn

# Merge records that share an identifier on each side first
provkit compare --unified a.json b.provn
```

Input format comes from `-f json|provn`, else the file extension, else the
content. Output format comes from `-t`, else the `-o` extension, else PROV-N
on standard output. `-` means standard input or output.

| Exit code | Meaning |
|---|---|
| 0 | `convert` succeeded, or `compare` found the documents equal |
| 1 | `compare` found a difference |
| 2 | A read or write error, or with `--unified` a unification failure |

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
share an identifier. API documentation: `cargo doc --open -p provkit`.

## Conformance

[CONFORMANCE.md](CONFORMANCE.md) lists every departure from the PROV
Recommendations, ProvPy and ProvToolbox, with its reason and test.
[CHANGELOG.md](CHANGELOG.md) lists the releases.

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

MIT OR Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
