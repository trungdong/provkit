# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-09-30

The first release. It has no `lenient` PROV-N profile, fluent builder API,
index, query or store, and no PROV-JSONLD, PROV-XML, PROV-O or validation.

### Added

- `provkit` library:
  - PROV-DM model with a `Record` enum and a struct per PROV-DM type.
  - Every relation formal attribute is optional.
  - `Kind`, the type of a record. `Kind::name` is both the PROV-N keyword and
    the PROV-JSON key, `Kind::from_name` is its inverse, and `Kind::ALL` lists
    the eighteen kinds in PROV-DM order. `Kind::is_element` tells an element
    (`entity`, `activity`, `agent`) from a relation.
  - `Record::formal` reads one formal attribute and `Record::formals` lists
    those present. `FormalRef::to_formal`, and `From<FormalRef>` for
    `Formal`, give an owned `Formal`, and `FormalAttr::ALL` lists every
    formal attribute.
  - `Kind`, `Record`, `FormalAttr`, `Formal`, `FormalRef`, `Error`,
    `provn::Profile`, `provn::Ignored` and `provn::ReadReport` are
    `#[non_exhaustive]`, so a `match` on any of them in another crate needs
    a wildcard arm.
  - `Attributes` clones share one allocation and copy it on the first write.
    Its `Hash` sorts the per-pair hashes, `Attributes::remove` keeps the order
    of the rest, and it implements `FromIterator`, `Extend` and `IntoIterator`
    (owned and by reference). `Attributes::iter` returns `AttributesIter`,
    and `into_iter` returns `AttributesIntoIter`; both are the crate's own
    types, so no dependency is part of the public API.
  - Strict name resolution with `QName` and `Namespaces`. `QName::new`
    returns `Result`, and both readers reject a name whose local part PROV-N
    cannot write wherever a name appears (a PROV-JSON record, bundle or
    attribute key, a name formal, a member of `hadMember`, a datatype, and
    the text of a qualified-name literal), with `Error::InvalidQName`. A
    local part is unwritable when it holds a character that `PN_LOCAL`
    admits nowhere, bare or after `\`: a space or another separator, a
    control character, a symbol outside `PN_CHARS` such as `£` or `×`, one
    of `{ } < > " | ^ \` or a backtick, a `%` not followed by two hex digits,
    or a first character that may only continue a name.
    `Namespaces::resolve` puts the whole text it was given in
    `Error::InvalidQName`, for example `ex:a b`; `QName::new` puts the local
    part there.
  - `Value` with plain, language-tagged, name, `dateTime` and typed literals.
    No typed literal is rejected for its text, except under a qualified-name
    datatype. `Value::typed` and both readers keep an ill-formed one as
    `Value::Typed`, and each value is written with the text it was read
    with. A `dateTime` in a formal position must parse. `Value::typed`
    refuses a qualified-name datatype, because the value must resolve to a
    `Value::QName`.
  - `Value::typed` trims the space, tab, CR and LF around an `xsd:dateTime`
    before parsing it, as XSD does for numbers, and the PROV-JSON reader
    trims a time formal the same way. A padded well-formed `dateTime` is a
    `Value::DateTime` and is written without the padding.
  - `Value` compares and hashes by value under one datatype. Spellings of
    one integer (`"5"`, `" 5 "`, `"+5"`, `"005"`), decimal, `xsd:double`,
    `xsd:float` or `xsd:boolean` are equal, so `Attributes` keeps one pair
    for them and `Document` equality ignores the spelling. Ill-formed text
    and every other datatype compare by text, and values of different
    datatypes are never equal.
  - `Value::Typed` holds a `TypedLiteral`, whose text and datatype are read
    through `lexical()` and `datatype()` and cannot be set from outside the
    crate. The XSD integer datatypes with a range compare by value inside the
    range and by text outside it.
  - Both readers resolve every qualified-name literal (`xsd:QName`,
    `prov:QUALIFIED_NAME`) to a `Value::QName`. Text that spells no
    qualified name is `Error::InvalidQName`; an undeclared prefix or a bare
    name with no default namespace is an error.
  - The PROV-JSON reader reads a formal written as a value object only when
    it has no `lang` and its `type` resolves to `xsd:QName` or
    `prov:QUALIFIED_NAME` for a name, or to `xsd:dateTime` for a time. Any
    other shape is `Error::JsonStructure` naming the formal.
  - `LangStr::new` accepts a tag of the grammar's `LANGTAG` shape, letters
    then any number of `-` and letters or digits, with no limit on the length
    of a subtag.
  - `well_known` names every XSD datatype compared by value, among them
    `XSD_DECIMAL`, `XSD_SHORT`, `XSD_BYTE`, `XSD_NON_NEGATIVE_INTEGER`,
    `XSD_POSITIVE_INTEGER`, `XSD_NON_POSITIVE_INTEGER`,
    `XSD_NEGATIVE_INTEGER`, `XSD_UNSIGNED_LONG`, `XSD_UNSIGNED_INT`,
    `XSD_UNSIGNED_SHORT` and `XSD_UNSIGNED_BYTE`.
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
    code point in string literals and IRIs: `\u` up to `FFFF` and `\U` from
    `10000`. A surrogate, a value above `10FFFF`, a `\U` below `10000` or a
    character `IRI_REF` forbids is a syntax error.
  - `Profile::Strict` accepts the whole PROV-N grammar and the MUST
    statements of its prose, with the departures `CONFORMANCE.md` records.
    It rejects `default` after `prefix` (production
    [45]), a statement after a bundle (production [1]), an identifier or
    attributes on `alternateOf`, `specializationOf` and `hadMember`
    (productions [24] to [26]), an identifier on `mentionOf`, a declaration
    of `prov` or `xsd` at its own IRI (section 3.7.4), bare `mentionOf` and
    `-` in a required position. `Profile::Default` reads all of these. Both
    profiles reject a second `default` and a prefix that is not a
    `PN_PREFIX`.
  - `prov:mentionOf` is a mention only when three names follow it, with or
    without an attribute block. With any other arguments it is an
    extensibility expression; under `Strict` an identifier or a `-` also
    makes it one, and under `Default` both read as for bare `mentionOf`.
  - A datatype of digits, as in `"x" %% 123`, reads as a name in the default
    namespace.
  - Both readers type an integer by range: `xsd:int` within `i32`, `xsd:long`
    within `i64`, `xsd:integer` beyond, keeping the text. This departs from
    PROV-N section 3.7.3, which calls an integer literal `xsd:int`, and makes
    `[ex:k=3000000000]` equal to the PROV-JSON `{"ex:k": 3000000000}`. The
    PROV-N writer writes a value bare when its text is `-?[0-9]+` and reads
    back as the same datatype, so `"5" %% xsd:long` is written quoted.
  - The PROV-N reader parses every extensibility expression (section 5,
    productions [49] to [51]) under both profiles and leaves it out of the
    document. Names inside one are not resolved and literals inside one are
    not built, but its tokens must match the productions, and nesting deeper
    than 64 levels of expressions and tuples is a syntax error.
  - `provn::read_report`, which returns a `provn::ReadReport` holding the
    document and the extensibility expressions it left out, one per outermost
    expression in source order.
  - `provn::Ignored`, an entry in that list: the expression's name as written
    and the line and column of the name.
  - `json::read`, `provn::read`, `provn::read_with` and `provn::read_report`
    ignore one leading byte-order mark.
  - Set-based `Document` equality.
  - `Document::merged_bundles()`, which reads bundles that share an
    identifier as one, as equality and both writers do. It borrows the
    document's bundles when every identifier is unique. `Document::bundle`
    returns the first bundle with an identifier.
  - `provn::statement` writes one record and `provn::name` one name. Both
    take `&mut Namespaces`, so a prefix they mint stays with the caller and a
    second call for the same namespace reuses it.
  - `Document::unified()`, which merges records by identifier and kind and
    reports conflicts as `Error::Unification`. It rejects the identifier
    sharing that PROV-CONSTRAINTS forbids and no other. Constraint 53 makes
    `used`, `wasGeneratedBy`, `wasInvalidatedBy`, `wasStartedBy`,
    `wasEndedBy`, `wasInformedBy`, `wasAttributedTo`, `wasAssociatedWith` and
    `actedOnBehalfOf` pairwise disjoint. Constraint 54 keeps entity, activity
    and agent identifiers apart from those nine, `wasInfluencedBy` and
    `wasDerivedFrom`. Constraint 55 makes `entity` and `activity` disjoint
    and lets an agent be either. `wasInfluencedBy` and `wasDerivedFrom` may
    share an identifier with any relation. `alternateOf`, `specializationOf`,
    `mentionOf` and `hadMember` records are never merged.
  - `Error`, with one variant and a typed payload per kind of failure. Every
    error from the PROV-N reader carries a line and column: `Error::Syntax`
    for text the grammar rejects, and `Error::At` around the error for a
    name, literal or record. `Error::DuplicateBundle` reports two bundles
    with one identifier in either reader.
  - Public modules `json`, `provn` and `well_known`. Every other public item
    has one path. It is at the crate root, except `provn::Profile` and
    `provn::Ignored`, which live under `provn`. `well_known` holds the
    `QName` statics and the `PROV` and `XSD` namespace IRIs.
  - Namespace arguments and results are `&str`.
  - Cargo features `json` and `provn`, both on by default.
- `provkit` binary (crate `provkit-cli`):
  - `provkit convert` between PROV-JSON and PROV-N, with pandoc-style format
    resolution (`-f`, `-t`, extension, content sniffing) and `-` for
    standard input and output.
  - `provkit compare`, with `--unified`, exit codes 0, 1 and 2, and one-sided
    records printed as PROV-N. Each scope renders against one namespace
    table and declares the `nsN` prefixes it minted, after its records.
  - A PROV-N file with ignored extensibility expressions prints one warning
    on standard error. The exit code does not change.
  - `-o` with an extension other than `.json` or `.provn` needs `-t`. PROV-N
    is the default on standard output only.
  - A closed output pipe ends the program quietly, with the exit code the
    command had reached.
  - Content sniffing takes `document` only as a whole word, so `documentary`
    is not PROV-N.
- Prebuilt `provkit` binaries on each GitHub release, one archive per target:
  - `provkit-x86_64-apple-darwin.tar.gz`
  - `provkit-aarch64-apple-darwin.tar.gz`
  - `provkit-x86_64-unknown-linux-gnu.tar.gz`
  - `provkit-x86_64-unknown-linux-musl.tar.gz`
  - `provkit-aarch64-unknown-linux-gnu.tar.gz`
  - `provkit-x86_64-pc-windows-msvc.zip`

  `cargo binstall provkit-cli` reads the same names.
- `provkit` builds without `serde` when the `json` feature is off, and the
  crate has its own README.
- `CONFORMANCE.md`, listing the departures from the Recommendations, the
  differences from ProvPy 3.2.2 with their issue numbers, and the
  implementation limits, each with its test.
- Corpus tests over prov-corpus v2.1.0, including the 204 shared PROV-N
  probes read under both profiles, and a CI build for
  `wasm32-unknown-unknown`.

[0.1.0]: https://github.com/trungdong/provkit/releases/tag/v0.1.0
