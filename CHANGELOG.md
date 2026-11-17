# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-09-30

The first release.

- `provkit`, a library for the W3C PROV data model: a `Document` of `Record`s
  and named `Bundle`s, readers and writers for PROV-JSON and PROV-N (the
  `json` and `provn` modules, each behind a default feature), set-based
  document equality, and `Document::unified()`, which merges records that
  share an identifier and kind under PROV-CONSTRAINTS and keeps each record
  once. The API reference is on [docs.rs](https://docs.rs/provkit/0.1.0);
  [CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md)
  lists where the library departs from the Recommendations, where it differs
  from ProvPy 3.3.0 and ProvToolbox 2.2.5, and its implementation limits,
  each with its reason and test.
  - A `hadMember`, `alternateOf`, `specializationOf` or `mentionOf` without a
    mandatory party is an error in `Record::from_parts`, both readers, both
    writers and `unified()`. Under `Strict`, `prov:mentionOf` with `-` for any
    of its three arguments is refused, with or without an identifier.
  - A PROV-N `\U` escape below U+10000 decodes. A surrogate or a value above
    U+10FFFF stays a syntax error.
  - A namespace IRI that is empty or all whitespace is an error.
    `Namespaces::declare`, `Namespaces::set_default`,
    `Namespaces::ensure_prefix` and `QName::new` refuse it, and both readers
    refuse `prefix z <>`, `default <>` and a PROV-JSON `""` or `" "`.
    `set_default` and `ensure_prefix` return a `Result`, and
    `Error::EmptyNamespace` names the prefix or the default namespace at
    fault.
- `provkit-cli`, the `provkit` command: `convert` between the two formats and
  `compare` two documents, with prebuilt binaries for macOS, Linux and
  Windows on each release. The command reference is in the
  [README](https://github.com/trungdong/provkit#command-line).
- A contributor guide under `book/`, built with mdBook and published at
  <https://trungdong.github.io/provkit/>, which reads the source one layer
  at a time with live snippets and a glossary.

Not in this release: a lenient PROV-N profile, a builder API, an index or
store, PROV-JSONLD, PROV-XML, PROV-O, and validation of PROV-CONSTRAINTS.

[0.1.0]: https://github.com/trungdong/provkit/releases/tag/v0.1.0
