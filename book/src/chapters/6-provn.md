# Chapter 6: PROV-N

PROV-N is the notation of the W3C Recommendation of April 2013, a
text form with one statement per [record](../reference/glossary.md#record-provkit), `prefix` and `default`
declarations for the [namespace](../reference/glossary.md#namespace-and-prefix-prov) table and `bundle ... endBundle` for
the named [bundles](../reference/glossary.md#bundle-prov). Its grammar is the authority for this module, and
this chapter follows the grammar from characters to tokens, from tokens
to statements and from a [document](../reference/glossary.md#document-prov) back to text. It reads the two
[profiles](../reference/glossary.md#profile-provkit) the reader offers, the lexer, the recursive-descent parser,
the [extensibility expressions](../reference/glossary.md#extensibility-expression-prov) the parser accepts and leaves out, and
the writer. It builds on chapter 2 for the `PN_PREFIX` and `PN_LOCAL`
character classes the lexer and writer share and for the scope that
resolves each name, on chapter 3 for the values a [literal](../reference/glossary.md#literal-and-typed-literal-prov) becomes, and on
chapter 4 for the builder the parser drives and for `Record::from_parts`,
which checks every record the parser makes.

## What the code does

### provkit/src/provn/mod.rs

The file declares five private modules, `chars`, `extension`, `lexer`,
`parser` and `write`, re-exports the four writer functions, and
defines the reader's public types and entry points.

```rust,ignore
{{#include ../../../provkit/src/provn/mod.rs:profile}}
```

[`Profile`](../api/provkit/provn/enum.Profile.html) is `Copy`, so a
caller passes it by value, and `#[non_exhaustive]`, so a later profile
does not break a `match` outside the crate. `Strict` refuses every
form the grammar does not allow, each listed on the
[variant](../api/provkit/provn/enum.Profile.html#variant.Strict), four
of the six with a production or section, among them a prefixed
`prov:mentionOf` that is an extensibility expression rather than a
mention unless three names follow it. `Default` is the grammar plus the forms
in that list, which ProvPy and ProvToolbox write. Below the span,
`Relaxation` has one variant per item in the `Strict` list, and
`Profile::allows` answers `false` for `Strict` and `true` for
`Default` whatever the relaxation; a third profile would name the ones
it reads there. The parser asks `allows` with the
relaxation in hand wherever one applies, instead of comparing the
profile. Two more types sit
below. [`Ignored`](../api/provkit/provn/struct.Ignored.html) is an
extensibility expression the reader parsed and left out, with its name
as written and the line and column of the name, and
[`ReadReport`](../api/provkit/provn/struct.ReadReport.html) is a
document plus the list of those.

```rust,ignore
{{#include ../../../provkit/src/provn/mod.rs:read_report}}
```

[`read_report`](../api/provkit/provn/fn.read_report.html) is the one
function that does the work. `strip_bom` drops one leading byte-order
mark, as the PROV-JSON reader did in chapter 5, `lexer::tokenize` turns
the whole text into a `Vec<Token>` ending in `Eof`, and
`Parser::new` takes the text, the tokens and the profile. The parser
drives a `DocumentBuilder` from chapter 4 through its `document` method,
and the report is `builder.finish()` beside the parser's `ignored`
list. [`read_with`](../api/provkit/provn/fn.read_with.html), above the
span, calls `read_report` and keeps the document, and
[`read`](../api/provkit/provn/fn.read.html) calls `read_with` with
`Profile::Default`, so the three entry points agree by construction
and report errors alike. Text the grammar rejects is [`Error::Syntax`](../api/provkit/enum.Error.html)
with a line, a column and a message, and a name, literal or record
that parses but that PROV does not allow is `Error::At`, holding the
line and column and the error from the lower layer.

### provkit/src/provn/lexer/mod.rs

The lexer covers section 3.7 of the Recommendation and its terminal
productions [52] to [62]. One rule shapes the file. Keywords are not
distinguished here. Every bare or prefixed
name is a `Name` token, and the parser decides whether a name at the
start of a statement is a keyword. The one context-sensitive rule is
the language tag, which only follows a string.

```rust,ignore
{{#include ../../../provkit/src/provn/lexer/mod.rs:token_kind}}
```

Each variant is one terminal. A `Name` carries its [prefix](../reference/glossary.md#namespace-and-prefix-prov) and
local part with the escapes of the local part already decoded, and an
empty prefix for a bare name. `Str` is unescaped too. `Int` keeps its text
as written, because chapter 3's `Value` types an integer from its text,
and `DateTime` keeps its text for `DateTime::parse`. `Typed` is the `%%` that joins a string to
its datatype, `Marker` is the `-` that stands for an absent argument,
`QNameLiteral` is the quoted `'prefix:local'` form, `Iri` and `LangTag` carry their text without the angle brackets or the `@`, and `Eof` marks the end of input. The rest are single characters. Two methods below the span answer
questions the parser asks. `digit_name` returns the digits of an
unsigned `Int`, because a bare local name made only of digits lexes
as an integer, the lexer cannot tell the two apart, and the parser
asks in identifier positions.
`is_identifier_or_marker` says whether a token can fill an
`identifierOrMarker` position. Below those, `Token` is a kind, a
1-based line, a 1-based column in characters, and the byte span the
token was read from, which the parser uses to quote a name as written.
`tokenize` builds a `Lexer` over the text and loops, skipping trivia
and taking one token at a time, until it has pushed `Eof`.

```rust,ignore
{{#include ../../../provkit/src/provn/lexer/mod.rs:next_token}}
```

`next_token` records the line, the column and the byte offset before
it reads anything, so the token is placed where it starts. The column
comes from a method that counts only the characters since its last
call, so a long line is scanned once rather than once per token. When
no character is left the token is `Eof`. Otherwise one `match` on the
first character chooses the recogniser. The punctuation arms are
single characters that `emit` consumes. `<` starts an IRI, `"` a string, and
`'` a qualified-name literal, each read by a method named for it. The
two arms with guards are the lookahead the grammar needs. `@` starts a
language tag only when the previous token was a string, and `%%` is
two characters, so its arm
tests the rest of the input rather than `c`. Everything else goes to
`literal_or_name`, below the span, which tries a dateTime first, then
measures an integer and a name and takes the longer, so `2011` is an
integer but `2011a` is a name, and keeps a lone `-` as `Marker`. The
`after_string` flag is set from the token just made. Further down,
`skip_trivia` passes over the grammar's `WS`, `//` comments and `/* */`
comments, `decode_code_point` reads the hex digits of a `\u` or `\U`
escape and makes a `char` of them, and `scan_name` and `scan_local`
apply the `PN_PREFIX` and `PN_LOCAL` classes from chapter 2 with escape
decoding and error positions added.

### provkit/src/provn/parser/mod.rs

There is one method per production. Names resolve through
`Namespaces`, records are built by `Record::from_parts`, and every
event goes to a `DocumentBuilder`. A grammar error is `Error::Syntax`,
and an error from the name, value or record layer is wrapped by `at`,
above the span, which boxes the error into `Error::At` with the
token's line and column unless it already carries a position.

```rust,ignore
{{#include ../../../provkit/src/provn/parser/mod.rs:parser}}
```

The parser holds the source text, the token slice, the index of the
current token, the profile, the document's namespace table, the open
bundle's table when there is one, and the list of ignored expressions.
It borrows the text and the tokens for a lifetime `'t`, and the helper
methods above `document` return references tied to that lifetime, so a
`&'t Token` from `peek` or `bump` stays usable after the next call
that takes `&mut self`. `bump` returns the current token and advances
unless it is `Eof`, so no method runs off the end of the slice.
`scope` builds chapter 2's `Scope` from the two tables, the bundle's in
front of the document's, and `resolve` resolves a name in it, placing
any error at the name's token. `identifier` is where a `digit_name`
becomes a bare local name. Above the span, `NameParts` is a name token
split into its parts before resolution, `Positional` is a type alias,
with a lifetime parameter, for an argument paired with the token it
came from, and `LiteralParts` and `Suffix` are the tokens of a
literal before any name in it is resolved. Chapter 4's `Result` alias
had no lifetime; `Positional<'t>` shows one on an alias.

```rust,ignore
{{#include ../../../provkit/src/provn/parser/mod.rs:parse_document}}
```

`document` is production [1]. It expects the `document` keyword,
reads the declarations, opens the builder with a copy of the table and
loops until `endDocument`. Each turn is a bundle or a statement, and
once a bundle has been read a statement is refused under `Strict`,
because production [1] puts every expression before every bundle.
After `endDocument` only `Eof` may follow. `bundle`, below the span,
reads the identifier and the bundle's own declarations, installs that
table before resolving the identifier, because section 3.7.4 puts a
bundle's name in the bundle's own scope, and refuses `bundle` and
`endDocument` inside a bundle. Each statement goes through one method.

```rust,ignore
{{#include ../../../provkit/src/provn/parser/mod.rs:parse_statement}}
```

`statement` is the dispatch on the keyword. A bare name is a PROV
statement or a syntax error. A prefixed name starts an extensibility
expression, except `prov:mentionOf` when `mention_shape_follows`, which
looks ahead without consuming for `(`, three names separated by
commas, and `)` or `, [`. A `-` counts as a name under both profiles.
The identifier form is `Default`'s, and `Strict` takes it only when a name
is `-`, so that `formals` refuses the `-`. A PROV statement goes
through `expression`, below the span, and the record it returns goes
to the builder.
`expression` reads every PROV statement with one method, where the
grammar has one production per kind. `keyword_kind` turns the keyword into
a `Kind` through `classify`, which reads `mentionOf` bare only when
the profile allows it, `identifier_and_first_argument` reads an
element's mandatory identifier or a relation's first argument and the
`id;` form when `;` follows, `remaining_arguments` loops on `,` until
the [attribute](../reference/glossary.md#attribute-prov) block or `)`, `check_arity` compares the count with the
kind's arities, and `formals` matches each argument to a formal in table
order. It refuses `-` for a mandatory party of `hadMember`,
`alternateOf`, `specializationOf` or `mentionOf` under both profiles,
and `-` in any other required position under `Strict`.
`Record::from_parts` checks the record. The attribute block is read
by `attribute_block`, which takes a closure for the part of each pair
after its name, so the same loop serves both PROV statements and
extensibility expressions.

### provkit/src/provn/parser/declarations.rs

This file is a second `impl Parser<'t>` block, as chapter 4 showed for
`Document`, holding the declaration productions of section 3.7.4.

```rust,ignore
{{#include ../../../provkit/src/provn/parser/declarations.rs:namespace_declarations}}
```

`namespace_declarations` is productions [45] to [48]. It loops over
`default` and `prefix` lines and returns the table at the first other
token. A second `default` in one scope is refused under both profiles.
A `default` after a `prefix` is refused under `Strict`, because
production [45] admits a `defaultNamespaceDeclaration` only first. A
prefix declared twice in one scope is refused under both. `prov` or
`xsd` declared at its own IRI is refused under `Strict`, because
section 3.7.4 says a namespace declaration MUST NOT declare them, and
`Namespaces::declare` refuses either prefix at any other IRI under
both profiles. `seen` is a `HashSet<&str>` borrowing from the tokens,
as chapter 4's sets borrowed their keys. `prefix_name`, below the span,
tests the prefix's text as written with `is_pn_prefix` from chapter 2,
because production [46] takes a `PN_PREFIX`, which has no escapes.
An empty or all-whitespace IRI, as in `prefix z <>`, `default <>` or
`prefix z <\u00A0>`, is refused under both profiles at the IRI token,
although `IRI_REF` admits it.

### provkit/src/provn/extension.rs

The file handles section 5 of the Recommendation. Each extensibility
expression is parsed in full and
left out of the document. Names are not resolved, literals are not
built, a time is checked, and the outermost expression is recorded as
an `Ignored`. The file is a third `impl` block for `Parser`.

```rust,ignore
{{#include ../../../provkit/src/provn/extension.rs:extensibility_expression}}
```

`extensibility_expression` consumes the name, parses the body at
depth zero and then pushes an `Ignored` with the name's text as
written, taken from the token's byte span, and its line and column.
`extensibility_body`, below the span, is production [49]. It reads
`(`, an optional `id;` or `-;`, one argument, then more arguments on
`,` until the attribute block or `)`. The attribute block goes through
the same `attribute_block` as a PROV statement, with a closure that
reads `=` and the tokens of a literal and drops them.
`extensibility_argument` is production [50]. A prefixed name followed
by `(` is a nested expression, `{` or `(` is a tuple under production
[51], a name or `-` is consumed, a dateTime is parsed because
production [44] defines its form, and a literal's tokens are consumed
and dropped. The nested cases call `enter` first.

```rust,ignore
{{#include ../../../provkit/src/provn/extension.rs:enter}}
```

`enter` is the whole depth check. It is called with the current depth
before each recursive call, and refuses once `depth` has reached
`MAX_DEPTH`, placing the error at the opening token, so an expression
may hold up to 64 levels of expressions and tuples below the outermost
one.

### provkit/src/provn/write/mod.rs

[`write`](../api/provkit/provn/fn.write.html) and
[`to_string`](../api/provkit/provn/fn.to_string.html) are the
document-level pair, and
[`statement`](../api/provkit/provn/fn.statement.html) and
[`name`](../api/provkit/provn/fn.name.html) write one record or one
name against a table the caller holds.

```rust,ignore
{{#include ../../../provkit/src/provn/write/mod.rs:provn_to_string}}
```

A namespace with no declared
prefix gets a minted `ns1`, `ns2` and so on, and the document is left
unchanged, because the function works on a clone of the table. Bundles that
share an identifier are written as one, through `merged_bundles` from
chapter 4. The body is rendered before the declarations are printed,
because every prefix minted while rendering a name lands in `names`,
and `declarations` then sees it. The block around the top-level
loop keeps `top` to that loop. The mutable borrow that
`WriteScope::of` takes on `names` ends with the last use of `top`,
so the bundle loop and `declarations` can borrow it again.
`write_bundle`, below the span, does the same for a bundle against its
own table over the document's, renders the bundle's identifier inside
the same block after the records, so a prefix minted for it is in
`own` when the declarations print, and prints only the declarations
the document does not already provide. `declarations` prints a
`default` line and a `prefix` line per entry, refusing an IRI that
holds a character `IRI_REF` forbids. `name_in` is where a prefix is
chosen. A declared prefix wins, then a bare name under the default
namespace, then a minted one, and `lexes_as_bare_name` runs the lexer
on the candidate bare text and takes a prefix instead unless the text
reads back as exactly one bare name with the same local part.

```rust,ignore
{{#include ../../../provkit/src/provn/write/mod.rs:expression}}
```

`expression` writes one record. `check_attributes` refuses an
attribute that spells one of the record's own formals, and
`check_parties` refuses a relation without a mandatory party, because
the reader would refuse both. An element writes its identifier, then all
of its formals if it has any, so an [activity](../reference/glossary.md#activity-prov) writes both times or
neither. A relation writes `id;` only when it has an identifier, and
then every formal in table order, with `formal` giving `-` for an
absent one. The attribute block follows when there are attributes,
each pair as `name=value`. `value`, at the end of the file, writes a
string with `"`, `\`, newline, return and tab escaped, a tagged string
with `@`, a name in single quotes, a dateTime as a string typed
`xsd:dateTime`, an `xsd:int` whose text is an `INT_LITERAL` and reads
back as `xsd:int` unquoted, and every other typed value as a string with `%%` and its datatype.

## Why this shape

### Why two profiles, and why is `Default` the default?

`Strict` is the reference for the grammar and the MUST statements of
the prose, and `Default` adds what ProvToolbox and ProvPy write, the
forms the `Relaxation` enum names, except the two grammar-invalid
forms ProvToolbox 2.2.5 writes and its `provext:` statements. The
`strict` profile of ProvPy 3.3.0 refuses four of those forms too, and
productions [1], [24] to [26] and [45] and the MUST in section 3.7.4
are the grammar behind each refusal
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"`Profile::Strict` accepts the whole PROV-N grammar"). The parser
keeps the difference to `allows` calls, one relaxation per
`Relaxation` variant. `read` uses `Default`, the grammar plus what the
two libraries write, and the test `the_default_profile_is_default`
pins `Profile::default()` to it.

### Why does the writer write every argument?

Every relation is written with `-` for an absent argument, except a
mandatory party of `hadMember`, `alternateOf`, `specializationOf` or
`mentionOf`, which the writer refuses. `activity` writes both times or
neither, and an identifier on `alternateOf`, `specializationOf`,
`hadMember` or `mentionOf` is written as `id;` before the arguments
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"The PROV-N writer writes every relation at full arity"). One rule
serves every relation that has its mandatory parties, and a document with missing data still
round-trips. The cost is that `Strict` rejects that output when a
required argument is `-` or a bare kind carries an identifier, while
`Default` accepts it, so a kept identifier reads back equal under
`Profile::Default`.

### Why is `mentionOf` written bare?

ProvToolbox's PROV-N grammar has this form
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"The PROV-N writer writes `mentionOf` bare"). `Strict` accepts only
`prov:mentionOf`, so the writer's output reads back under `Default`
and not under `Strict`, which `classify` enforces by taking the bare
keyword only when `Relaxation::BareMentionOf` is allowed.

### Why parse extensibility expressions and then drop them?

Section 5 of
[PROV-N](https://www.w3.org/TR/2013/REC-prov-n-20130430/#extensibility)
says "PROV-N compliant parsers MUST be able to parse expressions
matching the extensibilityExpression production" and "PROV compliant
implementations MAY ignore these expressions". The reader does both,
parsing them under either profile and leaving them out of the document
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"Extensibility expressions are parsed under both profiles and left
out"). ProvPy 3.3.0 does the same under each of its profiles and
warns about each expression it drops. Section 3.7.1 requires a prefix
to refer to a declared namespace, but the reader ignores the whole
expression, contents included, and reports it, so `ex:foo(zz:a)` reads with `zz`
undeclared, while a time is checked because production [44] defines
its form
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"Names inside an extensibility expression are not resolved"). The
depth limit bounds the recursion on hostile input, and the check runs
before recursing, so a deeply nested input fails without exhausting
the stack
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"Extensibility expressions nest to 64 levels").

### Why do `\u` escapes decode once?

The decoded content is not lexed again, so `"\u0022"` is the
one-character string `"`, and `"a\u000Ab"` holds a line feed in a
short string. Section 6 of
[PROV-N](https://www.w3.org/TR/2013/REC-prov-n-20130430/#media-type)
lets a code point be written as a `\u` or `\U` escape and does not say
at which stage it decodes. The lexer decodes it inside a string or an
IRI, because decoding before lexing would let `\u0022` end a string
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"A `\u` or `\U` escape in a string or IRI decodes"). In the code,
`string` and
`iri` decode an escape as they scan and push the character into the
token's content, so no second pass sees it. An escape that names a
surrogate is a syntax error, although section 6 lets `\u` cover U+0
to U+FFFF, because a string of Unicode scalar values cannot hold one
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"A `\u` escape that names a surrogate"). `decode_code_point` gets that
from `char::from_u32`, which returns `None` for a surrogate.

### Why does every error carry a line and a column?

Every error from the PROV-N reader carries a line and a column,
because the lexer stamps every token with its position and the parser
knows which token it is on. `Error::Syntax` carries the position of
text the grammar rejects, and `Error::At` wraps the error for a name,
literal or record, with the column counted in characters
([CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md),
"Every error from the PROV-N reader carries a line and a column").
`at` copies the line and column from the token onto any error from a
lower layer, so both variants carry a position a caller can print.

### Why does the lexer not know keywords?

The grammar is what makes it possible. Production [53] `PN_LOCAL` in section 3.7.1 of
[PROV-N](https://www.w3.org/TR/2013/REC-prov-n-20130430/#prod-PN_LOCAL)
admits the characters of `entity`, so that text is a `QUALIFIED_NAME`
under production [52] wherever one may stand, in an attribute name or
as a bare identifier under the default namespace. The lexer makes a
`Name` of it, and `is_keyword` and `classify` in the parser test the
token where a keyword may stand, at the start of a statement or a
declaration. `digit_name` follows the same pattern. The lexer cannot
tell a local name made of digits from an `INT_LITERAL`, so the parser
asks in identifier positions. Section
3.7.1 also notes that `QUALIFIED_NAME` and `PN_PREFIX` conflict for a
tokeniser and that in a `namespaceDeclaration` a tokeniser should
prefer `PN_PREFIX`, which `prefix_name` does by testing the written
text with `is_pn_prefix`.

## The Rust here

- **`#[default]` on an enum variant**. `#[derive(Default)]` on an enum
  needs one variant marked `#[default]`, and `Profile::default()`
  returns it.
  [Derive](https://doc.rust-lang.org/reference/attributes/derive.html),
  [Default for enums](https://doc.rust-lang.org/std/default/trait.Default.html#enums)
- **References tied to the input, not to `&self`**. Chapter 2 gave a struct
  a lifetime; here `Lexer::rest(&self) -> &'a str` returns a slice with the
  input's lifetime `'a` rather than the borrow of `self`, so the slice
  stays usable across `self.advance()`, and `Parser::peek` does the same
  with `&'t Token`.
  [Lifetime syntax](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)
- **Recursive descent, one method per production**. Each method of
  `Parser`, from `document` down, consumes the tokens of its production, calls the methods for the productions
  inside it, and returns a value or an error; chapter 2's `?` ends the
  whole parse at the first error.
  [Recoverable errors with `?`](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html)
- **`match` guards**. In `next_token`, `'@' if self.after_string` takes
  the arm only when the condition holds, and `_ if rest.starts_with("%%")` tests something
  other than the matched value.
  [Extra conditionals with match guards](https://doc.rust-lang.org/book/ch19-03-pattern-syntax.html#extra-conditionals-with-match-guards)
- **A closure taking `&mut Self`**. `attribute_block` holds `&mut
  self`, so a closure cannot also capture the parser; instead the parser is passed
  to the closure as a parameter on every call, and `impl FnMut` lets the
  closure mutate what it captured, such as `attributes`.
  [Closures](https://doc.rust-lang.org/book/ch13-01-closures.html)
- **A `const` depth limit on recursion**. Each nested call passes `depth
  + 1`, and `enter` refuses before the call that would exceed
  `MAX_DEPTH`, so the stack depth is bounded by a constant.
  [Constant items](https://doc.rust-lang.org/reference/items/constant-items.html)
- **`.map(drop)`**. `literal_parts` returns a `Result` holding tokens
  `extensibility_body` does not want; `map(drop)` turns the `Ok` payload into `()`
  and keeps the `Err`.
  [drop](https://doc.rust-lang.org/std/mem/fn.drop.html)
- **Let chains**. `if let Some(default) = ... && inherited... !=
  Some(default)` in `declarations` binds and tests in one condition,
  which edition 2024 allows.
  [Chains of conditions](https://doc.rust-lang.org/reference/expressions/if-expr.html#chains-of-conditions)
- **`let _ = writeln!(..)`**. Writing into a `String` cannot fail, and
  `let _` in `declarations` discards the `Result` that the `must_use` on it would
  otherwise warn about.
  [writeln!](https://doc.rust-lang.org/std/macro.writeln.html)
- **`Result::as_deref` with a slice pattern**. `as_deref` turns
  `&Result<Vec<Token>, Error>` into `Result<&[Token], &Error>`, and the
  pattern `Ok([first, second])` matches a slice of exactly two elements,
  so one `matches!` in `lexes_as_bare_name` tests the count and both
  kinds.
  [Result::as_deref](https://doc.rust-lang.org/std/result/enum.Result.html#method.as_deref),
  [Slice patterns](https://doc.rust-lang.org/reference/patterns.html#slice-patterns)
- **A scoped block around a borrow**. In `to_string`, the block keeps
  `top`, which holds `&mut names`, to the top-level loop. The borrow
  ends at the last use of `top`, with or without the block, after which
  `&names` can be lent to `write_bundle` and `declarations`.
  [References and borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html)

## Run this

```sh
cargo test -p provkit strict_is_the_whole_grammar
```

This reads one document per case, a `default` after a `prefix` at
document level and inside a bundle, a statement after a bundle, an
identifier or attributes on `hadMember`, `alternateOf` and
`specializationOf`, and `prov` and `xsd` declared at their own IRIs,
and checks that each
reads under `Default` and fails under `Strict` at the expected line
and column. The unit-test
binary reports the one test; the other test binaries report zero.

```text
test result: ok. 1 passed
```

```sh
cargo test -p provkit example_46_reads_in_both_forms
```

This reads the two `dictExt:hadMembers` forms of example 46 in the
Recommendation under both profiles, and checks that the document holds
the one [entity](../reference/glossary.md#entity-prov) beside them and that the report names
`dictExt:hadMembers` at its line and column.

```text
test result: ok. 1 passed
```

```sh
cargo test -p provkit prints_the_expected_text
```

This builds a document by hand with a default namespace, an entity
with a name, an integer, a string with escapes, a tagged string, a
dateTime and a double among its attributes, an activity with a start
time and no end time, a
generation with and one without an identifier, a mention, an entity
with a dot in its local part and a bundle, and compares the writer's
output with the expected text character for character.

```text
test result: ok. 1 passed
```

## Try it

`corpus/provn/probes/expected.tsv` says the probe `alt_with_attrs`
is rejected under `Strict` and accepted under `Default`, by rule
[24], and its `provpy` column is empty, because ProvPy 3.3.0 gives the
same results. Read `corpus/provn/probes/alt_with_attrs.provn` and
predict which token `Strict` refuses and where its message points.
Then run the doctest below, which the book compiles with the probe's
text pasted in.

```rust
# extern crate provkit;
use provkit::provn::{self, Profile};

let text = r#"document
prefix ex <http://example.org/>
alternateOf(ex:e2, ex:e1, [ex:k="x"])
endDocument
"#;
let err = provn::read_with(text, Profile::Strict).unwrap_err();
assert_eq!(
    err.to_string(),
    "PROV-N syntax error at line 3, column 27: \
     expected no attributes in alternateOf under the strict profile, found '['"
);
assert!(provn::read_with(text, Profile::Default).is_ok());
```

<details><summary>Answer</summary>

Production [24] is `alternateOf "(" eIdentifier "," eIdentifier ")"`,
with no attribute block, so under `Strict` the `[` after the second
identifier is the first token outside the grammar.
`remaining_arguments` sees the `,` then the `[`, and because
`takes_no_extras` is true for `alternateOf` under `Strict` it returns
the error placed at the `[`, on line 3 at column 27.

```text
PROV-N syntax error at line 3, column 27: expected no attributes in alternateOf under the strict profile, found '['
```

ProvPy writes an attribute block on `alternateOf`, so `Default` takes
it, and the row beginning "`Profile::Strict` accepts the whole PROV-N grammar"
lists this probe among its tests.

</details>

## Idioms first seen here

- `#[default] on an enum variant`: https://doc.rust-lang.org/reference/attributes/derive.html
- `references tied to the input, not to &self`: https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
- `recursive descent, one method per production`: https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html
- `match guards`: https://doc.rust-lang.org/book/ch19-03-pattern-syntax.html#extra-conditionals-with-match-guards
- `a closure taking &mut Self`: https://doc.rust-lang.org/book/ch13-01-closures.html
- `a const depth limit on recursion`: https://doc.rust-lang.org/reference/items/constant-items.html
- `.map(drop)`: https://doc.rust-lang.org/std/mem/fn.drop.html
- `let chains`: https://doc.rust-lang.org/reference/expressions/if-expr.html#chains-of-conditions
- `let _ = writeln!(..)`: https://doc.rust-lang.org/std/macro.writeln.html
- `Result::as_deref with a slice pattern`: https://doc.rust-lang.org/std/result/enum.Result.html#method.as_deref
- `a scoped block around a borrow`: https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html
