# Conformance

Where provkit 0.1.0 departs from the W3C PROV Recommendations, ProvPy 3.2.1
and ProvToolbox. Each row gives the departure, the reason and the test that
covers it, as `test_name` (file). Files are under `provkit/` unless noted.

## PROV-DM and model

| Departure | Reason | Test |
|---|---|---|
| Every formal attribute is `Option`. Mandatory-ness is not checked on read. | A document with missing arguments reads, and a `Default`-profile PROV-N read accepts it. | `from_parts_builds_every_kind_empty` (src/record.rs), `profiles` (src/provn/parser.rs) |
| An attribute named `prov:` plus the local name of one of the record's own formals (`prov:activity` on `wasGeneratedBy`) is an error. `Record::from_parts` rejects it, so both readers and `unified()` do, and the PROV-JSON writer refuses a hand-built record that carries one. | ProvPy folds such attributes into the formal; provkit rejects them because PROV-DM reserves the prov: namespace and defines no attribute meaning for these names. | `attribute_named_after_an_own_formal_is_rejected` (src/record.rs, src/provn/parser.rs), `attribute_spelling_an_own_formal_fails_to_write` (src/json/write.rs) |
| A record without an identifier is not equal to the same record with one. | The identifier is part of the assertion. ProvPy's `__eq__` treats the two as equal, which makes equality depend on which side has the identifier. | `records_compare_and_hash_by_value` (src/record.rs) |
| Bundles sharing an identifier (possible only in hand-built documents; readers reject them with `DuplicateBundle`) count as one bundle. Equality compares the union of their records, both writers merge them into one bundle, and `unified()` merges them first. On a prefix clash the first binding wins. | Equality, writing and unification then agree on one reading of an ill-formed document. | `repeated_bundle_identifiers_merge` (src/document.rs), `bundles_sharing_an_identifier_merge_first` and `same_identifier_bundles_with_clashing_prefixes_unify` (src/unify.rs), `bundles_sharing_an_identifier_merge_and_round_trip` (src/json/write.rs), `bundles_sharing_an_identifier_are_written_once` (src/provn/write.rs), `duplicate_bundle` (src/sink.rs) |
| ProvToolbox treats labels, types and statements as unordered sets. provkit keeps insertion order in `Attributes` and in record lists, and compares as sets. | Output follows input order, so a written document reads like its source. Equality ignores order, so the two models compare the same. | `ordered_deduplicated_set_equal` (src/attributes.rs), `equality_is_a_set_over_records` (src/document.rs) |
| ProvToolbox's normal form merges records by union. `Document::unified()` fails with `Error::Unification` when formal attributes conflict (Constraint 55). | A merge that silently picks a side hides a data error. | `conflicting_times_fail`, `conflicting_names_fail` (src/unify.rs) |
| `Document::unified()` never merges `hadMember` records. They are not grouped by identifier, so records sharing one stay separate, and they are not checked against other kinds under that identifier. Equality still compares a `Membership`'s identifier. | PROV-DM 5.6.1 gives `hadMember` no identifier. An identifier on a `Membership` is a PROV-JSON grouping key: `hadMember` with an identifier and an array under `prov:entity` reads as one record per member, all sharing the key, and merging them would conflict on `prov:entity`. | `identified_memberships_survive_unchanged` (src/unify.rs), `formals_blank_ids_and_fan_out` (src/json/read.rs) |
| All 28 `scruffy-*` corpus files fail to unify: 20 on a conflicting time, 8 on a conflicting name (the start3, start4, end3 and end4 pairs). | The files exist to hold conflicting records. | `unification_over_the_corpus` (tests/corpus_json.rs) |

## Names and values

| Departure | Reason | Test |
|---|---|---|
| Name resolution is strict. An undeclared prefix, or a bare name with no default namespace, is `Error::UndeclaredPrefix`. This includes `xsd:QName` values and datatypes. ProvPy keeps an unresolvable `xsd:QName` value opaque ([ProvPy #238](https://github.com/trungdong/prov/issues/238)). | Every name in a document has one IRI, so equality and unification never compare unresolved text. | `undeclared_prefix_and_bare_without_default_fail` (src/namespaces.rs), `errors` and `values_decode_per_table` (src/json/read.rs), `qualified_name_datatypes_read_as_names` (src/provn/parser.rs) |
| Names are equal when their IRIs are equal, however the namespace and local part split the IRI: `http://example.org/` with `foo/bar` equals `http://example.org/foo/` with `bar`. Ordering and hashing follow the IRI too, and an attribute whose IRI is a formal's (`prov:entity`) is that formal under any split. ProvPy compares the same way. | PROV-DM says a qualified name denotes an IRI. | `identity_is_the_iri` (src/qname.rs), `two_spellings_of_one_iri_merge` (src/unify.rs), `a_formal_spelled_with_another_split_is_the_formal` (src/json/read.rs) |
| `_` is never a prefix. `declare("_")` and resolving `_:x` are errors. PROV-JSON `_:` keys are blank (no identifier). PROV-N rejects `_:x` because it is not a QUALIFIED_NAME (PN_PREFIX starts with PN_CHARS_BASE). | `_:` is the blank-node form, which names no IRI. | `underscore_is_never_a_prefix` (src/namespaces.rs), `formals_blank_ids_and_fan_out` (src/json/read.rs), `blank_node_label_is_a_syntax_error` (src/provn/lexer.rs) |
| A prefix must be a `PN_PREFIX` (a letter first, no `:` or whitespace, no trailing `.`). `Namespaces::declare` rejects any other with `Error::InvalidQName`, so a PROV-JSON `prefix` key that is not one is a read error. | PROV-N cannot write any other prefix, and a table that holds only valid prefixes writes to both formats. | `prefixes_must_be_pn_prefixes` (src/namespaces.rs) |
| A bundle identifier resolves in the bundle's own scope first, then the document's (PROV-N 3.7.4), in both readers. Writers print it with the bundle's merged prefix table. | Follows the grammar's scoping rule, and the printed name reads back to the same IRI. | `bundle_id_resolves_in_bundle_scope` (src/json/read.rs), `bundle_identifier_resolves_in_its_own_scope` (src/provn/parser.rs), `bundle_prefix_overrides_the_documents_for_its_own_identifier` (src/provn/write.rs) |
| `xsd:anyURI` stays a typed literal. ProvPy turns it into an `Identifier`. | Values keep their lexical form and datatype, so writing returns what was read. | `any_uri_stays_a_typed_literal` (src/json/read.rs) |
| `xsd:string` folds to a plain string. | `"x"` and `"x" %% xsd:string` denote the same value, and equality must agree. | `typed_folds_string_and_parses_datetime` (src/value.rs) |
| `%XX` in a PROV-N local part is kept verbatim, not percent-decoded (as SPARQL PLX). Backslash escapes are unescaped. | Decoding would make `ex:a%2Fb` and `ex:a/b` the same name, and the grammar does not ask for it. | `local_part_rules` (src/provn/lexer.rs) |
| Non-finite floats convert as XSD `NaN`, `INF` and `-INF`. | XSD spells them differently from Rust's `NaN`, `inf` and `-inf`. | `non_finite_floats_use_xsd_spellings` (src/value.rs) |
| `xsd:dateTime` fractions beyond nine digits are truncated to nanoseconds for equality, ordering and hashing. The source text keeps every digit and is written back. | `DateTime` holds nanoseconds. No corpus value has more than nine fraction digits. | `fractions_beyond_nine_digits_truncate` (src/datetime.rs) |

## PROV-JSON

| Departure | Reason | Test |
|---|---|---|
| A duplicated JSON object key keeps the last value and is not reported. | `serde_json` builds the map before provkit sees it. | `duplicated_keys_keep_the_last` (src/json/read.rs) |
| Raw JSON numbers are re-rendered, not preserved. Integers type as `xsd:int` or `xsd:long` by range, and values in (`i64::MAX`, `u64::MAX`] as `xsd:integer`. Integers beyond `u64`, and any number with a fraction or exponent, become `xsd:double` from `f64` in shortest round-trip form, so digits beyond `f64` precision are lost. Writers never emit raw numbers. | `serde_json` runs without `arbitrary_precision`. Only third-party input is affected. | `integers_type_by_range` (src/json/read.rs) |
| Blank-node labels are not preserved. The writer mints `_:id1`, `_:id2`, ... in document order, continuing through bundles. | Blank nodes have no identity beyond their position, and equality ignores labels. | `shape_and_blank_ids`, `blank_ids_continue_through_bundles`, `blank_ids_follow_record_order` (src/json/write.rs) |
| A blank key on an element or a bundle is a read error. | Elements and bundles must have an identifier. | `errors` (src/json/read.rs) |
| A prefix literally named `default` is not written as a `prefix` key, because PROV-JSON uses that key for the default namespace. Its names get a minted `nsN` prefix instead. | Writing it would turn a prefix into the default namespace on reading back. | `a_prefix_named_default_is_minted_afresh` (src/json/write.rs) |

## PROV-N

| Departure | Reason | Test |
|---|---|---|
| The writer writes every relation at full arity, with `-` for an absent argument, including a missing mandatory one. `Strict` rejects that output and `Default` accepts it. `activity` writes both times or neither. | One rule for every relation, and a document with missing data still round-trips. | `relations_have_full_arity`, `activity_writes_both_times_or_neither` (src/provn/write.rs), `profiles` (src/provn/parser.rs) |
| The writer writes `mentionOf` bare, not `prov:mentionOf`. | ProvToolbox writes this form, and `Strict` accepts only `prov:mentionOf`. | `mention_is_written_bare` (src/provn/write.rs), `profiles` (src/provn/parser.rs) |
| `Profile::Strict` enforces three rules, as ProvPy 3.2 does: `mentionOf` only as `prov:mentionOf`, no `-` in the leading required positions, and no extensibility expressions (rejected under both profiles). It does not reject identifiers or attributes on `alternateOf`, `specializationOf`, `hadMember` or `mentionOf`, and does not require records before bundles. | Matches ProvPy, so the two tools accept the same documents. | `profiles` (src/provn/parser.rs), `strict_enforces_three_rules_only` (src/provn/parser.rs) |
| A second `prefix` for one name, or a second `default`, in one declaration block is a syntax error. | ProvPy does the same. | `duplicate_prefix_in_one_scope_is_rejected`, `second_default_in_one_scope_is_rejected` (src/provn/parser.rs) |
| An unsigned integer token is accepted as an identifier and as an attribute name. It is not accepted as a prefix or as a `%%` datatype. | The grammar's QUALIFIED_NAME allows a digit-initial PN_LOCAL. | `digit_names_and_empty_attributes`, `prefixes_and_datatypes_are_not_integers` (src/provn/parser.rs) |
| The PROV-N writer returns `Error::Structure` (format `"PROV-N"`, naming the name and the character) for text PROV-N cannot carry: a local part with whitespace, a control character, one of `{ } < > " \| ^ \` or a backtick, a `%` not followed by two hex digits, or a first character that cannot start a name (a combining mark, `·`, `‿`, `⁀`); a prefix that is not a `PN_PREFIX`; a namespace IRI with a character `IRI_REF` forbids. The local-part escape set is the Recommendation's `PN_CHARS_ESC`, `= ' ( ) , : - [ ] . ;`. The other characters the reader accepts after `\` are written bare, where they lex the same. A bare name that is all digits, starts with `@` or is empty is written with a prefix. Whatever the writer accepts reads back as an equal document. None of the rejected cases occurs in the corpus. | Text written raw would fail to read back or read back as a different document, and the grammar has no escape for these characters. | `whatever_the_writer_accepts_reads_back_equal`, `unrepresentable_local_parts_are_errors`, `unrepresentable_iris_are_errors`, `a_json_key_with_a_trailing_space_cannot_be_written`, `bare_names_that_would_misread_take_a_prefix` (src/provn/write.rs), `json_corpus_round_trips_through_provn` (tests/corpus_provn.rs) |
| `UndeclaredPrefix` and value-construction errors (bad `dateTime`, bad language tag) carry no line or column. `Error::Syntax` does. | The value and name layers are shared with the JSON reader, which has no positions. | `undeclared_prefix_and_bare_without_default_fail` (src/namespaces.rs), `errors_carry_positions` (src/provn/parser.rs) |

## Not implemented in 0.1.0

- `lenient` PROV-N profile
- builder API, index, query and store
- PROV-JSONLD, PROV-XML and PROV-O
- validation (the constraints in PROV-CONSTRAINTS)

## Corpus

The tests read [prov-corpus](https://github.com/trungdong/prov-corpus) v2.0.0
(submodule at `corpus/`), which holds ProvPy 3.2.1's fixtures. ProvPy (`prov`,
Python) shares it. Version 2.0.0 declares the prefixes that
[issue #1](https://github.com/trungdong/prov-corpus/issues/1) found missing.
Failing files are collected and reported together.

### PROV-JSON

398 documents. All 398 read, round-trip through PROV-JSON and round-trip
through PROV-N.

Tests: `reads_all_398`, `round_trips_all_398` (tests/corpus_json.rs),
`json_corpus_round_trips_through_provn` (tests/corpus_provn.rs).

### PROV-N specification examples

127 examples. 100 parse under `Strict`. The other 27 are excluded and asserted
rejected. They are listed in the corpus `provn/README.md` for grammar reasons,
and ProvPy 3.2.1 excludes the same 27.

Test: `spec_examples_parse_under_strict` (tests/corpus_provn.rs).

### ProvToolbox files

9 files. 4 parse under `Default`. 5 are excluded:

- `container0`: bare names without a default namespace.
- `container1`, `prov-family`, `prov-family-graphics`: dictionary
  set-of-pairs syntax.
- `container2`: bare IRI datatype.

Test: `provtoolbox_files_parse_under_default` (tests/corpus_provn.rs).

### ProvToolbox twins

388 PROV-N files with a PROV-JSON twin. 229 read equal to the twin. 159 are
known differences (ProvPy 3.2.1 `KNOWN_DIFFERENCES`). All 159 parse and are
unequal to the twin:

- 80 dynamic timestamps, where ProvToolbox wrote the wall-clock time.
- 79 datatype palette differences.

Test: `provtoolbox_corpus_equals_json_twins` (tests/corpus_provn.rs).
