# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-09-30

The first release.

- `provkit`, a library for the W3C
  [PROV data model](https://www.w3.org/TR/2013/REC-prov-dm-20130430/):
  a `Document` of `Record`s
  and named `Bundle`s, readers and writers for
  [PROV-JSON](https://www.w3.org/Submission/2013/SUBM-prov-json-20130424/)
  and
  [PROV-N](https://www.w3.org/TR/2013/REC-prov-n-20130430/)
  (the
  `json` and `provn` modules, each behind a default feature), set-based
  document equality, and `Document::unified()`, which merges records that
  share an identifier and kind under
  [PROV-CONSTRAINTS](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/)
  and keeps each record
  once. The API reference is on [docs.rs](https://docs.rs/provkit/0.1.0);
  [CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md)
  lists where the library departs from the Recommendations, where it differs
  from
  [Python `prov`](https://github.com/trungdong/prov)
  3.3.1 and
  [ProvToolbox](https://lucmoreau.github.io/ProvToolbox/)
  2.5.0, and its implementation limits,
  each with its reason and test.
- `provkit-cli`, the `provkit` command: `convert` between the two formats and
  `compare` two documents, with prebuilt binaries for macOS, Linux and
  Windows on each release. The command reference is in the
  [README](https://github.com/trungdong/provkit#command-line).
- A contributor guide under `book/`, built with mdBook and published at
  <https://trungdong.github.io/provkit/>, which reads the source one layer
  at a time with live snippets and a glossary.

[0.1.0]: https://github.com/trungdong/provkit/releases/tag/v0.1.0
