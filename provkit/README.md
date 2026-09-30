# provkit

A Rust library for the [W3C PROV](https://www.w3.org/TR/prov-overview/) data
model, with readers and writers for PROV-JSON and PROV-N.

A `Document` holds `Record`s, one enum variant per PROV-DM type, and named
`Bundle`s. Readers and writers are free functions in the `json` and `provn`
modules. They take and return text, so the library never opens a file.

## Example

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

`==` on `Document` is set-based. It ignores record order, attribute order,
namespace tables and the spelling of a number, boolean or `dateTime`.
`Document::unified()` merges records that share an identifier.

## Features

| Feature | Default | Enables |
|---|---|---|
| `json` | yes | The `json` module, with `serde` and `serde_json` |
| `provn` | yes | The `provn` module |

With both features off, the crate builds the model alone and depends on no
serialisation library.

## Minimum supported Rust version

Rust 1.98. The CI checks this version on every change.

## Command line

The [`provkit-cli`](https://crates.io/crates/provkit-cli) crate installs a
`provkit` binary that converts and compares PROV documents:

```sh
cargo install --locked provkit-cli
```

See the [repository](https://github.com/trungdong/provkit) for the command
reference, the conformance notes and the changelog.

## Licence

MIT OR Apache-2.0, at your option.
