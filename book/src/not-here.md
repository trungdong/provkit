# Not here on purpose

A reader meets each of these absences as a question at some chapter, such
as why there is no public builder or why `-o` writes in place. Each one is a
decision. The sections below say what the code does instead and link the
chapter where the question comes up.

## Builders for records, `Document`, `Bundle` and `Value`

Today a record is a struct literal with public fields, as in
[One document, three views](start/three-views.md). The library has no
public fluent builder. The `DocumentBuilder` that both readers
drive is crate-private. It and the record types are described in
[Chapter 4](chapters/4-records.md).

## Feature gates for a third format

The library has two format features, `json` and `provn`. It has no
`jsonld` feature, because it does not read or write PROV-JSONLD and an
empty feature is a promise the crate cannot keep.
The two existing gates are described in [Chapter 1](chapters/1-orientation.md).

## Record-set difference and format dispatch in the library

`compare` lives in the CLI, not in the library. The library reads from
`&str` and returns a `Document`, so it never sees a path. Which format a
path holds is therefore the CLI's question. Both are described in
[Chapter 7](chapters/7-cli.md).

## Lenient reading

Both readers refuse rather than guess. The library has no mode that reads
what it can and reports the rest. `provn::read_report` is not that mode. It
reads under a profile, fails as `provn::read` does, and lists the extensibility expressions it left out of the
document because PROV defines no meaning for them. The readers are
described in
[Chapter 5](chapters/5-json.md) and [Chapter 6](chapters/6-provn.md).

## Source positions on records

Errors carry positions, but records do not, because nothing consumes
them. `Error::Unification` names the record it concerns by its
identifier, and `Error::InRecord` by its section and key. The record
types are described in [Chapter 4](chapters/4-records.md).

## An atomic `-o` in the CLI

`convert` serialises the whole document in memory and then writes the
output file in one call. It does not write to a temporary file and
rename it. The command is described in [Chapter 7](chapters/7-cli.md).

## A `prefixed` column in the records table

The records table has no `prefixed` column. PROV-Dictionary's records
need a prefixed keyword, and the library does not implement that
extension. The table is described in [Chapter 4](chapters/4-records.md).
