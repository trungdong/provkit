# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-09-30

### Added

- `provkit` library:
  - PROV-DM model with a `Record` enum and a struct per PROV-DM type.
  - Every relation formal attribute is optional.
  - `Kind`, the type of a record. `Kind::name` is both the PROV-N keyword and
    the PROV-JSON key, `Kind::from_name` is its inverse, and `Kind::ALL` lists
    the eighteen kinds in PROV-DM order.
  - `Record::formal` reads one formal attribute and `Record::formals` lists
    those present. `FormalRef::to_owned` gives an owned `Formal`, and
    `FormalAttr::ALL` lists every formal attribute.
  - `Kind` and `Record` are `#[non_exhaustive]`, so a `match` on either in
    another crate needs a wildcard arm.
  - `Attributes` clones share one allocation and copy it on the first write.
    Its `Hash` sorts the per-pair hashes, `Attributes::remove` keeps the order
    of the rest, and it implements `FromIterator`, `Extend` and `IntoIterator`
    (owned and by reference).
  - Strict name resolution with `QName` and `Namespaces`.
  - `Value` with plain, language-tagged, name, `dateTime` and typed literals.
  - `DateTime` follows XSD 1.1: year `0000` exists and is a leap year, and
    `-0000` is the same year. `DateTime::new` accepts only values whose
    printed text `DateTime::parse` reads back, and its errors carry the text
    of the rejected components. `DateTime` has the accessors `year`, `month`,
    `day`, `hour`, `minute`, `second`, `nanosecond` and `offset_minutes`, and
    implements `FromStr`.
  - PROV-JSON read and write (`json` module).
  - PROV-N read and write (`provn` module), with the `Strict` and `Default`
    read profiles.
  - Set-based `Document` equality.
  - `Document::unified()`, which merges records by identifier and kind and
    reports conflicts as `Error::Unification`.
  - `Error`, with one variant and a typed payload per kind of failure. Every
    error from the PROV-N reader carries a line and column: `Error::Syntax`
    for text the grammar rejects, and `Error::At` around the error for a
    name, literal or record.
  - Public modules `json`, `provn` and `well_known`. Every other public item
    has one path, at the crate root, and `provn::Profile` lives under `provn`.
    `well_known` holds the `QName` statics and the `PROV` and `XSD`
    namespace IRIs.
  - Signatures take and return `&str` for namespaces, never `Arc<str>`.
  - Cargo features `json` and `provn`, both on by default.
- `provkit` binary (crate `provkit-cli`):
  - `provkit convert` between PROV-JSON and PROV-N, with pandoc-style format
    resolution (`-f`, `-t`, extension, content sniffing) and `-` for
    standard input and output.
  - `provkit compare`, with `--unified`, exit codes 0, 1 and 2, and one-sided
    records printed as PROV-N.
- `CONFORMANCE.md`, listing every departure from the Recommendations, ProvPy
  and ProvToolbox.
- Corpus tests over prov-corpus v2.0.0, and a CI build for
  `wasm32-unknown-unknown`.

### Not included

- `lenient` PROV-N profile, fluent builder API, index, query and store.
- PROV-JSONLD, PROV-XML, PROV-O and validation.

[0.1.0]: https://github.com/trungdong/provkit/releases/tag/v0.1.0
