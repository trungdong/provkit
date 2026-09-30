# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-09-30

### Added

- `provkit` library:
  - PROV-DM model with a `Record` enum and a struct per PROV-DM type.
  - Every relation formal attribute is optional.
  - Strict name resolution with `QName`, `Namespaces` and `Scope`.
  - `Value` with plain, language-tagged, name, `dateTime` and typed literals.
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
