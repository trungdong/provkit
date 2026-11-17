# Not here on purpose

A reader meets each of these absences as a question at some chapter, such
as why there is no builder or why `-o` writes in place. Each one is a
decision, not an oversight. The sections below say what the code does
instead and, where one is set, give the release or the condition that
brings the missing part, and link the chapter where the question comes
up.

## Builders for records, `Document`, `Bundle` and `Value`

Today a record is a struct literal with public fields, as in
[One document, three views](start/three-views.md). A fluent builder is
arrives in 0.3.0.
The record types are read in [chapter 4](chapters/4-records.md).

## Feature gates for a third format

The library has two format features, `json` and `provn`. A `jsonld`
feature arrives with PROV-JSONLD in 0.2.0. The gate stays closed until
then because an empty feature is a promise the crate cannot yet keep.
The two existing gates are read in [chapter 1](chapters/1-orientation.md).

## Record-set difference and format dispatch in the library

`compare` lives in the CLI, not in the library. The library reads from
`&str` and returns a `Document`, so it never sees a path. Which format a
path holds is therefore the CLI's question. Both are read in
[chapter 7](chapters/7-cli.md).

## Lenient reading

Both readers refuse rather than guess. A mode that reads what it can and
reports the rest needs a report type for what it skipped. That type
arrives in 0.3.0, together with the structural diff. `provn::read_report`
is not that mode. It reads under a profile, fails as `provn::read`
does, and lists the extensibility expressions it left out of the
document because PROV defines no meaning for them. The readers are
read in
[chapter 5](chapters/5-json.md) and [chapter 6](chapters/6-provn.md).

## Source positions on records

Errors carry positions, but records do not, because nothing consumes
them yet. Positions on records arrive with validation in 0.4.0. Until
then `Error::Unification` names the record it concerns by its
identifier, and `Error::InRecord` by its section and key. The record
types are read in [chapter 4](chapters/4-records.md).

## An atomic `-o` in the CLI

`convert` serialises the whole document in memory and then writes the
output file in one call. A write to a temporary file followed by a
rename arrives when the CLI grows a second command that writes. The
command is read in [chapter 7](chapters/7-cli.md).

## A `prefixed` column in the records table

The records table has no `prefixed` column. PROV-Dictionary's records
need a prefixed keyword. The column arrives with that extension. The
table is read in [chapter 4](chapters/4-records.md).
