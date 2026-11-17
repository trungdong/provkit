# The provkit book

This book is for people who will read or change the provkit source:
contributors, reviewers, and the maintainer coming back after a gap. It
assumes you can program, and its two tracks say what to read first when
Rust or PROV is new to you; pick the track for the one you know less. It is
not a user guide.
To install and run `provkit`, read the
[README](https://github.com/trungdong/provkit/blob/main/README.md); for
the library's API, the [reference](api/provkit/index.html); for where
provkit departs from the Recommendations or differs from ProvPy and
ProvToolbox,
[CONFORMANCE.md](https://github.com/trungdong/provkit/blob/main/CONFORMANCE.md).
The book describes the source on `main`.

The chapters follow the provkit source one layer at a time, lowest
layer first, so each chapter comes after the ones it builds on. Every
chapter says what the code does, why it has that shape, which Rust it
uses, and what to run to see it work.

Start with [One document, three views](start/three-views.md). Then pick
the track that fits you, [New to Rust](start/new-to-rust.md) or
[New to PROV](start/new-to-prov.md), or go straight to
[Chapter 1](chapters/1-orientation.md).
