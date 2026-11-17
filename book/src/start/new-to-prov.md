# New to PROV

PROV is the W3C family of specifications for describing where things
came from. [PROV-DM](https://www.w3.org/TR/2013/REC-prov-dm-20130430/)
is the data model, [PROV-N](https://www.w3.org/TR/2013/REC-prov-n-20130430/)
is a notation for reading and writing it, and
[PROV-CONSTRAINTS](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/)
says when two descriptions count as the same. provkit also reads and
writes PROV-JSON, a [member submission](https://www.w3.org/Submission/2013/SUBM-prov-json-20130424/)
for exchanging documents between programs. The
[glossary](../reference/glossary.md) defines most of the terms below.

## A document is a set of records

[PROV-DM section 2](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#section-prov-overview)
introduces three kinds of thing. An entity is a thing, such as a file
or a report. An activity is something that happens over a period of
time and uses or produces entities. An agent is someone or something
responsible for an activity or an entity. Relations link them, such as
"this activity used that entity". A provkit `Document` holds the
records that describe them, together with a namespace table.
[Chapter 4](../chapters/4-records.md) describes the type. This PROV-N
document says that an edit used a draft and produced a report.

```text
{{#include ../../examples/edit.provn}}
```

Each line between `prefix` and `endDocument` is one record. The first
word is the record's kind. The arguments in brackets come in a fixed
order, and the first names what the record is about. A list in square
brackets at the end, such as `[prov:label="Report"]`, holds other
attributes. Chapter 4 calls entities,
activities and agents elements, and the other kinds relations.

## Every record is one of eighteen kinds

[PROV-DM section 5](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#data-model-components)
defines seventeen of the kinds in its six components, and [PROV-N section 3](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prov-n-expressions)
gives each of them a statement form. The
eighteenth, `mentionOf`, comes from
[PROV-Links section 2](https://www.w3.org/TR/2013/NOTE-prov-links-20130430/#term-mention).
The table gives each kind's arguments in order and leaves out a
relation's optional identifier. Chapter 4 names each position by its
role, such as `trigger` or `starter`, and uses that name as a struct
field.

| Written as | Says |
|---|---|
| `entity(e)` | `e` is a thing with some fixed aspects |
| `activity(a, t1, t2)` | `a` happens between start time `t1` and end time `t2` |
| `agent(ag)` | `ag` bears some responsibility for an activity or an entity |
| `wasGeneratedBy(e, a, t)` | activity `a` produced entity `e` at time `t` |
| `used(a, e, t)` | activity `a` began to use entity `e` at time `t` |
| `wasInformedBy(a2, a1)` | activity `a2` used an entity that activity `a1` produced |
| `wasStartedBy(a, e, a1, t)` | entity `e`, the trigger, started activity `a`, and activity `a1`, the starter, produced `e` |
| `wasEndedBy(a, e, a1, t)` | entity `e` ended activity `a`, and activity `a1`, the ender, produced `e` |
| `wasInvalidatedBy(e, a, t)` | activity `a` began the destruction or expiry of entity `e` |
| `wasDerivedFrom(e2, e1, a, g, u)` | entity `e2` came from entity `e1` through activity `a`, and `g` and `u` name the `wasGeneratedBy` and `used` records involved |
| `wasAttributedTo(e, ag)` | entity `e` is ascribed to agent `ag` |
| `wasAssociatedWith(a, ag, pl)` | agent `ag` had a role in activity `a`, following plan `pl` |
| `actedOnBehalfOf(ag2, ag1, a)` | agent `ag2`, the delegate, acted for agent `ag1`, the responsible, in activity `a` |
| `wasInfluencedBy(o2, o1)` | `o1`, the influencer, had an effect on `o2`, the influencee, each an entity, activity or agent |
| `specializationOf(e1, e2)` | `e1` shares every aspect of `e2` and presents more specific ones |
| `alternateOf(e1, e2)` | `e1` and `e2` present aspects of the same thing |
| `hadMember(c, e)` | entity `e` is a member of collection `c` |
| `prov:mentionOf(e1, e2, b)` | `e1` is a specialisation of `e2` that also presents bundle `b`, which describes `e2` |

A plan, a person and a collection are entities or
agents with a `prov:type` attribute, and a revision is a derivation
with one
([PROV-N section 3.2.2](https://www.w3.org/TR/2013/REC-prov-n-20130430/#expression-Revision)).
The `records!` table in
[Chapter 4](../chapters/4-records.md) states the same list once, and
the rest of the crate follows from it.

## Names are IRIs written with prefixes

An identifier is a qualified name, a prefix and a local part
([PROV-DM section 5.7.1](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-identifier)
and [section 5.7.5](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-qualified-name)).
A namespace declaration binds the prefix to an IRI, a web address that
may hold non-ASCII characters
([PROV-DM section 5.7.4](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-NamespaceDeclaration),
[PROV-N section 3.7.4](https://www.w3.org/TR/2013/REC-prov-n-20130430/#expression-NamespaceDeclaration)),
so `ex:e1` with `ex` bound to `http://example.org/` is the IRI
`http://example.org/e1`. Two names are equal when their IRIs are equal,
whatever prefix each was written with. [Chapter 2](../chapters/2-names.md)
describes the code.

Entities, activities and agents always have an identifier, and it is
their first argument. A relation may have one, written first and
followed by a semicolon, as in `wasGeneratedBy(ex:gen; ex:report, ex:edit, -)`
([PROV-N section 2.5](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prov-n-standard-terms)).
`specializationOf`, `alternateOf`, `hadMember` and `mentionOf` have no
identifier in PROV-DM or PROV-Links.

## Formal attributes are positions and other attributes are pairs

A statement has fixed positions for the things its kind always has, such
as the activity and entity of `used`. provkit calls them formal
attributes. The W3C texts list them for each kind but give them no
collective name. Each position holds a name or a time. Every other attribute is a name-value pair in an
optional bracketed list, such as `[prov:label="Report"]` in the example
([PROV-N section 2.5](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prov-n-standard-terms),
[PROV-DM section 5.7.2](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-attribute)).
[Chapter 4](../chapters/4-records.md) holds the positions as struct fields,
and [Chapter 3](../chapters/3-values.md) describes the set of pairs.

## A missing argument is an unknown value, with four exceptions

Some positions are optional, such as the time of `used`. PROV-N writes
an absent position as `-` when only some of the optional positions are
given, and leaves them all out when none is
([PROV-N section 2.4](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prov-n-optionals)),
so `used(ex:edit, ex:draft, -)` has three positions and no time.

[PROV-CONSTRAINTS section 2.4](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/#constants-variables-and-placeholders)
replaces most `-` markers with an existential variable, which "denotes
a fixed object that exists, but its exact identity is unknown". This
book calls such an absent value an existential. Any concrete value may
stand in its place, so two records that differ only where one has an
existential can merge into one.

[Section 5.1](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/#optional-identifiers-and-attributes)
keeps the `-` in four positions. They are the plan of
`wasAssociatedWith`, the activity of `wasDerivedFrom`, and the
generation and usage of a `wasDerivedFrom` whose activity is `-`. The
token is the same everywhere, and the position decides its meaning. A
missing plan may mean that there is no plan, and without an activity
"one cannot tell whether one or more activities are involved in the
derivation". So there the `-` stands for no value, which
[Chapter 4](../chapters/4-records.md) calls the constant "none". It
merges only with another `-`.

## Records that share an identifier describe one thing

[PROV-CONSTRAINTS section 6.1](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/#uniqueness-constraints)
makes the identifier a key, so the records of one kind with one
identifier describe one thing. Two records may each give part of it.

```text
activity(ex:a, 2020-01-01T00:00:00, -)
activity(ex:a, -, 2020-01-02T00:00:00)
```

`Document::unified` merges them into one activity with both times,
because the other record's time fills each existential. Two different
start times would conflict, and `unified` returns an error. Other
attributes are combined and never conflict.
[Section 6.4](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/#impossibility-constraints)
also forbids some kinds to share an identifier, such as an entity and
an activity. The rules come from
[PROV-CONSTRAINTS section 2.4, Unification and Merging](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/#unification-and-merging),
and [Chapter 4](../chapters/4-records.md) shows the code.

`==` on a `Document` ignores record order, attribute order, namespace
tables and the spelling of a number, boolean or `dateTime`
([Chapter 3](../chapters/3-values.md)).

## Bundles are named sets inside a document

A bundle is a named set of records, so provenance can describe other
provenance ([PROV-DM section 5.4](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#component4),
[PROV-N section 3.4](https://www.w3.org/TR/2013/REC-prov-n-20130430/#component4)).
A bundle is itself an entity, so other records can name it. A prefix
declared inside a bundle applies only there, as
[Chapter 2](../chapters/2-names.md) describes.

## Python `prov` and ProvToolbox are the libraries provkit is compared with

[Python `prov`](https://github.com/trungdong/prov) is a Python implementation
of PROV-DM, published on PyPI as `prov`. The book compares provkit
with Python `prov` 3.3.0.

[ProvToolbox](https://lucmoreau.github.io/ProvToolbox/) is a Java
library for PROV-DM. Its command-line tool is `provconvert`, and the
book compares provkit with version 2.5.0.

Documents move between these tools and provkit, so
[CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md)
records how provkit differs from them, and the chapters cite its rows.
