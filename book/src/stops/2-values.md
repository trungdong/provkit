# Stop 2: Values

An attribute value is a string, a language-tagged string, a name, a
time or a typed literal, and a record carries its values as a set of
name and value pairs. This stop reads how each kind is stored and when
two of them are equal, which is the rule every later layer leans on,
from the deduplication of a record's attribute set to `compare` in the
command line. It builds on stop 1 for `QName`, which is both a kind of
value and the key of every pair, and for the `well_known` statics the
datatype checks compare against.

## What the code does

### provkit/src/value/mod.rs

The file opens with the two payload types that `Value` wraps.
[`LangStr`](../api/provkit/struct.LangStr.html) is a text and a BCP 47
tag; its `PartialEq` compares the tag case-insensitively and its `Hash`
lowercases the tag's bytes, so equal strings hash alike.
[`TypedLiteral`](../api/provkit/struct.TypedLiteral.html) is a text
and a datatype with both fields private, so another crate builds one
only through `Value::typed` or a `From` conversion and cannot change
the datatype afterwards; the two `compile_fail` doctests on the struct
show that both doors are shut. Then comes the enum.

```rust,ignore
{{#include ../../../provkit/src/value/mod.rs:value}}
```

The book shows this span and those that follow as they stand in the
files, and compiles none of them. A
[`Value`](../api/provkit/enum.Value.html) is one of five variants, and
each carries its payload. `Str` is an `xsd:string`, `LangStr` and
`QName` wrap the type above and stop 1's name, `DateTime` is a parsed
`xsd:dateTime`, and `Typed` is everything else, kept as written.
`Value::typed`, below the enum, is where a text and a datatype become
one of them. An `xsd:string` folds to `Str`. An `xsd:dateTime` that
parses becomes `DateTime`, after the space, tab, CR and LF around it
are trimmed. A qualified-name datatype is `Error::InvalidLiteral`,
because resolving the text needs a prefix table, and the crate-private
`typed_in`, which takes a `Scope`, is the path both readers use
instead. Every other datatype, and a `dateTime` that does not parse,
is `Typed`. The row of
[CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md)
beginning "A value has one representation" states the rule this
routing serves. `"x"` and `"x" %% xsd:string` denote one value, so they
must be one variant.

Equality and hashing come after the constructors.

```rust,ignore
{{#include ../../../provkit/src/value/mod.rs:value_equality}}
```

`PartialEq` for `Value` asks for the same variant and then equal
payloads, and two different variants are never equal. `Hash` feeds the
variant's discriminant and then the payload, so the two agree. Stop 1
wrote `QName`'s impls by hand to compare the IRI; here the comment
above `impl PartialEq` gives a different reason, which the first card
below takes up. The rule for typed literals sits in `TypedLiteral`'s
impls, above the enum, and in `Key`. Two typed literals are equal when
their datatypes are equal and their texts are equal or their keys are.
`Key::of` trims the XSD white space and reads the text under its
datatype. An integer type in `INTEGER_TYPES` gives `Key::Integer`, a
sign and the digits without leading zeros, but only when the number is
within the type's bounds. `xsd:decimal` gives `Decimal` with the
fraction's trailing zeros dropped. `xsd:double` and `xsd:float` give
the bits of the number, with one `NaN` and negative zero as zero, and
a float is read in single precision. `xsd:boolean` gives `Boolean`.
Any other datatype, and any text outside the pattern, gives
`Key::Text` over the untrimmed text, so `"abc"` as `xsd:int` and
`"300"` as `xsd:byte` compare as text. `Key` borrows slices of the
literal, so building one allocates nothing, and `TypedLiteral::hash`
hashes the datatype and then the key, so equal literals hash alike.
The doctest below is the rule in one program, and the book compiles
it.

```rust
# extern crate provkit;
use provkit::{Attributes, DateTime, QName, Value, well_known::{XSD_INT, XSD_LONG}};

fn main() -> Result<(), provkit::Error> {
    let int = |text: &str| Value::typed(text, XSD_INT.clone());
    assert_eq!(int("01")?, int("1")?);
    assert_eq!(int(" 1 ")?, int("+1")?);
    assert_ne!(int("1")?, Value::typed("1", XSD_LONG.clone())?);
    assert_ne!(int("abc")?, int("abd")?);
    assert_ne!(Value::from("01"), Value::from("1"));
    assert_eq!(Value::from(5), int("5")?);
    assert_ne!(Value::from(5i64), int("5")?);

    let zoned = DateTime::parse("2011-11-16T12:00:00Z")?;
    let local = DateTime::parse("2011-11-16T12:00:00")?;
    assert_eq!(zoned, DateTime::parse("2011-11-16T13:00:00+01:00")?);
    assert_ne!(zoned, local);
    assert_eq!(zoned.partial_cmp(&local), None);

    let name = QName::new("http://example.org/", "n")?;
    let mut attrs = Attributes::new();
    assert!(attrs.insert(name.clone(), int("01")?));
    assert!(!attrs.insert(name.clone(), int("1")?));
    assert!(attrs.insert(name, "1"));
    assert_eq!(attrs.len(), 2);
    Ok(())
}
```

The `From` impls are the last items before the tests.

```rust,ignore
{{#include ../../../provkit/src/value/mod.rs:value_from}}
```

Two `macro_rules!` write one `From` impl per row of their input.
`from_native!` maps `i32` to `xsd:int`, `i64` to `xsd:long` and `bool`
to `xsd:boolean`, with `to_string` as the text, so the datatype follows
the Rust type and `Value::from(5i64)` is an `xsd:long`, which is not
equal to `Value::from(5i32)`. `from_float!` spells `NaN`, `INF` and
`-INF` as XSD does and picks between plain digits and the exponent form
by magnitude, with the thresholds the comment gives. After the macros,
`From<&str>` and `From<String>` give `Str`, and `QName`, `DateTime` and
`LangStr` each map to their own variant. `Attributes::insert` takes
`impl Into<Value>`, so `insert(name, 3)` and `insert(name, "text")`
both work through these impls. The readers do not use them for a bare
integer literal; they call `Value::integer`, which keeps the text and
types it with `integer_datatype` by range.

### provkit/src/datetime/mod.rs

[`DateTime`](../api/provkit/struct.DateTime.html) is the parsed form
of an `xsd:dateTime`.

```rust,ignore
{{#include ../../../provkit/src/datetime/mod.rs:datetime}}
```

The struct holds the components XSD names, with the year an `i64` and
the fraction of a second as nanoseconds, an `offset` in minutes from
UTC that is `None` for a time written without a zone, and `source`.
`parse` reads the lexical form, splitting at the `T` and then reading
the date, the time and the zone with one small function each, and
every refusal is `Error::InvalidLiteral` with the `xsd:dateTime`
datatype and a reason. It stores the text it read in `source`. `new`
builds a value from components and leaves `source` empty. `Display`
prints `source` when there is one and otherwise the canonical text,
with hour 24 rolled to the next day's midnight, and `FromStr` calls
`parse`, so `text.parse::<DateTime>()` works. The equality rule is the
impls after the `impl DateTime` block.

```rust,ignore
{{#include ../../../provkit/src/datetime/mod.rs:datetime_order}}
```

`key` converts the components to seconds since 1970 in `i128`, taking
the offset off a zoned time so that `13:00:00+01:00` and `12:00:00Z`
give one key, and pairs that with the nanoseconds. Working in `i128`
means no year `parse` accepts can overflow. `eq` asks whether both are
zoned or both unzoned and then compares keys, `Hash` feeds the same two
facts, and `partial_cmp` returns `None` when one is zoned and the other
not, and the key order otherwise. There is no `Ord`. The fourth card
below sets that rule beside XSD's, which orders some zoned and unzoned
pairs. Hour 24 needs no special case here, because a day's seconds
plus 24 hours is the next day's midnight.

### provkit/src/attributes.rs

[`Attributes`](../api/provkit/struct.Attributes.html) is the set of
pairs a record carries.

```rust,ignore
{{#include ../../../provkit/src/attributes.rs:attributes}}
```

The struct has one field, an `IndexSet` of `(QName, Value)` pairs
behind an `Arc`. `IndexSet` keeps insertion order and refuses a pair already present,
and it tells a pair is present by the `Hash` and `Eq` of the tuple,
which are `QName`'s from stop 1 and `Value`'s from above. So
`ex:v="01" %% xsd:int` after `ex:v="1" %% xsd:int` is not added, and
the first spelling stays. `get` by name walks the pairs, because the
set is keyed by the whole pair.

```rust,ignore
{{#include ../../../provkit/src/attributes.rs:attributes_insert}}
```

`Arc::make_mut` is the copy on write. When this `Attributes` is the
only owner of the set, it hands back a mutable reference to it; when a
clone shares the set, it copies the pairs into a fresh allocation first
and leaves the other owner on the old one. `remove` checks `contains`
before it reaches for `make_mut`, so removing an absent pair leaves
the sharing in place, and `Extend` peeks at its iterator for the same
reason. `extend_from` into an empty set does the reverse and clones the
`Arc` itself. After the `impl Attributes` block come three groups of
items. `PairKey` is a borrowed pair, and `indexmap`'s `Equivalent`
trait lets `remove` look up the owned pair through it without cloning
either half. `Iter` and `IntoIter` wrap `indexmap`'s iterators. The
`IntoIterator`, `FromIterator` and `Extend` impls let `for`, `collect`
and `extend` work on the type.

```rust,ignore
{{#include ../../../provkit/src/attributes.rs:attributes_hash}}
```

`PartialEq`, above this span, goes through `IndexSet`'s own
equality, same length and every pair present, so order plays no part.
`Hash` has to agree, and a hasher fed the pairs in insertion order
would not. It hashes each pair with `PAIR_HASHER`, one `RandomState`
keyed once per process, sorts the results, and feeds the length and
the sorted hashes to the outer hasher. Two sets that hold the same
pairs in any order hash alike.

## Why this shape

### Why compare numbers by value within one datatype?

Two rows of
[CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md)
fix the rule. The one beginning "A typed literal is never rejected for
its text" says a typed literal is kept as written whatever its text,
because production [41] in section 3.7.3 of
[PROV-N](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prod-typedLiteral)
puts no constraint on the string, and that values compare by value
under one datatype, so `"5"`, `" 5 "`, `"+5"` and `"005"` are one
`xsd:int` and `"1e999"` equals `"INF"` as an `xsd:double`. The row
beginning "Values of the numeric and boolean datatypes compare by
value" gives the reason, equal values under one datatype are one
value, and records that ProvPy compares `xsd:int`, `xsd:double` and
`xsd:boolean` by value and `xsd:long`, `xsd:integer` and `xsd:float`
by text. The doc comment on `Value` adds the two edges. Text outside
the datatype's lexical space compares as text, which is why
`Key::integer` checks the bounds and `"300"` as `xsd:byte` has no
integer key, and values of different datatypes are never equal, so
`"5"` as `xsd:int` and `"5"` as `xsd:long` are two values. The comment
above `impl PartialEq for Value` answers a narrower question, why the
impl is written out when a derived one would compare the same things.
The commit "Write out the equality of Value and Attributes again"
explains that a derived `PartialEq` also implements
`StructuralPartialEq`, which lets another crate match a `const Value`
as a pattern, and that is a promise the public surface does not make.

### Why are integers typed by range?

The departure row of
[CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md)
beginning "Integer literals are typed by range in both formats" gives
the reason. Section 3.7.3 of
[PROV-N](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prod-convenienceNotation)
calls a bare integer sugar for `xsd:int`, but a number outside the
`xsd:int` range has no valid `xsd:int` form, and the PROV-JSON note lets
a native number stand for `xsd:decimal`. One rule for both formats
means `[ex:k=3000000000]` equals the PROV-JSON `{"ex:k": 3000000000}`.
ProvToolbox reads every bare integer with `Integer.valueOf` and fails
above 2147483647, so the PROV-N writer writes an integer bare only when
it is an `xsd:int` and quotes an `xsd:long` or `xsd:integer` with its
datatype. `integer_datatype` is the one function that picks the type,
from the digits alone, so leading zeros do not change it. The `From`
impls need no range check, because a Rust `i32` is already within
`xsd:int` and an `i64` within `xsd:long`, so there the Rust type names
the datatype.

### Why keep the source text of a `dateTime`?

The doc comment on `DateTime` states it. The text a value was parsed
from is kept and printed back, and a value built from components prints
in canonical form. The commit "Add DateTime with XSD value-space
equality" introduced the field for that purpose, keeping the source
text for `Display` while comparing in value space. Two rows of
[CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md)
need it.
The row beginning "`xsd:dateTime` follows XSD 1.1" lets the fraction
have any number of digits, and the row beginning "`xsd:dateTime`
fractions beyond nine digits" holds equality, ordering and hashing to
nanoseconds while the source text keeps every digit. The row beginning
"`xsd:anyURI` stays a typed literal" states the principle for values
in general, writing returns what was read. So `2011-11-16T24:00:00Z`
compares equal to the next midnight and prints as written, and
`parses_and_keeps_source` in the tests checks the round trip.

### Why `PartialOrd` and no `Ord`?

Section 3.3.7.1 of
[XSD 1.1 Part 2](https://www.w3.org/TR/xmlschema11-2/#dateTime) says
that "dateTime and other types related to dates and times have only a
partial order", takes its equality and order from the seven-property
model of section D.2.1, and in section 3.3.7.3 lists the datatype's
`ordered` facet as partial. A note in 3.3.7.1 says the order of a
zoned value against an unzoned one is found by imputing offsets of
+14:00 and -14:00 to the unzoned one, and that many such pairs are
incomparable because the two imputations disagree. Rust's `Ord`
promises a total order, so the doc comment on `DateTime` says it
implements `PartialOrd` and not `Ord`. `partial_cmp` returns `None`
for every zoned and unzoned pair, which is a simpler rule than XSD's.
It leaves incomparable every pair XSD does, and also the pairs far
enough apart for both imputations to agree. The commit's note adds
that `Eq` still holds, because equality is reflexive where order is
undefined.

### Why an `Arc` with copy on write around the attribute set?

The doc comment on `Attributes` states the behaviour. Clones share one
allocation, and the first write to a clone copies the pairs. The
commit "Share attribute storage between clones" gives the case. A
PROV-JSON `hadMember` lists its members in an array and the reader
fans it out into one record per member, each carrying the same
attribute set, and a collection with thousands of members and as many
attributes used to deep-copy that set into every record. With the
`Arc` the records share one allocation, and
`member_fan_out_shares_one_attribute_allocation` in the JSON reader's
tests checks that they do. The same commit made `extend_from` into an
empty set clone the `Arc` rather than the pairs, so unifying a single
record copies nothing. `clones_share_until_written` is the rule in the
small. A clone shares until its first `insert`, and then it owns a
copy.

## The Rust here

- **`impl Into<String>` and `impl Into<Value>` arguments**, first used
  in `provkit/src/value/mod.rs` at `LangStr::new`. An argument typed
  `impl Into<T>` accepts anything that converts into `T`, so a caller
  passes a `&str` or a `String` to `new`, and a number, a `&str` or a
  `QName` to `Attributes::insert`.
  [Traits as parameters](https://doc.rust-lang.org/book/ch10-02-traits.html#traits-as-parameters)
- **`compile_fail` doctests**, first used in `provkit/src/value/mod.rs`
  at `TypedLiteral`. A doctest marked `compile_fail` passes only when
  it fails to compile, and the error code after the comma pins which
  error; the two on `TypedLiteral` show that its fields cannot be set
  from outside the crate.
  [Documentation tests, attributes](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html#attributes)
- **Enums with data in variants**, first used in
  `provkit/src/value/mod.rs` at `Value`. Each variant carries its own
  payload type, and a `match` on the enum binds the payload, as `eq`
  and `datatype` do.
  [Defining an enum](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html)
- **`FromStr` and `str::parse`**, first used in
  `provkit/src/value/mod.rs` at `integer_datatype`. `parse::<i64>()`
  turns text into a number through `i64`'s `FromStr` and returns a
  `Result`; `impl FromStr for DateTime` gives `DateTime` the same
  method.
  [FromStr](https://doc.rust-lang.org/std/str/trait.FromStr.html)
- **`std::mem::discriminant`**, first used in
  `provkit/src/value/mod.rs` at `impl Hash for Value`. It gives the
  variant of an enum value without its fields, which the hash feeds
  first so that two variants with equal payloads hash apart.
  [discriminant](https://doc.rust-lang.org/std/mem/fn.discriminant.html)
- **`macro_rules!` generating trait impls over a `$t:ty` list**, first
  used in `provkit/src/value/mod.rs` at `from_native!`. The `$t:ty`
  fragment captures a type, and the body writes one `impl From<$t>`
  per row; stop 1's `names!` used the same repetition for statics.
  [Macros by example](https://doc.rust-lang.org/reference/macros-by-example.html)
- **`From` and `Into`**, first used in `provkit/src/value/mod.rs` at
  `impl From<&str> for Value`. Implementing `From<T> for Value` gives
  `Value::from(t)` and, through the blanket impl, `t.into()`, which is
  what an `impl Into<Value>` argument calls.
  [Recoverable errors with Result](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html),
  [From](https://doc.rust-lang.org/std/convert/trait.From.html)
- **`use std::fmt::Write as _`**, first used in
  `provkit/src/datetime/mod.rs` at `use std::fmt::{self, Write as _}`.
  The `as _` brings the trait's methods into scope without binding its
  name, which is all `write!` into a `String` needs in `text`.
  [Underscore imports](https://doc.rust-lang.org/reference/items/use-declarations.html#underscore-imports)
- **`#[allow(clippy::too_many_arguments)]` with a reason**, first used
  in `provkit/src/datetime/mod.rs` at `DateTime::new`. The attribute
  silences one lint on one item, and the comment above it says why
  eight arguments are right here.
  [too_many_arguments](https://clippy.rust-lang.org/master/index.html#too_many_arguments),
  [Lint check attributes](https://doc.rust-lang.org/reference/attributes/diagnostics.html#lint-check-attributes)
- **`and_then`, `map_err` and `ok_or_else` chains**, first used in
  `provkit/src/datetime/mod.rs` at `DateTime::new` and `parse`.
  `and_then` runs the next check only when the last passed, `map_err`
  turns a `&str` reason into an `Error`, and `ok_or_else` turns a
  `None` from `split_once` into an `Err`, each before one `?`.
  [Result](https://doc.rust-lang.org/std/result/enum.Result.html)
- **`i128::from` widening**, first used in
  `provkit/src/datetime/mod.rs` at `date`. `From<i64> for i128` is
  lossless, so the calendar arithmetic in `key` and `days_from_civil`
  cannot wrap for any year the struct can hold.
  [i128](https://doc.rust-lang.org/std/primitive.i128.html)
- **`bool::then`**, first used in `provkit/src/datetime/mod.rs` at
  `impl PartialOrd for DateTime`. `cond.then(|| x)` is `Some(x)` when
  the condition holds and `None` otherwise, which is the whole of
  `partial_cmp`; stop 1's `then_some` is the same without a closure.
  [bool::then](https://doc.rust-lang.org/std/primitive.bool.html#method.then)
- **`IndexSet`**, first used in `provkit/src/attributes.rs` at
  `Attributes`. The set counterpart of stop 1's `IndexMap`, keeping
  insertion order and refusing duplicates by `Hash` and `Eq`.
  [IndexSet](https://docs.rs/indexmap/latest/indexmap/set/struct.IndexSet.html)
- **`Arc::make_mut` and copy on write**, first used in
  `provkit/src/attributes.rs` at `insert`. It returns a mutable
  reference to the value inside an `Arc`, cloning the value into a new
  allocation first when other owners share it.
  [Arc::make_mut](https://doc.rust-lang.org/std/sync/struct.Arc.html#method.make_mut)
- **`move` closures returning `impl Iterator + 'a`**, first used in
  `provkit/src/attributes.rs` at `get`. The closure takes ownership of
  the cloned `name`, so the returned iterator borrows only the set and
  the caller's key may be a temporary.
  [Closures](https://doc.rust-lang.org/book/ch13-01-closures.html)
- **Implementing `Iterator`, `DoubleEndedIterator`,
  `ExactSizeIterator`, `IntoIterator`, `FromIterator` and `Extend`**,
  first used in `provkit/src/attributes.rs` at
  `impl FromIterator<(QName, Value)> for Attributes`. `FromIterator`
  makes `collect` work, `Extend` makes `extend` work, `IntoIterator`
  for the type and for `&` of it makes `for` work, and the two wrapper
  iterators forward `next`, `next_back` and `size_hint` to `indexmap`'s.
  [std::iter](https://doc.rust-lang.org/std/iter/index.html)

## Run this

```sh
cargo test -p provkit integers_compare_by_value_under_one_datatype
```

This checks, for every XSD integer type, that a small number spelt
plain, padded, with leading zeros and with a `+` is one value, and
that the type's bounds are too. The unit-test binary reports the one
test; the other test binaries report zero.

```text
test result: ok. 1 passed
```

```sh
cargo test -p provkit xsd_equality
```

This checks that a zoned time equals its UTC spelling and hashes
alike, that an unzoned time is not equal to a zoned one and has no
order against it, and that a fraction of zeros changes nothing.

```text
test result: ok. 1 passed
```

```sh
cargo test -p provkit clones_share_until_written
```

This clones an attribute set, checks that the clone shares its
storage, writes to the clone and checks that the two sets now have
their own.

```text
test result: ok. 1 passed
```

## Try it

Write two PROV-N files that spell one `xsd:int` two ways. `/tmp/c.provn`
holds the value with a leading zero.

```text
document
  prefix ex <http://example.org/>
  entity(ex:e, [ex:n="01" %% xsd:int])
endDocument
```

`/tmp/d.provn` holds it without.

```text
document
  prefix ex <http://example.org/>
  entity(ex:e, [ex:n="1" %% xsd:int])
endDocument
```

Predict what `provkit compare /tmp/c.provn /tmp/d.provn` prints and
what it exits with. Then change both datatypes to `xsd:string` and
predict again.

<details><summary>Answer</summary>

Nothing is printed and the exit status is 0, because both values are
the integer one under one datatype and `compare` finds no record on
one side only.

```sh
provkit compare /tmp/c.provn /tmp/d.provn; echo $?
```

```text
0
```

Under `xsd:string` the two texts are two values, so `compare` lists
each record under the file it came from and exits 1. The datatype is
not printed, because `xsd:string` folds to `Value::Str` and the writer
spells a plain string without one.

```sh
provkit compare /tmp/c.provn /tmp/d.provn; echo $?
```

```text
only in /tmp/c.provn:
  entity(ex:e, [ex:n="01"])
only in /tmp/d.provn:
  entity(ex:e, [ex:n="1"])
1
```

</details>

## Idioms first seen here

- `impl Into<String> and impl Into<Value> arguments`: https://doc.rust-lang.org/book/ch10-02-traits.html#traits-as-parameters
- `compile_fail doctests`: https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html#attributes
- `enums with data in variants`: https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html
- `FromStr and str::parse`: https://doc.rust-lang.org/std/str/trait.FromStr.html
- `std::mem::discriminant`: https://doc.rust-lang.org/std/mem/fn.discriminant.html
- `macro_rules! generating trait impls over a $t:ty list`: https://doc.rust-lang.org/reference/macros-by-example.html
- `From and Into`: https://doc.rust-lang.org/std/convert/trait.From.html
- `use std::fmt::Write as _`: https://doc.rust-lang.org/reference/items/use-declarations.html#underscore-imports
- `#[allow(clippy::too_many_arguments)] with a reason`: https://clippy.rust-lang.org/master/index.html#too_many_arguments
- `and_then, map_err and ok_or_else chains`: https://doc.rust-lang.org/std/result/enum.Result.html
- `i128::from widening`: https://doc.rust-lang.org/std/primitive.i128.html
- `bool::then`: https://doc.rust-lang.org/std/primitive.bool.html#method.then
- `IndexSet`: https://docs.rs/indexmap/latest/indexmap/set/struct.IndexSet.html
- `Arc::make_mut and copy on write`: https://doc.rust-lang.org/std/sync/struct.Arc.html#method.make_mut
- `move closures returning impl Iterator + 'a`: https://doc.rust-lang.org/book/ch13-01-closures.html
- `implementing Iterator, DoubleEndedIterator, ExactSizeIterator, IntoIterator, FromIterator and Extend`: https://doc.rust-lang.org/std/iter/index.html
