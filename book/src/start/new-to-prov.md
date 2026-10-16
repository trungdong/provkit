# New to PROV

PROV is the W3C family of specifications for describing where things
came from. Three of them matter here. [PROV-DM](https://www.w3.org/TR/2013/REC-prov-dm-20130430/)
is the data model, [PROV-N](https://www.w3.org/TR/2013/REC-prov-n-20130430/)
is a notation for reading and writing it, and
[PROV-CONSTRAINTS](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/)
says when two descriptions count as the same. provkit also reads and
writes PROV-JSON, a [member submission](https://www.w3.org/Submission/2013/SUBM-prov-json-20130424/)
for exchanging documents between programs. PROV-N is the format for
people, implemented at [Stop 5](../stops/5-provn.md), and PROV-JSON is the
format for programs, implemented at [Stop 4](../stops/4-json.md). The
[glossary](../reference/glossary.md) defines each term below.

## A document is a set of records

[PROV-DM section 2](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#section-prov-overview)
introduces three kinds of thing, entities, activities and agents, and
the relations between them. A provkit `Document` holds the records that
describe them, together with a namespace table. [Stop 3](../stops/3-records.md)
builds it.

## Every record is one of eighteen kinds

[PROV-DM section 5](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#data-model-components)
defines the kinds, grouped into six components. [PROV-N section 3](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prov-n-expressions)
gives each kind a statement form, such as `used(a, e, t)`. The
`records!` table at [Stop 3](../stops/3-records.md) states the same list
once, and the rest of the crate follows from it.

## Names are IRIs written with prefixes

An identifier is a qualified name, a prefix and a local part
([PROV-DM section 5.7.1](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-identifier)
and [section 5.7.5](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-qualified-name)).
A namespace declaration binds the prefix to an IRI
([PROV-DM section 5.7.4](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-NamespaceDeclaration),
[PROV-N section 3.7.4](https://www.w3.org/TR/2013/REC-prov-n-20130430/#expression-NamespaceDeclaration)),
so `ex:e1` with `ex` bound to `http://example.org/` is the IRI
`http://example.org/e1`. Two names are equal when their IRIs are equal,
whatever prefix each was written with. [Stop 1](../stops/1-names.md)
implements this.

## Values are literals with a datatype or a language

A value is a literal, written with an XSD datatype or a language tag
([PROV-DM section 5.7.3](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-value),
[PROV-N section 3.7.3](https://www.w3.org/TR/2013/REC-prov-n-20130430/#expression-literal)).
Equality depends on the datatype. Two `xsd:int` literals `"01"` and `"1"`
are the same number, and two `dateTime` values are the same instant
however they are spelled, but the same text under two different
datatypes is not equal. [Stop 2](../stops/2-values.md) shows the rules.

## Formal attributes are positions and other attributes are pairs

A statement has fixed positions for the things its kind always has, such
as the activity and entity of `used`. PROV-N writes each absent position
as `-`
([PROV-N section 2.4](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prov-n-optionals)),
so `used(ex:a, ex:e, -)` has three positions. Every other attribute is a
name-value pair in an optional bracketed list
([PROV-N section 2.5](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prov-n-standard-terms),
[PROV-DM section 5.7.2](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-attribute)).
[Stop 3](../stops/3-records.md) holds the positions as struct fields,
and [Stop 5](../stops/5-provn.md) reads and writes the bracketed list.

## Bundles are named sets inside a document

A bundle is a named set of records, so provenance can describe other
provenance ([PROV-DM section 5.4](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#component4),
[PROV-N section 3.4](https://www.w3.org/TR/2013/REC-prov-n-20130430/#component4)).
A prefix declared inside a bundle applies only there. [Stop 1](../stops/1-names.md)
resolves names in a bundle's scope, [Stop 4](../stops/4-json.md) reads the
`bundle` object, and [Stop 5](../stops/5-provn.md) reads and writes the
`bundle` statement.

## Two documents are equal as sets, and unification merges what shares an identifier

`==` on a `Document` ignores record order, attribute order, namespace
tables, blank-node labels and the spelling of a number, boolean or
`dateTime`. `Document::unified` merges records that share an identifier
and kind, and returns an error when their formal attributes conflict.
The rules come from [PROV-CONSTRAINTS section 2.4, Unification and Merging](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/#unification-and-merging).
The crate documentation in `provkit/src/lib.rs` summarises both, and
[Stop 3](../stops/3-records.md) reads the code.
