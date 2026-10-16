# Not here on purpose

A reader meets each of these absences as a question at some stop, such
as why there is no builder or why `-o` writes in place. Each one is a
decision with a release attached, not an oversight. The sections below
give the reason and the release for each, and link the stop where the
question comes up.

## Builders for records, `Document`, `Bundle` and `Value`

Today a record is a struct literal with public fields, as in
[One document, three views](start/three-views.md). A fluent builder is
planned for 0.3.0. A builder has to check the fields that validation
requires, so it waits until validation fixes which fields those are.
The record types are read in [stop 3](stops/3-records.md).

## Feature gates for a third format

The library has two format features, `json` and `provn`. A `jsonld`
feature arrives with PROV-JSONLD in 0.2.0. The gate stays closed until
then because an empty feature is a promise the crate cannot yet keep.
The two existing gates are read in [stop 0](stops/0-orientation.md).

## Record-set difference and format dispatch in the library

`compare` lives in the CLI, not in the library. The library reads from
`&str` and returns a `Document`, so it never sees a path. Which format a
path holds is therefore the CLI's question. Both are read in
[stop 6](stops/6-cli.md).

## Lenient reading

Both readers refuse rather than guess. A mode that reads what it can and
reports the rest needs a report type for what it skipped. That type is
planned for 0.3.0, together with the structural diff. The readers are
read in [stop 4](stops/4-json.md) and [stop 5](stops/5-provn.md).

## Source positions on records

Errors carry positions, but records do not, because nothing consumes
them yet. Positions on records arrive with validation in 0.4.0. The record types are read in [stop 3](stops/3-records.md).

## An atomic `-o` in the CLI

`convert` serialises the whole document in memory and then writes the
output file in one call. A write to a temporary file followed by a
rename arrives when the CLI grows a second command that writes. The command is read in [stop 6](stops/6-cli.md).

## A `prefixed` column in the records table

The records table has no `prefixed` column.
PROV-Dictionary's records need a prefixed keyword. The column arrives with that extension. The table is read in
[stop 3](stops/3-records.md).
