# Chapter 2: Names

A PROV identifier is a [namespace](../reference/glossary.md#namespace-and-prefix-prov) IRI plus a local part. Everything a
[document](../reference/glossary.md#document-prov) names, from an [entity](../reference/glossary.md#entity-prov) to an [attribute](../reference/glossary.md#attribute-prov) key, is one of these,
and the two readers turn every `prefix:local` they meet into one. This
chapter describes the character rules the local part and the [prefix](../reference/glossary.md#namespace-and-prefix-prov) obey, the
name type itself, and the namespace tables that both readers resolve
through. It builds on chapter 1 only, for the module list and the
`pub use` lines that bring `QName`, `Namespaces` and `well_known` to
the crate root.

## What the code does

### provkit/src/pn.rs

The PROV-N grammar spells a name as `QUALIFIED_NAME`, production [52]
of section 3.7.1 of
[PROV-N](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prod-QUALIFIED_NAME),
and builds it from the character classes `PN_LOCAL` [53],
`PN_CHARS_OTHERS` [54], `PN_CHARS_ESC` [55] and `PERCENT` [56]. The
prefix follows `PN_PREFIX` from
[SPARQL 1.0](https://www.w3.org/TR/2008/REC-rdf-sparql-query-20080115/#rPN_PREFIX),
production [99], which rests on `PN_CHARS_BASE` [95], `PN_CHARS_U` [96]
and `PN_CHARS` [98]. This file holds those classes as small
functions, one per production, and two of them matter to every other
file in this chapter.

`prefix_len` measures a `PN_PREFIX` at the front of a string. The
first character must be in `PN_CHARS_BASE`, which holds letters and
the Unicode ranges SPARQL lists, and no `_` or digit. Later characters
may be any `PN_CHARS`, and a `.` may appear when something follows it.
So the loop moves `end` only past a `PN_CHARS` character, and it steps
over a `.` without extending the result. The book shows this span and
those that follow as they stand in the files, and compiles none of
them.

```rust,ignore
{{#include ../../../provkit/src/pn.rs:prefix_len}}
```

The lexer calls `prefix_len` to find where a prefix ends, and
`Namespaces::declare` calls `is_pn_prefix`, which asks whether the
whole string is one prefix.

The local part has a looser alphabet and an escape. `local_char`
answers, for one character at one position, how PROV-N writes it.
`LocalChar::Bare` means as it is, `LocalChar::Escaped` means after a
`\`, and `None` means no `PN_LOCAL` holds that character at that
position.

```rust,ignore
{{#include ../../../provkit/src/pn.rs:local_char}}
```

Five rules sit in `local_char`. A `-` is escaped only as the first
character, and a `.` only as the first or the last, because `PN_LOCAL` [53]
admits both bare anywhere else. The text then matches what
[Python `prov`](https://github.com/trungdong/prov)
3.3.1
writes. The other `PN_CHARS_ESC` characters, such as `:` and `=`, are always
written escaped. A `%` stands unescaped only when two hex digits follow,
because `PERCENT` is the only place `%` can stand. Everything else goes
through `is_bare_local_char`, which admits `PN_CHARS_U`, digits and the plain `PN_CHARS_OTHERS`
characters anywhere, and the rest of `PN_CHARS`, such as a combining
mark, only after the first character. `QName::new` and the PROV-N
writer both rely on this one function, so a name the constructor
accepts is a name the writer can print.

### provkit/src/qname.rs

[`QName`](../api/provkit/struct.QName.html) is two fields and nothing
else.

```rust,ignore
{{#include ../../../provkit/src/qname.rs:qname}}
```

The namespace is an `Arc<str>`, a reference-counted string that many
names share, and the local part is a `Box<str>`, one owned string per
name. Neither field is public. `QName::new` takes two `&str`, refuses
an empty or whitespace-only namespace, checks the local part with `check_local`, and builds the name; the
crate-private `in_namespace` takes an `Arc<str>` instead, so a name
resolved through a namespace table shares the table's allocation rather
than copying the IRI. The check refuses a character that no
`PN_LOCAL` holds at its position, with or without an escape, which covers a `%`
without two hex digits after it and a first character that may only
continue a name. An empty local part is valid.

Equality, ordering and hashing are written by hand, and all three
look at the joined IRI rather than at the two fields.

```rust,ignore
{{#include ../../../provkit/src/qname.rs:qname_identity}}
```

`iri_bytes` chains the bytes of the namespace with the bytes of the
local part without building a `String`, and `is` compares that stream
with another split pair after a cheap length check. `eq` and `cmp`
walk two such streams byte by byte. `Hash` cannot write the namespace
and the local part as two calls, because the `Hasher` contract does not
promise that two `write` calls equal one call with the joined bytes.
It copies the bytes through a fixed buffer instead, which makes fewer
calls than one `write_u8` per byte, and ends with a terminator as
`str` does. Two names with the same IRI then feed any
hasher the same calls, however their splits differ. The doctest below
is the rule in one program, and the book compiles it.

```rust
# extern crate provkit;
use provkit::{Namespaces, QName, well_known};

fn main() -> Result<(), provkit::Error> {
    let mut ns = Namespaces::new();
    ns.declare("ex", "http://example.org/")?;
    ns.set_default("http://example.org/")?;
    let prefixed = ns.resolve("ex:e1")?;
    let bare = ns.resolve("e1")?;
    assert_eq!(prefixed, bare);
    assert_eq!(prefixed, QName::new("http://example.org/", "e1")?);
    assert_eq!(prefixed, QName::new("http://example.org/e", "1")?);
    assert_eq!(prefixed.iri(), "http://example.org/e1");
    assert_eq!(well_known::PROV_TYPE.local(), "type");
    assert!(ns.declare("_", "http://example.org/").is_err());
    assert!(ns.declare("prov", "http://example.org/").is_err());
    Ok(())
}
```

After the impls comes [`well_known`](../api/provkit/well_known/index.html),
the one public module under `qname.rs` and the last item before its
tests, which holds the `prov:` and
`xsd:` names the rest of the crate compares against.

```rust,ignore
{{#include ../../../provkit/src/qname.rs:well_known}}
```

Each name is a `static` behind a `LazyLock`, built on its first use
and kept for the life of the program. A `static` needs a constant
initialiser and an `Arc` cannot be built in one, so the lazy wrapper
is what makes a `QName` static possible. The `names!` macro writes
one documented static per row, and the `#[doc = concat!(...)]` line
gives each the prefixed spelling, `prov:type` and so on, as its
rustdoc text.

### provkit/src/namespaces/mod.rs

[`Namespaces`](../api/provkit/struct.Namespaces.html) is the prefix
table a document or a [bundle](../reference/glossary.md#bundle-prov) carries.

```rust,ignore
{{#include ../../../provkit/src/namespaces/mod.rs:namespaces}}
```

The struct has four fields. `default` is the namespace a [bare name](../reference/glossary.md#bare-name-provkit) resolves in.
`prefixes` is an `IndexMap`, a map that remembers insertion order, so
a writer can print the declarations in the order the document made
them. `by_iri` is the reverse table the writers read, from an IRI to
every prefix bound to it, kept in step with `prefixes` by `bind` and
`unindex`. `next_minted` is where the search for a fresh `nsN` prefix
starts, which keeps minting many prefixes linear. The table decides
how names are written and never takes part in equality, and the rest
of the file obeys that rule.

`prov` and `xsd` are not in the table. They live in the `RESERVED`
static above the struct, and `lookup` and `resolve` consult that
static before the table, so every `Namespaces` answers for them from
birth. `declare` is how a caller puts a prefix into the table; the
writers' minting and `with_fallback` reach `bind` directly.

```rust,ignore
{{#include ../../../provkit/src/namespaces/mod.rs:declare}}
```

Three refusals come first. A prefix that `is_pn_prefix` rejects is
`Error::InvalidPrefix`, and that covers `_`, a prefix ending in `.`
and anything with a space or a colon. PROV-N cannot write any other
prefix, and a table that holds only valid prefixes writes to both
formats ([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"`Namespaces::declare` rejects a prefix"). An empty or
all-whitespace IRI is `Error::EmptyNamespace`, which names the prefix and
is a departure from the Recommendation
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"A namespace IRI that is empty or all whitespace"). `prov` or `xsd`
at any IRI but its fixed one is `Error::ReservedPrefix`, which carries
the prefix, the expected IRI and the one found. A declaration of `prov` or `xsd`
at the right IRI is accepted and changes nothing, so a PROV-JSON
document that restates a predeclared binding still reads, and so does
a PROV-N one under the `Default` profile. Everything else goes to
`bind`, which replaces an earlier binding of the same prefix in place
and keeps the reverse table in step.

The last item in this chapter is `Scope`, which is how a bundle sees the
document around it.

```rust,ignore
{{#include ../../../provkit/src/namespaces/mod.rs:scope}}
```

A `Scope` is two references, the bundle's own table and an optional
parent, and `tables()` yields them in that order. `resolve_parts`
walks the pair with `find_map`, so the first table that binds the
prefix, or the first that has a default namespace, answers. The bundle
wins where it speaks and the document fills in where it is silent, and
no table is copied or merged to get there. Both readers build a
`Scope::within` of the bundle's table and the document's for the
statements of a bundle;
`Namespaces::resolve` is the same path with no parent. The `_` check
sits here as well, before the table lookup, so `_:x` is reported as an
invalid name rather than an undeclared prefix. A caller who wants one
merged table instead gets it from `with_fallback`, further up the
file, which clones the bundle's table and absorbs the document's.

## Why this shape

### Why is the namespace an `Arc<str>` and the local part a `Box<str>`?

An `Arc<str>` is a reference-counted, immutable string, so cloning one
bumps a counter instead of copying text and many names share one
namespace allocation. The code makes the sharing concrete. A prefix
table stores each IRI as an `Arc<str>`, and `Scope::resolve_parts`
clones that `Arc` into every name it resolves through
`QName::in_namespace`, so the name shares its namespace with the other
names built from the same table. The local part is a `Box<str>`, one owned allocation per name with no
count to maintain, and `in_namespace` builds it fresh from the `&str`
it is given. A document read through one table holds the IRI of each
binding once, however many names resolve through it.

### Why is equality on the IRI and not the prefix?

A name denotes the IRI its two parts join to, and the prefix a
document used to spell the name is not stored. Names are equal when
their IRIs are equal, however the namespace and the local part split
the IRI, and ordering and hashing follow the IRI, because PROV-DM says
a [qualified name](../reference/glossary.md#identifier-and-qualified-name-prov)
denotes an IRI. Strict name resolution in the readers is the other
side of the same rule ([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"Name resolution is strict"). The hand-written impls above, rather than
derived field-wise ones, are what make `http://example.org/` with
`foo/bar` equal `http://example.org/foo/` with `bar`. A prefix is
therefore notation. `ex:e1` under `prefix ex`, `e1` under a default
namespace with the same IRI and `xx:e1` under another prefix are one
name, and the Try it below shows `compare` treating them as one.

### Why are `prov` and `xsd` reserved?

Section 3.7.4 of
[PROV-N](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prod-namespaceDeclaration)
says a namespace declaration must not declare the prefixes `prov` and
`xsd`; they are predeclared. `prov` or `xsd` at another IRI is
therefore `Error::ReservedPrefix`. Table 1 of PROV-N, the prefix table
of PROV-DM and the PROV-JSON submission all print
`http://www.w3.org/2000/10/XMLSchema#`, but the 2001 IRI is the XML
Schema namespace and Python `prov` uses it, so `well_known::XSD` holds the
2001 IRI and `declare("xsd", ...)` accepts only that one
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"`xsd` is `http://www.w3.org/2001/XMLSchema#`").

### Why is `_` never a prefix?

`_:` is the [blank-node](../reference/glossary.md#blank-node-prov)
form, which names no IRI, so PROV-JSON keys that start `_:` are blank
and PROV-N rejects `_:x` because it is not a `QUALIFIED_NAME`
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"The underscore is never a prefix"). Two checks enforce it, one per
caller. `PN_PREFIX` starts with a `PN_CHARS_BASE`
character, and `_` only enters at `PN_CHARS_U`, so `is_pn_prefix("_")`
is false and `declare` refuses it as an invalid prefix.
`Scope::resolve_parts` then checks for the
`_` prefix before it looks anything up, so a reader that hands it `_:x`
gets `Error::InvalidQName` naming the whole text rather than an
undeclared-prefix error that a declaration could seem to fix.

### Why a two-level `Scope`?

Section 3.7.4 of PROV-N scopes a prefix declaration made in a bundle
to that bundle, and lets a bundle re-declare a prefix the surrounding
document declared. Both readers follow that rule, and so do the
writers, which print a bundle's names with its own table in front of
the document's ([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"A bundle identifier resolves in the bundle's own scope first"). A `Scope` is `Copy` and holds two
references, so a reader builds one per bundle at no cost and neither
table moves or changes. `WriteScope`, further down the file, is the
same pair for the writers, with a mutable own table because minting a
prefix adds to it.

## The Rust here

- **Ownership and borrowing**. Functions such as `prefix_len` and
  `QName::new` take `&str`, a borrowed view of text someone else owns,
  and return a length or a new value; `QName::new` is where a borrowed
  `&str` is copied into an owned string the name keeps.
  [What is ownership?](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html),
  [References and borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html)
- **`Arc<str>`**. A reference-counted pointer to an immutable string,
  which is what `QName` holds as its namespace; cloning it bumps the
  count, and the text is freed when the last clone drops.
  [Arc](https://doc.rust-lang.org/std/sync/struct.Arc.html)
- **`Box<str>`**. An owned string on the heap with no capacity field and
  no count; the local part and the `prefixes` keys, which never change
  once built, use it instead of `String`.
  [Box](https://doc.rust-lang.org/std/boxed/struct.Box.html)
- **`#[derive(Clone)]`**. The derive writes a `clone` that clones each
  field, so a `QName` clone bumps the namespace count and copies the
  local part.
  [Derivable traits](https://doc.rust-lang.org/book/appendix-03-derivable-traits.html)
- **Hand-written `PartialEq`, `Eq`, `Ord`, `PartialOrd` and `Hash`**.
  The impls on `QName` replace the derived field-wise versions with
  ones over the IRI; `hash<H: Hasher>` is generic so any hasher can be fed, and
  `PartialOrd` delegates to `Ord` with `Some(self.cmp(other))`.
  [PartialEq](https://doc.rust-lang.org/std/cmp/trait.PartialEq.html),
  [Hash](https://doc.rust-lang.org/std/hash/trait.Hash.html)
- **`impl Iterator<Item = u8> + '_` and lifetime elision**. The return
  type of `iri_bytes` names a trait, not a concrete iterator, and `'_`
  says the iterator borrows from `self` for as long as the elided input
  lifetime.
  [Validating references with lifetimes](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html),
  [Lifetime elision](https://doc.rust-lang.org/reference/lifetime-elision.html)
- **Iterator adaptors `all`, `eq` and `cmp`**. `all` in
  `hex_pair_follows` asks whether every item passes a test; `eq` and
  `cmp`, in `QName::is` and `Ord::cmp`,
  compare two iterators item by item, which gives byte order without a
  `String`.
  [Iterator](https://doc.rust-lang.org/std/iter/trait.Iterator.html)
- **`matches!`**. The macro tests a value against a pattern and returns
  a `bool`, so a character class such as `is_pn_chars_base` is one
  pattern of ranges joined by `|`.
  [matches](https://doc.rust-lang.org/std/macro.matches.html)
- **`?` on `Option` and `Result`**. `chars.next()?` in `prefix_len`
  returns `None` from the function when the string is empty; in
  `QName::in_namespace`,
  `check_local(local)?` returns the `Err` early.
  [Recoverable errors with Result](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html)
- **`then_some` and `is_some_and`**. `is_some_and` tests the value
  inside an `Option` and is false for `None`; `then_some` on a `bool`
  makes a `Some` when true and `None` when false, as `local_char` does
  for `%`.
  [Option](https://doc.rust-lang.org/std/option/enum.Option.html)
- **`LazyLock`**. A `static` whose value is built by a closure on first
  access and shared after, which lets a `QName`, with an `Arc` inside,
  be a static in `well_known` at all.
  [LazyLock](https://doc.rust-lang.org/std/sync/struct.LazyLock.html)
- **`macro_rules!` with `$(...)*` repetition**. The matcher of `names!`
  captures one row per repetition and the body is written out once per
  row, which is how every static in `well_known` comes from one
  template.
  [Macros](https://doc.rust-lang.org/book/ch20-05-macros.html),
  [Macros by example](https://doc.rust-lang.org/reference/macros-by-example.html)
- **`#[doc = concat!(...)]`**. A doc attribute takes a string literal or
  a macro call that expands to one, so `names!` builds each static's
  rustdoc text from its prefix and local part with `concat!`.
  [The #[doc] attribute](https://doc.rust-lang.org/rustdoc/write-documentation/the-doc-attribute.html)
- **`IndexMap`**. A hash map from the `indexmap` crate that keeps
  insertion order, so iteration and `get_index_of` over `prefixes`
  follow declaration order.
  [indexmap](https://docs.rs/indexmap)
- **`let ... else`**. The pattern must match or the `else` block must
  diverge, by a `return` as in `unindex`, a `break`, a `continue` or a
  panic; the value is then bound without a nested `if let`.
  [let statements](https://doc.rust-lang.org/reference/statements.html#let-statements)
- **`entry().or_default()` and `partition_point`**. In `bind`, `entry`
  gives one place to insert or update a map value, and `partition_point`
  finds where a new prefix goes in a sorted `Vec` by binary search.
  [HashMap::entry](https://doc.rust-lang.org/std/collections/hash_map/struct.HashMap.html#method.entry),
  [slice::partition_point](https://doc.rust-lang.org/std/primitive.slice.html#method.partition_point)
- **`#[must_use]`**. The compiler warns when a caller drops the result,
  which matters for `with_fallback`, a method that returns a new table
  and leaves `self` unchanged.
  [The must_use attribute](https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-must_use-attribute)
- **A `Copy` struct with a lifetime**. `Scope<'a>` holds two references
  that live for `'a`, so it cannot outlive the tables, and deriving
  `Copy` means passing it by value costs two pointers.
  [Validating references with lifetimes](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)
- **`mod tests;` in a sibling file**. A `mod` line with no body loads
  `namespaces/tests.rs`, so the tests sit beside the code but out of the
  file; `qname.rs` keeps its tests inline instead.
  [Test organization](https://doc.rust-lang.org/book/ch11-03-test-organization.html)

## Run this

```sh
cargo test -p provkit identity_is_the_iri
```

This checks that two splits of one IRI are equal, hash alike and
order `Equal`, including past the 64-byte hash buffer, and that two
IRIs that differ in Unicode normalisation are two names. The unit-test
binary reports the one test; the other test binaries report zero.

```text
test result: ok. 1 passed
```

```sh
cargo test -p provkit resolves_prefixed_bare_and_predeclared
```

This resolves `ex:e1` through a declared prefix, `e1` through the
default namespace, and `prov:type` and `xsd:int` through the
predeclared bindings of a fresh table.

```text
test result: ok. 1 passed
```

```sh
cargo test -p provkit a_bundle_table_falls_back_to_the_document
```

This builds a document table with `ex` and a bundle table with `b`,
merges them with `with_fallback`, and resolves a name through each
prefix.

```text
test result: ok. 1 passed
```

## Try it

Write two PROV-N files that spell one entity two ways. `/tmp/a.provn`
declares a prefix and uses it.

```text
document
  prefix ex <http://example.org/>
  entity(ex:e1)
endDocument
```

`/tmp/b.provn` sets a default namespace and uses a bare name.

```text
document
  default <http://example.org/>
  entity(e1)
endDocument
```

Predict what `provkit compare /tmp/a.provn /tmp/b.provn` prints and
what it exits with. Then change `b` to `prefix xx <http://example.org/>`
and `entity(xx:e1)` and predict again.

<details><summary>Answer</summary>

Nothing is printed and the exit status is 0, because both names are
`http://example.org/e1` and `compare` finds no [record](../reference/glossary.md#record-provkit) on one side
only.

```sh
provkit compare /tmp/a.provn /tmp/b.provn; echo $?
```

```text
0
```

The `xx` spelling gives the same answer, because the prefix is not
part of the name.

```sh
provkit compare /tmp/a.provn /tmp/b.provn; echo $?
```

```text
0
```

To see what a difference looks like, change `/tmp/b.provn` to `prefix ex
<http://example.org/>` and `entity(ex:e2)`, so that only the local part
differs from `/tmp/a.provn`. Now the IRIs differ and `compare` lists each
record under the file it came from, and exits 1.

```sh
provkit compare /tmp/a.provn /tmp/b.provn; echo $?
```

```text
only in /tmp/a.provn:
  entity(ex:e1)
only in /tmp/b.provn:
  entity(ex:e2)
1
```

</details>

## Idioms first seen here

- `ownership and borrowing`: https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html
- `Arc<str>`: https://doc.rust-lang.org/std/sync/struct.Arc.html
- `Box<str>`: https://doc.rust-lang.org/std/boxed/struct.Box.html
- `#[derive(Clone)]`: https://doc.rust-lang.org/book/appendix-03-derivable-traits.html
- `hand-written PartialEq, Eq, Ord, PartialOrd and Hash`: https://doc.rust-lang.org/std/hash/trait.Hash.html
- `impl Iterator + '_ and lifetime elision`: https://doc.rust-lang.org/reference/lifetime-elision.html
- `iterator adaptors all, eq and cmp`: https://doc.rust-lang.org/std/iter/trait.Iterator.html
- `matches!`: https://doc.rust-lang.org/std/macro.matches.html
- `? on Option and Result`: https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html
- `then_some and is_some_and`: https://doc.rust-lang.org/std/option/enum.Option.html
- `LazyLock`: https://doc.rust-lang.org/std/sync/struct.LazyLock.html
- `macro_rules! with $(...)* repetition`: https://doc.rust-lang.org/reference/macros-by-example.html
- `#[doc = concat!(...)]`: https://doc.rust-lang.org/rustdoc/write-documentation/the-doc-attribute.html
- `IndexMap`: https://docs.rs/indexmap
- `let ... else`: https://doc.rust-lang.org/reference/statements.html#let-statements
- `entry().or_default() and partition_point`: https://doc.rust-lang.org/std/primitive.slice.html#method.partition_point
- `#[must_use]`: https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-must_use-attribute
- `Copy struct with a lifetime`: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
- `mod tests; in a sibling file`: https://doc.rust-lang.org/book/ch11-03-test-organization.html
