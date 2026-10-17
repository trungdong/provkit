#!/usr/bin/env python3
"""Check that the New to Rust table matches the stop pages' idiom lists.

Every stop page closes with an "Idioms first seen here" list, and the
table in book/src/start/new-to-rust.md repeats those idioms by hand.
This script rebuilds the (label, stop file, URL) triples from both and
exits 1 with the first difference. Run with `python3 -I` from the
repository root.
"""

import re
import sys
from pathlib import Path

SRC = Path("book/src")
LIST_ITEM = re.compile(r"^- `([^`]+)`: (\S+)$")
TABLE_ROW = re.compile(
    r"^\| `([^`]+)` \| \[Stop (\d)\]\(\.\./stops/([^)]+)\) \| \[[^\]]+\]\(([^)]+)\) \|$"
)


def from_lists():
    triples = []
    for page in sorted((SRC / "stops").glob("[0-8]-*.md")):
        text = page.read_text(encoding="utf-8")
        _, _, tail = text.partition("\n## Idioms first seen here\n")
        for line in tail.splitlines():
            match = LIST_ITEM.match(line)
            if match:
                triples.append((match.group(1), page.name, match.group(2)))
    return triples


def from_table():
    triples = []
    for line in (SRC / "start" / "new-to-rust.md").read_text(encoding="utf-8").splitlines():
        match = TABLE_ROW.match(line)
        if not match:
            continue
        label, stop, page, url = match.groups()
        if not page.startswith(f"{stop}-"):
            sys.exit(f"new-to-rust.md: `{label}` says Stop {stop} but links {page}")
        triples.append((label, page, url))
    return triples


def main():
    lists, table = from_lists(), from_table()
    if not lists or not table:
        sys.exit("new-to-rust: found no idiom lists or no table rows")
    for index, (want, have) in enumerate(zip(lists, table), 1):
        if want != have:
            sys.exit(
                f"new-to-rust.md row {index} differs from the stop page list:\n"
                f"  stop page: {want}\n"
                f"  table row: {have}"
            )
    if len(lists) > len(table):
        sys.exit(f"new-to-rust.md lacks a row for {lists[len(table)]}")
    if len(table) > len(lists):
        sys.exit(f"new-to-rust.md row {len(lists) + 1} has no stop page list entry: {table[len(lists)]}")
    print(f"new-to-rust ok, {len(table)} idioms")


if __name__ == "__main__":
    main()
