#!/usr/bin/env python3
"""Check that every snippet a page includes exists.

A page takes source in with `{{#include path}}` or `{{#include
path:anchor}}`, and mdBook renders an empty block without an error when
the file has no such anchor. This script reads every page under
book/src, resolves each include against the page's directory, and exits
1 at the first file that is missing or that lacks the `ANCHOR: name` or
`ANCHOR_END: name` comment. Run with `python3 -I` from the repository
root.
"""

import re
import sys
from pathlib import Path

SRC = Path("book/src")
INCLUDE = re.compile(r"\{\{#include\s+([^}\s:]+)(?::([^}\s]+))?\s*\}\}")


def main():
    count = 0
    for page in sorted(SRC.rglob("*.md")):
        if SRC / "api" in page.parents:
            continue
        lines = page.read_text(encoding="utf-8").splitlines()
        for number, line in enumerate(lines, 1):
            for path, anchor in INCLUDE.findall(line):
                target = page.parent / path
                if not target.is_file():
                    sys.exit(f"{page}:{number}: {path} is not a file")
                count += 1
                if not anchor:
                    continue
                text = target.read_text(encoding="utf-8")
                for mark in ("ANCHOR", "ANCHOR_END"):
                    if not re.search(rf"\b{mark}:\s*{re.escape(anchor)}(?![\w-])", text):
                        sys.exit(f"{page}:{number}: {path} has no '{mark}: {anchor}'")
    if not count:
        sys.exit("anchors: found no include under book/src")
    print(f"anchors ok, {count} includes")


if __name__ == "__main__":
    main()
