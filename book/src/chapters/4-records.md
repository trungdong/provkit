# Chapter 4: Records and documents

A PROV [document](../reference/glossary.md#document-prov) is a set of [records](../reference/glossary.md#record-prov) of eighteen kinds, each a struct
with an identifier, its [formal attributes](../reference/glossary.md#formal-attribute-prov) and the [attribute](../reference/glossary.md#attribute-prov) set of
chapter 3, held at the top level or in a named [bundle](../reference/glossary.md#bundle-prov). This chapter reads
the table that states the eighteen kinds once, the formal values that
fill their positions, the document that holds them, the equality both
`compare` and the tests lean on, the [unification](../reference/glossary.md#unification-prov) that merges records
sharing an identifier, the builder both readers drive, and the error
type every layer returns. It builds on chapter 2 for `QName`, which is
every identifier and every name-valued formal, and on chapter 3 for
`DateTime`, `Attributes` and the equality of values.

## What the code does

### provkit/src/record/mod.rs

The file opens with `#[macro_use] mod formal;`, then `Record::from_parts`
and `Record::formals`, then the `records!` macro and the table it reads.
The table is where a reader starts.

```rust,ignore
{{#include ../../../provkit/src/record/mod.rs:records_table}}
```

The book shows this span and those that follow as they stand in the
files, and compiles none of them. A row is the variant's name, its
PROV-N keyword, which is also its PROV-JSON key, `element` or
`relation`, `bare` on the four relations PROV-DM gives no identifier,
the argument counts PROV-N allows after the identifier, and the
record's fields with the formal each one holds. The doc line above a
row becomes the doc of the generated struct and of its
[`Record`](../api/provkit/enum.Record.html) variant. The macro above
the table turns each row into code. Its first rules begin with a
marker such as `@id_type`, and the main rule calls them to pick one
piece of code by the row's shape. One repetition in the main rule
writes a struct per row.

```rust,ignore
{{#include ../../../provkit/src/record/mod.rs:record_struct_rule}}
```

The `id` field is a `QName` on an element and an `Option<QName>` on a
relation, which is what `@id_type` picks. Each formal is an `Option`
of the type `formal_type!` gives it, `QName` or `DateTime`, under the
doc line `formal_doc!` gives it, and `attributes` is chapter 3's set. The
same main rule writes [`Kind`](../api/provkit/enum.Kind.html), one
`Copy` variant per row, with `Kind::ALL` as a slice of every variant,
and `name`, `from_name`, `is_element`, `formals` and the crate-private
`is_bare` and `arities`, each a `match` with one generated arm per
row. It writes
`Record` as an enum with one variant per kind wrapping that kind's
struct, and `kind`, `id`, `attributes`, `attributes_mut`, `formal` and
the private `assemble` on it. `Record::from_parts`, above the macro,
is ordinary code. `Formals::new` checks that each formal belongs to
the kind, holds a name or a time as the `formals!` table says, and
appears once. `Kind::check_attributes` refuses an attribute named `prov:` plus
the local name of one of the kind's formals, because that value
belongs in the positional argument. Then `assemble` takes each field
out of the checked list, and `@new_id` turns a missing identifier on
an element into `Error::InvalidRecord`. Both readers call
`from_parts`, so every record they produce has passed these checks.
`Record::formals` is the inverse, the present formals in
`Kind::formals` order, and `formals_round_trip_through_from_parts_for_every_kind`
in the tests drives a record of every kind through the two.

### provkit/src/record/formal.rs

The `formals!` table lists every formal attribute once, with its value
type and its local name under `prov:`.

```rust,ignore
{{#include ../../../provkit/src/record/formal.rs:formals_table}}
```

The macro above it writes
[`FormalAttr`](../api/provkit/enum.FormalAttr.html), with `ALL`,
`local_name` and `is_time`, and then two macros, `formal_type!` and
`formal_doc!`, each with one rule per row, which the `records!` table
in `mod.rs` calls for every field. `#[macro_use]` on `mod formal;` is
what lets `mod.rs` see them, and a `macro_rules!` macro is visible only
after the point where it is defined, which is why `mod formal;` comes
first in `mod.rs`. After `Display`, which prints a formal as
`prov:entity`, come the two value types.

```rust,ignore
{{#include ../../../provkit/src/record/formal.rs:formal_twins}}
```

[`Formal`](../api/provkit/enum.Formal.html) owns its value and
[`FormalRef`](../api/provkit/enum.FormalRef.html) borrows one, so the
second is `Copy` and carries a lifetime. `from_parts` takes the owned
form, because the record keeps the value, and `Record::formals` and
`Record::formal` hand back the borrowed form, because the record still
holds it. `to_formal` and the `From` impl clone a borrowed value into
an owned one, which is how `merge` below passes what it read on to
`from_parts`, and `UnificationCause::Formal` holds the owned form.
`Error` has no lifetime parameter, and a `FormalRef` needs one.
Below the twins,
`FormalValue` is a `pub(super)` trait, visible within the `record`
module and nowhere else, implemented for `QName` and `DateTime`. `Formals::take<T>` is
generic over it, and the compiler picks `T` from the type of the field
being filled, so one line of the macro template fills a name field
and a time field alike.

### provkit/src/document.rs

[`Bundle`](../api/provkit/struct.Bundle.html) and
[`Document`](../api/provkit/struct.Document.html) are plain structs
with public fields.

```rust,ignore
{{#include ../../../provkit/src/document.rs:document}}
```

A bundle is an identifier, a [namespace](../reference/glossary.md#namespace-and-prefix-prov) table and records, and a
document is a namespace table, top-level records and bundles. Every
reference between records is a `QName`, so a document holds no
pointer into itself and can be cloned, compared and sent between
threads freely. Neither type derives `PartialEq`; the impls at the end
of the file are written out and read below. `bundle` finds
the first bundle with an identifier, and the next method reads the
bundles as equality and the writers do.

```rust,ignore
{{#include ../../../provkit/src/document.rs:merged_bundles}}
```

`merged_bundles` returns a `Cow<'_, [Bundle]>`, which is either a
borrow of the document's own `bundles` or an owned `Vec` the method
built. The `all` call, chapter 2's adaptor, inserts each identifier into
a `HashSet` of borrowed names and stops at the first one already
there, so a document whose identifiers are all distinct, which is
every document a reader builds, copies nothing. Otherwise the loop
keeps one bundle per identifier at the position of its first copy,
absorbs the later copies' namespace tables with the first binding of
a [prefix](../reference/glossary.md#namespace-and-prefix-prov) winning, and appends their records. After it,
`records_with_id` and `all_records` are the two lookups, the second
chaining the top level onto a `flat_map` over the bundles. Equality
is the last thing in the file before the tests.

```rust,ignore
{{#include ../../../provkit/src/document.rs:document_eq}}
```

`same_records` collects each side into a `HashSet<&Record>` and
compares the sets, so record order and repeats play no part, and the
set is keyed by `Record`'s derived `Hash` and `Eq`, which reach chapter
3's rules for every value. `bundle_sets` maps each bundle identifier
to the set of records of every bundle carrying it. A `Bundle` is equal
to another when their identifiers and record sets are, and a
`Document` when the top-level sets and the bundle maps are. Namespace
tables take no part in either. Grouping by identifier keeps `==`
reflexive for a hand-built document which repeats one, and
`repeated_bundle_identifiers_merge` in the tests checks that it does.

### provkit/src/unify/mod.rs

The one public item is a method on `Document`, written in this module
rather than in `document.rs`.

```rust,ignore
{{#include ../../../provkit/src/unify/mod.rs:unified}}
```

[`unified`](../api/provkit/struct.Document.html#method.unified) itself
is short. It walks `merged_bundles`, so two bundles sharing an
identifier are one before anything merges, runs `unify_records` on
each bundle and on the top level, and returns a new document, with
`?` passing the first error out of the loop. `unify_records`, below
the span, first calls `check_attributes` on every record, because a
hand-built record can carry an attribute named after one of its own
formals. It then groups positions by identifier in an `IndexMap`,
skipping records with no identifier and the four bare kinds, whose
identifier is a PROV-JSON grouping key and nothing more. Within a
group it splits by kind, and `check_kinds` refuses the pairs of kinds
that Constraints 53 to 55 keep apart, through
`may_share_an_identifier`. Each kind's members then go through
`merge`, the merged record takes the first member's position, and
`each_once` drops any record equal to an earlier one in the same
scope.

```rust,ignore
{{#include ../../../provkit/src/unify/mod.rs:merge}}
```

`merge` walks the kind's formals in table order and builds the parts
for `from_parts`. The `conflict` closure captures `id` and `attr` and
builds the `Error::Unification` for a clash, so the two places that
fail share one message. `is_none_position`, above the span, says
whether the formal is one of the four where an absent value is the
constant "none", the `plan` of an association, the `activity` of a
derivation, and the `generation` and `usage` of a derivation whose
`activity` is absent. There every member must hold the same value as
the first, an absent one included. Everywhere else an absent value is
an existential. The loop over `filter_map` sees only the present
values, takes the first, lets an equal one pass and fails on a
different one, so a `None` beside a value yields the value. The
attribute sets are merged with `extend_from`, which chapter 3 showed
shares the first set's allocation, and `from_parts` rebuilds the
record under the shared identifier. The doctest below is the rule in
one program, and the book compiles it.

```rust
# extern crate provkit;
use provkit::{Activity, Attributes, DateTime, Document, QName, Record};

fn main() -> Result<(), provkit::Error> {
    let id = QName::new("http://example.org/", "a")?;
    let start = DateTime::parse("2020-01-01T00:00:00")?;
    let end = DateTime::parse("2020-01-02T00:00:00")?;
    let activity = |start_time: Option<DateTime>, end_time: Option<DateTime>| {
        Record::Activity(Activity {
            id: id.clone(),
            start_time,
            end_time,
            attributes: Attributes::new(),
        })
    };

    let mut split = Document::new();
    split.records.push(activity(Some(start.clone()), None));
    split.records.push(activity(None, Some(end.clone())));
    let mut whole = Document::new();
    whole.records.push(activity(Some(start), Some(end)));

    assert_ne!(split, whole);
    assert_eq!(split.unified()?, whole);

    let mut twice = whole.clone();
    twice.records.push(whole.records[0].clone());
    assert_eq!(twice, whole);
    assert_eq!(twice.unified()?.records.len(), 1);
    Ok(())
}
```

### provkit/src/builder.rs

[`Document`](../api/provkit/struct.Document.html) has public fields,
so any code can assemble one. The readers do not; they drive this
builder, which is crate-private and so has no page in the API
reference.

```rust,ignore
{{#include ../../../provkit/src/builder.rs:builder}}
```

`State` is the builder's position in the event sequence, and the
`InBundle` variant carries the open bundle, so there is no separate
field that might disagree with the state. `DocumentBuilder` holds the
document under construction, the identifiers of the bundles started
so far, and the state. `finish`, `start_document`, `record`,
`end_bundle` and `end_document` each check the state with
`debug_assert!`, which
compiles to nothing in a release build, because only the two readers
send events and they send them in order. `end_bundle` uses
`std::mem::replace` to take the open bundle out of the state and put
`InDocument` in its place in one move. The one event that can fail is
the start of a bundle.

```rust,ignore
{{#include ../../../provkit/src/builder.rs:start_bundle}}
```

`HashSet::insert` returns `false` when the identifier is already in
the set, and that is `Error::DuplicateBundle`. The PROV-N reader
reaches this line for every bundle it opens, so a second `bundle
ex:b` is refused here. The PROV-JSON reader resolves every bundle key
and refuses a repeat with the same error before it reads any bundle
body, and then drives `start_bundle` for each bundle, so the check
runs again there. Both readers
report one error for a repeated identifier, and `duplicate_bundle` in
the tests pins the builder's half. The identifier set is why
`many_bundles_check_duplicates_by_lookup` can start thousands of
bundles without a scan of the earlier ones per bundle.

### provkit/src/error.rs

Every failure in the crate is one
[`Error`](../api/provkit/enum.Error.html). The file opens with the
helper every message uses to quote input.

```rust,ignore
{{#include ../../../provkit/src/error.rs:clip}}
```

`clip` returns the text itself, borrowed, when it is at most `QUOTED`
characters long, and otherwise an owned string of the first `QUOTED`
characters, a marker and the length, so a message never carries a
megabyte of rejected input while the error's payload keeps all of it.
It counts characters, not bytes, so the cut never splits a multi-byte
character. The enum follows. It derives `thiserror::Error`, each
variant's `#[error("...")]` is its `Display`, and the enum and its
struct variants are `#[non_exhaustive]`, so a `match` in another
crate needs a wildcard arm and a pattern on a struct variant ends in
`..`; the doctest and the `compile_fail` doctest on the enum show
both. Two variants wrap another error.

```rust,ignore
{{#include ../../../provkit/src/error.rs:error_at}}
```

```rust,ignore
{{#include ../../../provkit/src/error.rs:error_in_record}}
```

`At` is a line, a column and a boxed error, and `InRecord` is a
section, a key and a boxed error. The field is a `Box<Error>` because
an enum cannot hold a value of its own type by value. Both print the
inner error after their own prefix, and both leave `source()` empty,
so a caller printing the chain of causes does not see the message
twice; `at_prints_its_error_and_has_no_source` and its twin in the
tests check both. The cause variants, `InvalidQName`, `InvalidLiteral`,
`InvalidRecord`, `UndeclaredPrefix` and the rest, carry no position of
their own. The PROV-N reader wraps one of them in `At` at the token it
was reading, and the PROV-JSON reader wraps one in `InRecord` naming
the section and the record's key, with a second `InRecord` naming the
bundle when the record sits in one. Later in the file, `Json` is the
one variant under `#[cfg(feature = "json")]`, chapter 1's gate on a
single variant, because its payload comes from an optional
dependency, and `Io` uses `#[from]` so an `io::Error` from a writer's
stream converts with `?`. `JsonError` marks its `serde_json` error
with `#[source]`, so there the chain is kept. The last item before
the tests is `type Result<T> = std::result::Result<T, Error>`, which
is the `Result` every signature in the crate names.

## Why this shape

### Why one macro table for the eighteen kinds?

Without the table, each kind's facts would be written by hand in
several places. The record module would repeat them in the `Kind`
variants, the keywords, the formals, the structs, the `Record`
variants and the arms of every `match` over the kinds. The PROV-N
parser would hold its own copy of the argument counts. With the
table, a kind's facts sit on one row, and the enum, the struct, the
`Record` variant and every generated `match` arm follow from it.
Adding a kind is adding a row, a re-export in `lib.rs`, a line in the
table that the test `formal_tables_match_the_spec` checks and, where
PROV-CONSTRAINTS names the kind, an entry in the constraint tables of
`unify/mod.rs`. `Kind` and `Record` are
`#[non_exhaustive]`, so a row added later does not break a `match` in another crate.

### Why is every formal an `Option`?

The PROV-JSON reader does not check that a mandatory formal is
present, and `Strict` rejects `-` in a required argument of a relation
because the grammar does, so a document with missing arguments reads,
and `Default` reads such a PROV-N document
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"Every formal attribute is an `Option`"). Section 2.4 of
[PROV-N](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prov-n-optionals)
defines the marker. When only some optional terms are
omitted, position matters, so "an additional marker must be used to
indicate that a particular term is not available", and "the symbol
'-' is used for this purpose". A field that can be absent in what the
readers accept has to be an `Option`, and the absent value is what
`merge` reads as an existential.

### Why is `Document` equality set-based?

Equality ignores record order, attribute order, namespace tables,
[blank-node](../reference/glossary.md#blank-node-prov) labels and the
spelling of a
[literal](../reference/glossary.md#literal-and-typed-literal-prov),
and it keeps identifier presence, while `Attributes` and the record
lists keep insertion order ([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"`Document` equality is set-based"). Output follows input order, so a
written document reads like its source, and the two models compare the
same. Bundles are matched by identifier, which is why `bundle_sets` is
a map and not a list.

### Why do the readers refuse a repeated bundle identifier while a hand-built document may hold one?

A bundle identifier names one bundle, so both readers refuse a second
bundle with the same identifier, although the PROV-N grammar accepts
one ([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"Two bundles with one identifier are `Error::DuplicateBundle`"). In
PROV-JSON a bundle key given twice counts as such a repeat. ProvPy
rejects them too in PROV-N. The fields of `Document` are public, so
nothing stops a program from pushing two bundles with one identifier,
and they then count as one
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"Bundles that share an identifier"). Equality compares the union of
their records, both writers merge them through `merged_bundles`, and
`unified()` merges them first, so equality, writing and unification
agree on one reading of an ill-formed document.

### What does `unified` follow?

Records sharing an identifier and a kind merge, with
`Error::Unification` for conflicting formals, because a merge that
silently picks a side hides a data error. An absent formal unifies
with a present one, except in the four positions where
PROV-CONSTRAINTS reads `-` as the constant "none". The four
identifier-less kinds and records with no identifier never merge,
while an exact repeat of any record is kept once
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md), the rows
beginning "`Document::unified()` follows Constraints 53 to 55", "An
absent formal unifies with a present one" and "`unified()` never
merges the four identifier-less kinds"). The constraints themselves
are in
[PROV-CONSTRAINTS](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/).
Constraints 22 and 23 in section 6.1 make the identifier a key for
each statement, so the statements about one identifier must merge
into one. Definition 4 and Table 3 in section 5.1 say which
parameters are expandable, and
[Constraints 53](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/#impossible-property-overlap),
[54](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/#impossible-object-property-overlap)
and
[55](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/#entity-activity-disjoint)
in section 6.4 are the impossibility constraints `check_kinds`
applies.

### Why `At` and `InRecord` rather than a position on every variant?

The PROV-N reader reports `Error::Syntax` for text the grammar rejects
and `Error::At` around the error for a name, literal or record, and
the PROV-JSON reader reports `Error::InRecord` naming the section and
the record's key around an error found while building a record
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"Every error from the PROV-N reader carries a line and a column"). The
PROV-N reader knows where each token sits, and a PROV-JSON error is
more useful with its record. One helper in the parser wraps an error
from the name, value or record layer in `At` at its token, and the
PROV-JSON reader wraps one in `InRecord` the same way. `Display` prints
the inner error, so it is not also the `source()`.

## The Rust here

- **Internal `@rule` arms and nested repetition in `macro_rules!`**. A
  rule whose pattern starts with a marker is called by the macro itself
  to pick a piece of code by a row's shape, as `records!(@id_type
  $shape)` does, and the `$($field: $attr,)*` inside each row is a
  repetition nested in the one over rows; chapter 2's `names!` used a
  single level.
  [Repetitions](https://doc.rust-lang.org/reference/macros-by-example.html#repetitions),
  [Internal rules](https://veykril.github.io/tlborm/decl-macros/patterns/internal-rules.html)
- **A macro that defines macros**. The template of `formals!` writes
  `macro_rules! formal_type` with one rule per row, so the table of
  formals is the one place a formal's type is stated.
  [Macros by example](https://doc.rust-lang.org/reference/macros-by-example.html)
- **`#[macro_use]` and textual macro scope**. A `macro_rules!` macro is
  in scope from its definition to the end of the enclosing module, and
  `#[macro_use]` on a `mod` carries its macros out to the text after the
  declaration, which is why `mod formal;` is the first item.
  [The macro_use attribute](https://doc.rust-lang.org/reference/macros-by-example.html#the-macro_use-attribute)
- **`#[non_exhaustive]`**. A `match` on `Kind` or `Record` in another
  crate must have a wildcard arm, so a variant added later is not a breaking
  change; `Error` puts it on its struct variants too, so a pattern there
  ends in `..`.
  [The non_exhaustive attribute](https://doc.rust-lang.org/reference/attributes/type_system.html#the-non_exhaustive-attribute)
- **Associated constants**. `Kind::ALL` is a `const` inside an `impl`,
  reached through the type, and a slice as its type leaves the length
  out of the type, so the list can grow with the table.
  [Associated constants](https://doc.rust-lang.org/reference/items/associated-items.html#associated-constants)
- **Owned and borrowed twins**. `Formal` holds values and `FormalRef`
  holds references with a lifetime, with `From<FormalRef<'_>> for
  Formal` as the one-way bridge; chapter 2's `QName` borrowed with `&str`,
  and here the borrowed form is a type of its own.
  [Validating references with lifetimes](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)
- **A generic method bounded by a private trait**. `Formals::take<T:
  FormalValue>` is one function for `QName` and `DateTime`, and the
  caller never names `T`, because the field being assigned fixes it;
  chapter 3's `impl Into<T>` arguments are the same mechanism on the
  argument side.
  [Generic data types, in function definitions](https://doc.rust-lang.org/book/ch10-01-syntax.html#in-function-definitions)
- **`pub(super)`**. `FormalValue` is visible in the parent module,
  `record`, and its children, and nowhere else, which is narrower than the
  `pub(crate)` chapter 1 showed; `Formals` and its methods carry the same
  marker.
  [pub(in path), pub(crate), pub(super), and pub(self)](https://doc.rust-lang.org/reference/visibility-and-privacy.html#pubin-path-pubcrate-pubsuper-and-pubself)
- **`Cow`**. `Cow<'_, [Bundle]>` is either `Borrowed` of the document's
  own slice or `Owned` of a `Vec`, and a caller reads both as a slice,
  so the common case allocates nothing; `clip` returns a `Cow<str>` for
  the same reason.
  [Cow](https://doc.rust-lang.org/std/borrow/enum.Cow.html)
- **`HashSet` and `HashMap` with borrowed keys**. A `HashSet<&QName>` in
  `merged_bundles` or a `HashSet<&Record>` in `same_records` holds
  references into the document, so building one clones nothing, and
  the set lives no longer than the borrow; chapter 2's
  maps owned their keys.
  [HashSet](https://doc.rust-lang.org/std/collections/struct.HashSet.html),
  [Storing keys with associated values in hash maps](https://doc.rust-lang.org/book/ch08-03-hash-maps.html)
- **`chain` and `flat_map`**. `chain` runs a second iterator after the
  first, and `flat_map` turns each bundle into an iterator over its
  records and flattens them, so one expression in `all_records` walks
  every record.
  [Iterator::flat_map](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.flat_map)
- **An `impl` block in another module**. Inherent methods may be written
  in any module of the defining crate, so `unified` lives beside the
  merging code it calls and still appears on `Document`.
  [Inherent implementations](https://doc.rust-lang.org/reference/items/implementations.html#inherent-implementations)
- **An enum as a state machine and `std::mem::replace`**. The variant
  of `State` is the state and `InBundle` carries the data that exists
  only in that state; `mem::replace` swaps a new state in and hands the old one back,
  so the bundle moves out without a clone.
  [mem::replace](https://doc.rust-lang.org/std/mem/fn.replace.html)
- **`debug_assert!`**. It checks an invariant in debug builds and
  compiles to nothing in release builds, which fits the builder's
  checks on callers inside the crate.
  [debug_assert](https://doc.rust-lang.org/std/macro.debug_assert.html)
- **`thiserror` derives**. `#[derive(thiserror::Error)]` on `Error`
  with `#[error("...")]` on each variant writes `Display` and
  `std::error::Error`, `#[from]` writes the `From` impl that `?` uses,
  `#[source]` names the cause, and `#[error(transparent)]` forwards both
  to the payload.
  [thiserror](https://docs.rs/thiserror/latest/thiserror/)
- **A recursive type through `Box`**. An enum cannot contain itself by
  value, because its size would have no bound, so the inner error of
  `At` lives on the heap behind a `Box<Error>`; chapter 2's `Box<str>` was the same
  pointer holding text.
  [Enabling recursive types with boxes](https://doc.rust-lang.org/book/ch15-01-box.html#enabling-recursive-types-with-boxes)
- **A `Result` type alias**. `type Result<T> = std::result::Result<T,
  Error>` fixes the error type once, so every signature in the crate
  writes `Result<T>`.
  [Type aliases](https://doc.rust-lang.org/reference/items/type-aliases.html)

## Run this

```sh
cargo test -p provkit formals_round_trip_through_from_parts_for_every_kind
```

This builds a record of every kind through `from_parts` with every
formal filled, given in reverse order, checks that `formals` returns
them in table order and that `formal` agrees, and rebuilds the record
from them to the same value and hash. The unit-test binary reports
the one test; the other test binaries report zero.

```text
test result: ok. 1 passed
```

```sh
cargo test -p provkit equality_is_a_set_over_records
```

This checks that two documents holding the same records in a different
order, one of them with a repeat, are equal, and that one more record
makes them unequal.

```text
test result: ok. 1 passed
```

```sh
cargo test -p provkit merges_formals_and_attributes
```

This unifies two generations under one identifier, one with an [entity](../reference/glossary.md#entity-prov)
and one with an [activity](../reference/glossary.md#activity-prov), and checks that the merged record holds
both, keeps the first position and carries both labels.

```text
test result: ok. 1 passed
```

```sh
cargo test -p provkit -- --exact builder::tests::duplicate_bundle
```

This starts a bundle, ends it and starts one with the same identifier,
and checks for `Error::DuplicateBundle` naming it. `--exact` matches
the whole path, because a PROV-JSON reader test also has
`duplicate_bundle` in its name.

```text
test result: ok. 1 passed
```

## Try it

Write two PROV-N files that describe one activity. `/tmp/e.provn`
gives its times in two records, each with the other time absent.

```text
document
  prefix ex <http://example.org/>
  activity(ex:a, 2020-01-01T00:00:00, -)
  activity(ex:a, -, 2020-01-02T00:00:00)
endDocument
```

`/tmp/f.provn` gives both times in one record.

```text
document
  prefix ex <http://example.org/>
  activity(ex:a, 2020-01-01T00:00:00, 2020-01-02T00:00:00)
endDocument
```

Predict what `provkit compare /tmp/e.provn /tmp/f.provn` prints and
what it exits with. Then predict the same command with `--unified`.

<details><summary>Answer</summary>

Without the flag the two documents are two records against one, and
no record on either side equals one on the other, so `compare` lists
both of `e`'s records and `f`'s one and exits 1.

```sh
provkit compare /tmp/e.provn /tmp/f.provn; echo $?
```

```text
only in /tmp/e.provn:
  activity(ex:a, 2020-01-01T00:00:00, -)
  activity(ex:a, -, 2020-01-02T00:00:00)
only in /tmp/f.provn:
  activity(ex:a, 2020-01-01T00:00:00, 2020-01-02T00:00:00)
1
```

With `--unified` each side is unified before the comparison. In
`e` the two activities share an identifier and a kind, and in each
position an absent time meets a present one, which is a start or end
time and not one of the four "none" positions, so the absent value
takes the present one. The merged record equals `f`'s, nothing is
printed and the exit status is 0.

```sh
provkit compare --unified /tmp/e.provn /tmp/f.provn; echo $?
```

```text
0
```

</details>

## Idioms first seen here

- `internal @rule arms and nested repetition in macro_rules!`: https://doc.rust-lang.org/reference/macros-by-example.html#repetitions
- `a macro that defines macros`: https://doc.rust-lang.org/reference/macros-by-example.html
- `#[macro_use] and textual macro scope`: https://doc.rust-lang.org/reference/macros-by-example.html#the-macro_use-attribute
- `#[non_exhaustive]`: https://doc.rust-lang.org/reference/attributes/type_system.html#the-non_exhaustive-attribute
- `associated constants`: https://doc.rust-lang.org/reference/items/associated-items.html#associated-constants
- `owned and borrowed twins`: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
- `a generic method bounded by a private trait`: https://doc.rust-lang.org/book/ch10-01-syntax.html#in-function-definitions
- `pub(super)`: https://doc.rust-lang.org/reference/visibility-and-privacy.html#pubin-path-pubcrate-pubsuper-and-pubself
- `Cow`: https://doc.rust-lang.org/std/borrow/enum.Cow.html
- `HashSet and HashMap with borrowed keys`: https://doc.rust-lang.org/std/collections/struct.HashSet.html
- `chain and flat_map`: https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.flat_map
- `an impl block in another module`: https://doc.rust-lang.org/reference/items/implementations.html#inherent-implementations
- `an enum as a state machine and std::mem::replace`: https://doc.rust-lang.org/std/mem/fn.replace.html
- `debug_assert!`: https://doc.rust-lang.org/std/macro.debug_assert.html
- `thiserror derives`: https://docs.rs/thiserror/latest/thiserror/
- `a recursive type through Box`: https://doc.rust-lang.org/book/ch15-01-box.html#enabling-recursive-types-with-boxes
- `a Result type alias`: https://doc.rust-lang.org/reference/items/type-aliases.html
