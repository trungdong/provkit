# Chapter 5: PROV-JSON

PROV-JSON is the W3C member submission of April 2013, a JSON object
keyed by [record](../reference/glossary.md#record-prov) kind and then by identifier, with a `prefix` object
for the [namespace](../reference/glossary.md#namespace-and-prefix-prov) table and a `bundle` object for the named [bundles](../reference/glossary.md#bundle-prov).
This chapter reads the three layers of the `json` module. serde sees the
shape and refuses what is not PROV-JSON, `read` turns the shape into
builder calls, and `write` goes back from a [document](../reference/glossary.md#document-prov) to text. It
builds on chapter 4 for the builder both readers drive and for
`Record::from_parts`, which checks every record this reader makes, and
on chapter 2 for the scope that resolves each key to a `QName`.

## What the code does

### provkit/src/json/mod.rs

Every position in an error from this module is a 1-based line and a
1-based column counted in characters. The file declares three private
modules, re-exports `write` and
`to_string`, and defines the public reader.

```rust,ignore
{{#include ../../../provkit/src/json/mod.rs:json_read}}
```

The body of [`read`](../api/provkit/json/fn.read.html) is the three
layers, one call each. `strip_bom` drops one leading byte order mark,
`serde_json::from_str` reads the text into the wire type
`JsonDocument`, and `read::emit` drives the builder with what the wire
type holds. Text that is not JSON is [`Error::Json`](../api/provkit/enum.Error.html) holding a
[`JsonError`](../api/provkit/struct.JsonError.html) with the line and
column where parsing stopped. Valid JSON that is not a PROV document is
`Error::JsonStructure`, and `shape_error`, below the span, tells the
two apart by `serde_json`'s `classify`, which reports JSON of the wrong
shape as a data error. A fault found while the JSON is read carries the
position of the offending token. For a key the reader refuses, the
position is where `serde_json` puts an error raised once the key's
value has been read, past the white space after the value and past a
closing brace that follows it. An error inside a record is wrapped in
`Error::InRecord` naming the section and the record's key, and inside a
bundle a second `InRecord` names the bundle. `char_column` recounts
the column `serde_json` reports in characters, because `serde_json`
counts bytes and the PROV-N reader counts characters.

### provkit/src/json/wire.rs

These are the types serde reads into and writes from, private to the
module, and the model never sees them. `JsonDocument` is a `prefix` table, an `IndexMap` from `Kind` to
a section, the bundles in order, and the first bundle key given twice
if there is one. A section is an `IndexMap` from identifier to
`OneOrMany<JsonRecord>`, and a record is an `IndexMap` from [attribute](../reference/glossary.md#attribute-prov)
key to `JsonValue`. The `Deserialize` impls for the document and the
bundle both call one visitor.

```rust,ignore
{{#include ../../../provkit/src/json/wire.rs:container_visitor}}
```

`ContainerVisitor` reads the keys of a document or bundle object in a
`visit_map` loop. `next_key` hands over the next key, or `None` at the
end of the object, and the `match` decides what the key's value is.
`prefix` is the namespace table, `bundle` is the bundles when the
visitor was built for a document, and any name `Kind::from_name` knows
is a section. Each of those arms reads the value with `next_value` and
continues. Every other arm builds an error, for `prefix`, `bundle` or
a section seen twice, for `bundle` inside a bundle, or for an unknown
name, and falls through to the code below the `match`. That code
first reads the refused key's value into `IgnoredAny`, which parses
and discards it, so text that is not JSON is reported as that, and
then returns the refusal, placed where the parser now stands, at the
end of the value. The namespace table and each section are read
through a newtype.

```rust,ignore
{{#include ../../../provkit/src/json/wire.rs:unique}}
```

`Unique<V>` wraps an `IndexMap<String, V>` and exists for its
`Deserialize` impl. The visitor is a struct defined inside the
function, generic over `V` through a `PhantomData<V>` field because it
stores no `V` of its own, and its `visit_map` is the same loop with the
`IndexMap` `entry` API deciding each key. A vacant entry takes the
value. An occupied one reads the value into `IgnoredAny`, discards it
and returns `duplicate field` naming the key. Chapter 2 called
`entry().or_default()`; here the two variants of `Entry` are matched by
name. `Bundles`, below the span, reads the `bundle` object with a
`HashSet` instead, because it records a repeated key and stops rather
than refusing it, so the reader can report the repeat as a duplicate
bundle in its place among the keys.

Under one identifier a section holds one record or several.

```rust,ignore
{{#include ../../../provkit/src/json/wire.rs:one_or_many}}
```

`OneOrMany<T>` derives `Serialize` with `#[serde(untagged)]`, which
writes `One` as the object itself and `Many` as the array, with nothing
naming the variant. Its `Deserialize` is written by hand for
`JsonRecord`. `deserialize_any` asks the parser what shape comes next,
and the visitor answers in `visit_map` for an object and in
`visit_seq` for an array. Both read a record body through `Body`, a
unit struct that implements `DeserializeSeed`, so
`next_element_seed(Body)` reads each array element with the visitor a
single object gets. This half is not derived, because an untagged enum
tries each variant in turn and reports that none matched, while
dispatching on the shape keeps the inner error.
`Body::visit_map`, below the span, reads every attribute value as
`&RawValue`, which checks the value's syntax and borrows its text from
the input without building a value from it, and `from_raw` turns that
text into the enum the reader sees.

```rust,ignore
{{#include ../../../provkit/src/json/wire.rs:json_value}}
```

A number is kept as its token text. A string is its content, a boolean
its value, and a `{"$": ...}` object is `Typed`, with its `type` and
`lang` members as strings not yet resolved. `Many` is an array, and an
element is never itself an array, because `ValueVisitor::visit_seq`
refuses a nested one before parsing it. `Invalid` is a value the wire
layer could not read, holding the message and a `Place`, which is
either a position inside the fragment or the address of the byte after
it. An invalid value is a variant rather than an error, because an
error raised inside the visitor would be placed where the outer parser
stands, past the whole value, and would not name its record. The value
is carried to the reader instead, which places it and wraps it in the
record. A key given twice in a record that
holds no invalid value yet becomes an `Invalid` value the same way,
placed after its second value. The
`Serialize` impl at the end of the file writes each variant back, a
`Number` through `RawValue::from_string` so its text reaches the
output unchanged.

### provkit/src/json/read/mod.rs

`emit` turns the wire structs into document-builder events, and is the
entry point `read` calls once.

```rust,ignore
{{#include ../../../provkit/src/json/read/mod.rs:emit}}
```

Faults are reported in a fixed order. `refuse_invalid` walks the document's sections for the
first `JsonValue::Invalid` and returns it as `Error::InRecord`, with
`invalid_value`, above the span, turning its `Place` into a line and
column in the input. Then `namespaces` builds the document's table from
`prefix`, `start_document` opens the builder, and `emit_sections`
drives one `record` call per record. The bundle loop resolves every
bundle key in the bundle's own scope, `Scope::within` of its table over
the document's, and refuses a blank key, a bad namespace table, a key
that does not resolve and a repeat, each wrapped in an `InRecord`
whose section is `bundle`, before any bundle body is read. The closure
`located` captures `key` and is called at each of those points, by
hand or through `map_err`, so every error under one key is wrapped the
same way; chapter 3's `map_err` calls took a closure written in place.
The second loop emits each bundle between `start_bundle` and
`end_bundle`, and `end_document` closes the sequence chapter 4 described.
`emit_sections`, below the span, is where a section's `OneOrMany` is
read. `record_id` resolves the key, or returns `None` for a `_:` key
on a relation and an error for one on an element, and `as_slice` on
the group gives the one body or every body of the array, each passed
to `records_from`.

```rust,ignore
{{#include ../../../provkit/src/json/read/mod.rs:records_from}}
```

`records_from` reads one body into one record, or into several. Each
key resolves in scope, and `formal_named` says whether the name is one
of the kind's formals. A formal's value goes through `formal_value`,
which reads a time from a string or an `xsd:dateTime` object and a name
from a string or an `xsd:QName` or `prov:QUALIFIED_NAME` object, and
any other value goes through `decode_value` into chapter 3's `Value`, one
attribute per element of an array. The `prov:entity` of a `hadMember`
is the exception. Its elements are gathered as `members`, and when
there are any the function returns one record per member, each with a
clone of the identifier, the other formals and the attributes, by
mapping the members through `from_parts` and collecting the results
into one `Result`. `number`, the last function in the file, types a
raw JSON number from its text, an integer by range and anything else
as `xsd:double`, keeping the text as written.

### provkit/src/json/write.rs

[`write`](../api/provkit/json/fn.write.html) and
[`to_string`](../api/provkit/json/fn.to_string.html) are the public
pair.

```rust,ignore
{{#include ../../../provkit/src/json/write.rs:json_write}}
```

`write` takes any `impl Write` by mutable reference, so a caller
passes a file, a socket or a `Vec<u8>`. `to_wire` builds a
`JsonDocument` from the document, `serde_json::to_writer_pretty`
streams it with two-space indentation, and a newline follows. The
`map_err` closure sorts the error `serde_json` returns into two cases.
When the writer failed, `io_error_kind` is `Some` and the error becomes
`Error::Io`; otherwise it is `Error::Json`. `&mut *out` reborrows the
writer for the call, so `out` is still usable for `write_all` on the
next line. `to_string` writes into a `Vec<u8>` and converts it with
`String::from_utf8`, whose `expect` never fires because `serde_json`
writes UTF-8. `to_wire`, below the span, starts from a
working copy of the document's namespace table, so a minted [prefix](../reference/glossary.md#namespace-and-prefix-prov)
never touches the caller's document, and `bundle_wire` writes each
bundle against its own table over the document's. The records of each
scope go through `sections`.

```rust,ignore
{{#include ../../../provkit/src/json/write.rs:sections}}
```

The first statement gives each record an identifier or a blank label.
`ids` is a `Vec` of `Result<&QName, String>`, used as an either-or
rather than as success and failure, so a record with an identifier
borrows it and one without gets `_:id1`, `_:id2` and so on from the
counter `next_blank`, which the caller passes as `&mut usize` so the
numbers continue through bundles. The loop over `Kind::ALL` groups
records by kind in table order and by identifier within a kind,
`record_wire` turns each one into its body, and a group of one body
becomes `OneOrMany::One` while more become `Many`. `record_wire`, below
the span, writes the formals first, from `Record::formals`, then the
attributes, with several values under one name collected into a
`JsonValue::Many`, and `value_wire` writes a plain string bare and
every other value as a `$` object, so the reader never sees a raw
number or boolean the writer made. The doctest below runs the three
layers once, and the book compiles it.

```rust
# extern crate provkit;
use provkit::json;

fn main() -> Result<(), provkit::Error> {
    let text = r#"{
        "prefix": {"ex": "http://example.org/"},
        "entity": {"ex:e": [{"ex:n": 1.50}, {"ex:n": 2}]},
        "wasGeneratedBy": {"_:g": {"prov:entity": "ex:e"}}
    }"#;
    let doc = json::read(text)?;
    assert_eq!(doc.records.len(), 3);
    assert_eq!(doc.records[2].id(), None);

    let out = json::to_string(&doc)?;
    assert!(out.contains("\"$\": \"1.50\""));
    assert!(out.contains("\"_:id1\""));
    assert_eq!(json::read(&out)?, doc);
    Ok(())
}
```

## Why this shape

### Why capture values as raw JSON text?

`1.50` and `1e400` read as `xsd:double` with that text and compare by
value, and the writers never emit raw numbers, so the value is typed
from its token and no digit is lost ([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"A JSON number keeps its text"). The serde visitor hands a number over
as `i64`, `u64` or `f64`, so a reader built on those turns `-0` and
any integer beyond `u64` into an `xsd:double` with re-rendered text,
refuses `1e400` as out of range, drops the trailing zero of `1.50`,
and disagrees with the PROV-N reader, which types a bare integer by
its text, on the same number. A dependant that enables the
`arbitrary_precision` feature of `serde_json` changes how numbers
arrive, and the raw text is the same under both feature sets.

### Why refuse a repeated key?

The rule applies in a namespace table, among the sections of a
document or bundle, among the records of a section, among the
attributes of a record and among the members of a value object
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"A key given twice in one PROV-JSON object is an error"). ProvPy 3.3.0
refuses a repeated key as well. A reader that keeps the last value
drops a namespace, a record, a section, a bundle or a value without a
message. RFC 8259
says the names in an object SHOULD be unique and I-JSON (RFC 7493)
says they MUST be, so a document with a repeated key is malformed by
the standards' own terms. Keeping the last value would hide that fault
from the reader. The PROV-N readers of both libraries already reject a
repeated prefix and a repeated bundle, so refusing a repeated key gives
PROV-JSON the same rule.

### Why do positions match `serde_json`'s?

The PROV-N reader carries a line and a column on every error, and the
PROV-JSON reader on every error it finds while reading the JSON. Both
count the column in characters. A PROV-JSON error found while building
a record carries no position, and `Error::InRecord` names the section
and the key of the record instead
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"Every error from the PROV-N reader carries a line and a column"). The
reader raises some errors itself, a value it could not decode or a key
given twice in a record, after `serde_json` has accepted the text, and
each is placed where `serde_json` gives the same error when it reads
the document in one pass, so an error from either source reads the
same way. For a value that is the line and column of
its token. For a refused key it is past the value, past the white
space after it and past a closing brace that follows, because that is
where `serde_json` stands when a map visitor returns an error after
`next_value`. The column is counted again in characters by
`char_column`, because `serde_json` counts bytes and the PROV-N reader
counts characters. `strip_bom` runs before the parse, so a leading
byte-order mark is counted in no column, because it is an encoding
signature, not content, while U+FEFF is otherwise a name character
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"`json::read`, `provn::read`, `provn::read_with` and
`provn::read_report` ignore one leading UTF-8 byte-order mark").

### Why fan a `hadMember` with several members into several records?

A `hadMember` with N members reads as N records that share their
attributes, because the ProvToolbox shared corpus writes the members
of a
[collection](../reference/glossary.md#collection-and-membership-prov)
as an array under `prov:entity`, ProvPy reads that form, the
submission's schema does not define it, and PROV-DM holds one member
per relation ([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"A PROV-JSON `hadMember` with an empty `prov:entity` array"). Section
3.2.14 of
[PROV-JSON](https://www.w3.org/Submission/2013/SUBM-prov-json-20130424/#term-membership)
writes three memberships on one collection as three objects under
`hadMember`, each with one `prov:entity`, which is the model the
fan-out produces, one membership per member. Every record gets the
identifier and the attributes, as in ProvPy 3.3.0, because the
records are alike except for the member, and
which member comes first should not decide which record carries the
identifier ([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"A `hadMember` with several members gives every record the identifier
and the attributes").

### Why accept an array of objects under one identifier?

The submission's schema does not define the form, and ProvPy reads
and writes it. Two records of one kind can share an identifier, and a
JSON object holds a key once ([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"The PROV-JSON reader and writer take an array of record objects under
one identifier"). Section 2 of
[PROV-JSON](https://www.w3.org/Submission/2013/SUBM-prov-json-20130424/#PROV-JSON-overview)
makes each section a map of records indexed by identifier, so without
the array a second `entity` under the identifier of the first has
nowhere to go, while `Document` equality, which chapter 4 showed is a set
over records, keeps both. ProvToolbox's reader refuses the form.

### Why are blank-node labels not preserved?

The writer mints `_:id1`, `_:id2` and so on in document order,
continuing through bundles, and a blank key on an element or a bundle
is a read error, because
[blank nodes](../reference/glossary.md#blank-node-prov) have no
identity beyond their position, equality ignores labels, and elements
and bundles need an identifier ([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"Blank-node labels are not preserved"). Section 2.1 of
[PROV-JSON](https://www.w3.org/Submission/2013/SUBM-prov-json-20130424/#blank-node-ids)
says the same of the format. Identifiers of relations are optional in
PROV but required in PROV-JSON because they are keys, so a blank node
identifier MUST be created for such a relation, and "Blank node
identifiers do not carry any semantics and SHOULD be ignored by
applications consuming PROV-JSON documents". `record_id` reads a `_:`
key as no identifier, and `sections` writes a fresh one.

## The Rust here

- **serde's `Deserialize` and `Visitor`**. `Deserialize::deserialize`
  hands the deserializer a visitor, and the deserializer calls the
  `visit_*` method for the shape it finds; `visit_map` gets a
  `MapAccess` whose `next_key` and `next_value` read the object one
  entry at a time, so `ContainerVisitor` decides each key before its
  value is read.
  [Visitor](https://docs.rs/serde/latest/serde/de/trait.Visitor.html),
  [MapAccess](https://docs.rs/serde/latest/serde/de/trait.MapAccess.html),
  [Implementing Deserialize](https://serde.rs/impl-deserialize.html)
- **`DeserializeSeed`**. A seed is a value that drives a read, where
  `Deserialize` is a type; `next_element_seed(Body)` reads each array
  element through `Body`'s visitor, the one a single object gets.
  [DeserializeSeed](https://docs.rs/serde/latest/serde/de/trait.DeserializeSeed.html)
- **`IgnoredAny`**. Reading a value into it parses and discards it, so
  a key `ContainerVisitor` refuses has its value checked as JSON and
  the parser moves past it.
  [IgnoredAny](https://docs.rs/serde/latest/serde/de/struct.IgnoredAny.html)
- **`serde_json::value::RawValue`**. `next_value::<&RawValue>()` in
  `Body::visit_map` checks one value's syntax and borrows its text from
  the input without building a value from it, and `get` returns it as
  `&str`; it needs the `raw_value` feature of `serde_json`.
  [RawValue](https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html)
- **`#[serde(untagged)]`**. The derived `Serialize` writes a variant's
  payload with nothing naming the variant, so `One` is the object and
  `Many` the array.
  [Enum representations](https://serde.rs/enum-representations.html)
- **Manual `Serialize`**. In `serialize_container`, `serialize_map`
  opens an object, `serialize_entry` writes each pair and `end` closes
  it, which is how `prefix` comes first and `bundle` last whatever the
  map order.
  [Implementing Serialize](https://serde.rs/impl-serialize.html)
- **`PhantomData`**. A zero-sized field that names a type parameter the
  struct does not store, so `Visit<V>` can implement `Visitor` with
  `Value = Unique<V>`.
  [PhantomData](https://doc.rust-lang.org/std/marker/struct.PhantomData.html)
- **Generic newtypes**. `Unique<V>` is a tuple struct around one field,
  which gives the field a second `Deserialize`, here one that refuses a
  repeated key, and `.0` reaches the map inside.
  [Using the newtype pattern](https://doc.rust-lang.org/book/ch20-02-advanced-traits.html#using-the-newtype-pattern-to-implement-external-traits-on-external-types)
- **`IndexMap` `Entry` matched by variant**. `entry(key)` in `Unique`'s
  visitor is `Vacant` or `Occupied`, and matching the two is what makes
  a repeat visible; chapter
  2's `or_default` chose for the caller.
  [Entry](https://docs.rs/indexmap/latest/indexmap/map/enum.Entry.html)
- **`Vec::with_capacity`**. In `emit` the vector is allocated once for
  the number of bundles, which is known, so no push grows it.
  [Vec::with_capacity](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.with_capacity)
- **`collect` into `Result<Vec<_>, _>`**. `Result` implements
  `FromIterator`, so collecting an iterator of results in
  `records_from` gives the first `Err` or a `Vec` of every `Ok`.
  [FromIterator for Result](https://doc.rust-lang.org/std/iter/trait.FromIterator.html#impl-FromIterator%3CResult%3CT,+E%3E%3E-for-Result%3CV,+E%3E)
- **`&mut *out` reborrowing**. Dereferencing a `&mut` and borrowing it
  again lends the writer for one call without moving the reference, so
  the next line of `write` can use `out`.
  [Borrow operators](https://doc.rust-lang.org/reference/expressions/operator-expr.html#borrow-operators)
- **`String::from_utf8(..).expect(..)` with a reason**. `from_utf8`
  returns a `Result` because not every byte sequence is text; `expect`
  panics with the message when it is `Err`, which `to_string` never
  meets, because `serde_json` writes UTF-8.
  [String::from_utf8](https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8),
  [Result::expect](https://doc.rust-lang.org/std/result/enum.Result.html#method.expect)
- **`Result<&QName, String>` as either-or**. `ok_or_else` in `sections`
  builds a `Result` whose `Err` is not a failure but the other case, a blank
  label, and the `match` on it later reads both sides as data.
  [Result](https://doc.rust-lang.org/std/result/enum.Result.html)

## Run this

```sh
cargo test -p provkit values_decode_per_table
```

This reads one [entity](../reference/glossary.md#entity-prov) whose attributes hold every value form, a plain
string, raw integers typed by range, a raw double, a boolean, `$`
objects with and without a `type`, a language-tagged string, names
under `xsd:QName` and `prov:QUALIFIED_NAME`, a dateTime, a typed
`xsd:long` and an array, and checks the `Value` each decodes to. The
unit-test binary reports the one test; the other test binaries report
zero.

```text
test result: ok. 1 passed
```

```sh
cargo test -p provkit repeated_keys_in_a_document_or_bundle
```

This reads a document with `prefix` given twice, one with `bundle`
given twice and a bundle with `prefix` given twice, and checks that
each is `Error::JsonStructure`.

```text
test result: ok. 1 passed
```

```sh
cargo test -p provkit shape_and_blank_ids
```

This writes a document holding a generation with no identifier, an
entity and an entity in an undeclared namespace, and checks that
`prefix` comes before `entity` and `entity` before `wasGeneratedBy`,
that the undeclared namespace is minted as `ns1`, that the generation
is written under `_:id1`, and that the caller's namespace table is
untouched.

```text
test result: ok. 1 passed
```

## Try it

Write `/tmp/dup.json` with one entity key given twice.

```json
{"prefix": {"ex": "http://example.org/"}, "entity": {"ex:e": {}, "ex:e": {}}}
```

Predict what `provkit convert /tmp/dup.json` prints on standard error
and what it exits with, and where in the line the message points.
Then run the file through Python's `json` module to see what a general
JSON parser does with the same text.

<details><summary>Answer</summary>

The second `ex:e` is a key given twice among the records of a section,
which `Unique` refuses. The CLI prints the error with the file name and
exits 2, as it does for any error.

```sh
provkit convert /tmp/dup.json; echo $?
```

```text
provkit: /tmp/dup.json: PROV-JSON: duplicate field `ex:e` at line 1 column 76
2
```

The message names the key and the position. The second value is the
`{}` after the repeated key, and the brace that follows it closes the
`entity` object, so the error sits past that brace, at column 76, the
count of characters on the line up to and including it.

```sh
python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])))' /tmp/dup.json
```

```text
{'prefix': {'ex': 'http://example.org/'}, 'entity': {'ex:e': {}}}
```

Python keeps one `ex:e` and says nothing, so the document changes
unannounced.

</details>

## Idioms first seen here

- `serde's Deserialize and Visitor`: https://docs.rs/serde/latest/serde/de/trait.Visitor.html
- `DeserializeSeed`: https://docs.rs/serde/latest/serde/de/trait.DeserializeSeed.html
- `IgnoredAny`: https://docs.rs/serde/latest/serde/de/struct.IgnoredAny.html
- `serde_json::value::RawValue`: https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html
- `#[serde(untagged)]`: https://serde.rs/enum-representations.html
- `manual Serialize`: https://serde.rs/impl-serialize.html
- `PhantomData`: https://doc.rust-lang.org/std/marker/struct.PhantomData.html
- `generic newtypes`: https://doc.rust-lang.org/book/ch20-02-advanced-traits.html#using-the-newtype-pattern-to-implement-external-traits-on-external-types
- `IndexMap Entry matched by variant`: https://docs.rs/indexmap/latest/indexmap/map/enum.Entry.html
- `Vec::with_capacity`: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.with_capacity
- `collect into Result<Vec<_>, _>`: https://doc.rust-lang.org/std/iter/trait.FromIterator.html#impl-FromIterator%3CResult%3CT,+E%3E%3E-for-Result%3CV,+E%3E
- `&mut *out reborrowing`: https://doc.rust-lang.org/reference/expressions/operator-expr.html#borrow-operators
- `String::from_utf8(..).expect(..) with a reason`: https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8
- `Result<&QName, String> as either-or`: https://doc.rust-lang.org/std/result/enum.Result.html
