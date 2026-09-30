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
    No typed literal is rejected for its text. `Value::typed` and both readers
    keep an ill-formed one as `Value::Typed`, and each value is written with
    the text it was read with. A `dateTime` in a formal position must still
    parse, and `Value::typed` still refuses a qualified-name datatype.
  - `Value` compares and hashes by value under one datatype. Spellings of
    one integer (`"5"`, `" 5 "`, `"+5"`, `"005"`), decimal, `xsd:double`,
    `xsd:float` or `xsd:boolean` are equal, so `Attributes` keeps one pair
    for them and `Document` equality ignores the spelling. Ill-formed text
    and every other datatype compare by text, and values of different
    datatypes are never equal.
  - Both readers keep a qualified-name literal (`xsd:QName`,
    `prov:QUALIFIED_NAME`) whose text spells no qualified name as
    `Value::Typed`. Text that spells one must resolve, as before.
  - `LangStr::new` accepts a tag of the grammar's `LANGTAG` shape, letters
    then any number of `-` and letters or digits, with no limit on the length
    of a subtag.
  - `well_known` names every XSD datatype compared by value: `XSD_DECIMAL`,
    `XSD_SHORT`, `XSD_BYTE`, `XSD_NON_NEGATIVE_INTEGER`,
    `XSD_POSITIVE_INTEGER`, `XSD_NON_POSITIVE_INTEGER`,
    `XSD_NEGATIVE_INTEGER`, `XSD_UNSIGNED_LONG`, `XSD_UNSIGNED_INT`,
    `XSD_UNSIGNED_SHORT` and `XSD_UNSIGNED_BYTE` join the names already
    there.
  - `DateTime` follows XSD 1.1: year `0000` exists and is a leap year, and
    `-0000` is the same year. `DateTime::new` accepts only values whose
    printed text `DateTime::parse` reads back, and its errors carry the text
    of the rejected components. `DateTime` has the accessors `year`, `month`,
    `day`, `hour`, `minute`, `second`, `nanosecond` and `offset_minutes`, and
    implements `FromStr`.
  - PROV-JSON read and write (`json` module).
  - PROV-N read and write (`provn` module), with the `Strict` and `Default`
    read profiles.
  - The PROV-N reader lexes the terminals of the Recommendation and SPARQL
    1.0. A name may start with `@`, a `\` in a local part escapes only the
    eleven characters of `PN_CHARS_ESC`, and `/*/` opens a comment that is
    not closed. `\uXXXX` and `\UXXXXXXXX` (upper-case hex digits) decode to a
    code point in string literals and IRIs, and a surrogate, a value above
    `10FFFF` or a character `IRI_REF` forbids is a syntax error.
  - `Profile::Strict` accepts exactly the PROV-N grammar. It rejects `default`
    after `prefix` (production [45]), a statement after a bundle (production
    [1]), an identifier or attributes on `alternateOf`, `specializationOf`
    and `hadMember` (productions [24] to [26]), and a redeclaration of the
    predeclared `prov` and `xsd` prefixes (section 3.7.4), as well as bare
    `mentionOf` and `-` in a required position. `Profile::Default` reads all
    of these.
  - A datatype of digits, as in `"x" %% 123`, reads as a name in the default
    namespace.
  - Both readers type an integer by range: `xsd:int` within `i32`, `xsd:long`
    within `i64`, `xsd:integer` beyond, keeping the text. This departs from
    PROV-N section 3.7.3, which calls an integer literal `xsd:int`, and makes
    `[ex:k=3000000000]` equal to the PROV-JSON `{"ex:k": 3000000000}`. The
    PROV-N writer writes a value bare when its text is `-?[0-9]+` and reads
    back as the same datatype, so `"5" %% xsd:long` is written quoted.
  - `json::read`, `provn::read` and `provn::read_with` ignore one leading
    byte-order mark.
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
