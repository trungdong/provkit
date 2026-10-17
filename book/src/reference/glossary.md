# Glossary

PROV and Rust share several words. Each entry here says which sense the book means, and the heading carries the tag `(PROV)` or `(Rust)`. A word with a sense in both gets two adjacent headings that link to each other. Entries run in alphabetical order of the term.

PROV-DM is the [data model](https://www.w3.org/TR/2013/REC-prov-dm-20130430/), PROV-N the [notation](https://www.w3.org/TR/2013/REC-prov-n-20130430/), and PROV-JSON the [JSON serialisation](https://www.w3.org/Submission/2013/SUBM-prov-json-20130424/).

### activity (PROV)

Something that occurs over a period of time and acts on or with entities ([PROV-DM section 5.1.2](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-Activity)).

### agent (PROV)

Something that bears responsibility for an activity or an entity ([PROV-DM section 5.3.1](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-agent)).

### alternate (PROV)

Two entities that present aspects of the same thing, linked by `alternateOf` ([PROV-DM section 5.5.2](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-alternate)).

### Arc and Rc (Rust)

Reference-counted pointers that let several owners share one value. `Rc` stays on one thread and `Arc` crosses threads ([`Rc<T>`](https://doc.rust-lang.org/book/ch15-04-rc.html) in the Rust book, [`Arc`](https://doc.rust-lang.org/std/sync/struct.Arc.html) in std).

### attribute (PROV)

A name-value pair attached to a record, such as `prov:type="createFile"`. The name is a qualified name ([PROV-DM section 5.7.2](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-attribute)). The Rust sense is [attribute (Rust)](#attribute-rust).

### attribute (Rust)

A `#[...]` or `#![...]` annotation on an item, such as `#[derive(Debug)]` or `#[cfg(test)]` ([the Rust reference chapter on attributes](https://doc.rust-lang.org/reference/attributes.html)). The PROV sense is [attribute (PROV)](#attribute-prov).

### attribution (PROV)

The relation `wasAttributedTo`, which ties an entity to the agent responsible for it ([PROV-DM section 5.3.2](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-attribution)).

### blank node (PROV)

A generated identifier of the form `_:id` that PROV-JSON uses as the map key for a relation that has no identifier of its own ([PROV-JSON section 2.1](https://www.w3.org/Submission/2013/SUBM-prov-json-20130424/#blank-node-ids)).

### borrow and reference (Rust)

A reference (`&T` or `&mut T`) lends access to a value without taking ownership, and borrowing is the act of creating one ([the Rust book chapter on references and borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html)).

### Box (Rust)

A smart pointer that owns a value on the heap ([the Rust book chapter on `Box<T>`](https://doc.rust-lang.org/book/ch15-01-box.html)).

### bundle (PROV)

A named set of provenance descriptions, which is itself an entity and so can carry provenance ([PROV-DM section 5.4](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-bundle)). Rust has no sense of this word, as [bundle (Rust)](#bundle-rust) explains.

### bundle (Rust)

Rust has no such term. The nearest Cargo words are crate and package, which group code and not provenance ([crate and package (Rust)](#crate-and-package-rust), [the Rust book chapter on packages and crates](https://doc.rust-lang.org/book/ch07-01-packages-and-crates.html)). The PROV sense is [bundle (PROV)](#bundle-prov).

### collection and membership (PROV)

A collection is an entity that has members, and `hadMember` links the collection to each member ([PROV-DM section 5.6](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#component6)).

### communication (PROV)

The relation `wasInformedBy`, which says one activity used something another activity generated ([PROV-DM section 5.1.5](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-Communication)).

### Cow (Rust)

A clone-on-write type that holds either a borrowed value or an owned one and clones only when a caller needs to change it. The std library documents [`Cow`](https://doc.rust-lang.org/std/borrow/enum.Cow.html).

### crate and package (Rust)

A crate is one compilation unit, a library or a binary. A package is one `Cargo.toml` that builds one or more crates ([the Rust book chapter on packages and crates](https://doc.rust-lang.org/book/ch07-01-packages-and-crates.html)).

### delegation (PROV)

The relation `actedOnBehalfOf`, which says one agent acted for another ([PROV-DM section 5.3.4](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-delegation)).

### derivation (PROV)

The relation `wasDerivedFrom`, which says one entity was transformed from, created from, or influenced by another ([PROV-DM section 5.2.1](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-Derivation)). The Rust word with the same stem is [derive (Rust)](#derive-rust).

### derive (PROV)

To establish that one entity came from another, which is what a `wasDerivedFrom` record states ([PROV-DM section 5.2.1](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-Derivation)). The Rust sense is [derive (Rust)](#derive-rust).

### derive (Rust)

The `#[derive(...)]` attribute, which asks the compiler to generate a standard trait implementation for a type ([Appendix C of the Rust book](https://doc.rust-lang.org/book/appendix-03-derivable-traits.html)). The PROV sense is [derive (PROV)](#derive-prov).

### doctest (Rust)

A code example in a doc comment that `cargo test` compiles and runs ([the rustdoc book](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html)).

### document (PROV)

The top-level PROV-N construct, `document ... endDocument`, which holds namespace declarations, records and bundles ([PROV-N section 4](https://www.w3.org/TR/2013/REC-prov-n-20130430/#document)).

### edition (Rust)

A named set of language rules, such as 2021, that a crate opts into with `edition` in `Cargo.toml` ([the Rust edition guide](https://doc.rust-lang.org/edition-guide/)). Crates on different editions link together.

### end and start (PROV)

`wasEndedBy` and `wasStartedBy` relate an activity to the entity that triggered its end or start ([PROV-DM section 5.1.6](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-Start) for start, [section 5.1.7](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-End) for end).

### entity (PROV)

A physical, digital or conceptual thing that provenance describes ([PROV-DM section 5.1.1](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-entity)).

### enum and variant (Rust)

An enum is a type that is exactly one of several named variants, and each variant can carry data ([the Rust book chapter on defining an enum](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html)).

### extensibility expression (PROV)

A PROV-N statement whose name is not one of the standard relations, so that applications can add their own ([PROV-N section 5](https://www.w3.org/TR/2013/REC-prov-n-20130430/#extensibility)).

### feature (Rust)

A named, optional part of a crate that a user turns on in `Cargo.toml` or on the command line ([the Cargo book chapter on features](https://doc.rust-lang.org/cargo/reference/features.html)).

### formal attribute (PROV)

One of the positional arguments of a PROV-N statement, such as the entity and activity in `used(a, e, t)`, as opposed to the optional attribute list in square brackets ([PROV-N section 2.5](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prov-n-standard-terms) for the layout, [section 3.1](https://www.w3.org/TR/2013/REC-prov-n-20130430/#component1) for each statement's arguments).

### generate (PROV)

To bring an entity into existence through an activity, which is what a `wasGeneratedBy` record states ([PROV-DM section 5.1.3](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-Generation)). The Rust sense is [generate (Rust)](#generate-rust).

### generate (Rust)

What a macro does at compile time, producing code from the tokens it receives ([the Rust book chapter on macros](https://doc.rust-lang.org/book/ch20-05-macros.html)). The PROV sense is [generate (PROV)](#generate-prov).

### generation (PROV)

The relation `wasGeneratedBy`, which says an activity produced an entity ([PROV-DM section 5.1.3](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-Generation)). The Rust word with the same stem is [generate (Rust)](#generate-rust).

### generic and bound (Rust)

A generic item takes type parameters, and a bound such as `T: Display` limits which types can fill one ([the Rust book chapter on generic data types](https://doc.rust-lang.org/book/ch10-01-syntax.html)).

### identifier and qualified name (PROV)

An identifier is a qualified name, a prefix and a local part, that names a record ([PROV-DM section 5.7.1](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-identifier) and [section 5.7.5](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-qualified-name), with the grammar in [PROV-N section 3.7.1](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prod-QUALIFIED_NAME)).

### influence (PROV)

The most general relation, `wasInfluencedBy`, of which attribution, derivation and the others are special cases ([PROV-DM section 5.3.5](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-influence)).

### invalidation (PROV)

The relation `wasInvalidatedBy`, which says an activity ended the existence of an entity ([PROV-DM section 5.1.8](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-Invalidation)).

### lifetime (Rust)

The span of code during which a reference stays valid, written `'a` when a signature names it ([the Rust book chapter on lifetimes](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)).

### literal and typed literal (PROV)

A literal is a value such as `"abc"`. A typed literal pairs the value with a datatype, as in `"2013-01-01" %% xsd:date` ([PROV-DM section 5.7.3](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-value), [PROV-N section 3.7.3](https://www.w3.org/TR/2013/REC-prov-n-20130430/#expression-literal)).

### macro (Rust)

Code that writes code. Declarative macros (`macro_rules!`) match patterns, and procedural macros run Rust functions on token streams ([the Rust book chapter on macros](https://doc.rust-lang.org/book/ch20-05-macros.html)).

### mention (PROV)

The relation `prov:mentionOf`, which says one entity is a view of another entity inside a named bundle ([PROV-Links section 2](https://www.w3.org/TR/2013/NOTE-prov-links-20130430/#term-mention)). It parses as an extensibility expression in PROV-N.

### module (Rust)

A named scope inside a crate that groups items and controls which are public ([the Rust book chapter on modules](https://doc.rust-lang.org/book/ch07-02-defining-modules-to-control-scope-and-privacy.html)).

### namespace and prefix (PROV)

A namespace is an IRI, and a prefix is the short name that a document binds to it so that `prefix:local` expands to an identifier ([PROV-DM section 5.7.4](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-NamespaceDeclaration), [PROV-N section 3.7.4](https://www.w3.org/TR/2013/REC-prov-n-20130430/#expression-NamespaceDeclaration)).

### Option and Result (Rust)

`Option<T>` is either `Some(value)` or `None`. `Result<T, E>` is either `Ok(value)` or `Err(error)` ([the Rust book chapter on enums](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html) for `Option`, [the chapter on recoverable errors](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html) for `Result`).

### ownership and move (Rust)

Each value has one owner, and assigning or passing it to another owner moves it, so the old name stops being usable ([the Rust book chapter on ownership](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html)).

### profile (PROV)

A provkit setting that decides how strictly a reader treats input. `Strict` reads the whole grammar and the MUST statements of its prose, and `Default` adds the extensions ProvPy and ProvToolbox write. [Chapter 6](../chapters/6-provn.md) shows the code.

### PROV-CONSTRAINTS (PROV)

The W3C Recommendation that defines when a document is valid and how its descriptions unify ([PROV-CONSTRAINTS](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/)).

### PROV-JSON (PROV)

The 2013 W3C member submission that serialises PROV as JSON ([PROV-JSON](https://www.w3.org/Submission/2013/SUBM-prov-json-20130424/)).

### PROV-N (PROV)

The text notation for PROV, with one functional-style statement per record ([PROV-N](https://www.w3.org/TR/2013/REC-prov-n-20130430/)).

### record (PROV)

provkit's word for one PROV-DM description, such as an entity, an activity or a relation. [Chapter 4](../chapters/4-records.md) shows the type.

### rustdoc and clippy (Rust)

`rustdoc` builds HTML documentation from doc comments, and `clippy` is the linter that flags common mistakes ([the rustdoc book](https://doc.rust-lang.org/rustdoc/), [the Clippy book](https://doc.rust-lang.org/clippy/)).

### specialization (PROV)

The relation `specializationOf`, which says one entity is a more specific view of another ([PROV-DM section 5.5.1](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-specialization)).

### trait (Rust)

A set of methods that a type can implement, which lets code work with any type that has them ([the Rust book chapter on traits](https://doc.rust-lang.org/book/ch10-02-traits.html)).

### type (PROV)

The `prov:type` attribute, which says what kind of thing a record describes, for example `prov:Person` on an agent ([PROV-DM section 5.7.2.4](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-attribute-type)). The Rust sense is [type (Rust)](#type-rust).

### type (Rust)

The set of values a name can hold and the operations on them, such as `u8`, `String` or a struct you define ([the Rust book chapter on data types](https://doc.rust-lang.org/book/ch03-02-data-types.html)). The PROV sense is [type (PROV)](#type-prov).

### unification (PROV)

Merging two descriptions that share an identifier into one, provided their attributes do not conflict ([PROV-CONSTRAINTS section 2.4, Unification and Merging](https://www.w3.org/TR/2013/REC-prov-constraints-20130430/#unification-and-merging)).

### usage (PROV)

The relation `used`, which says an activity consumed an entity ([PROV-DM section 5.1.4](https://www.w3.org/TR/2013/REC-prov-dm-20130430/#term-Usage)).

### workspace (Rust)

A set of packages that share one `Cargo.lock` and one output directory ([the Cargo book chapter on workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)).
