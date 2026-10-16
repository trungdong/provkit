#!/usr/bin/env python3
"""Build the Rust idiom index from the `Rust notes:` trailers in the git log.

Every commit message ends with a trailer such as

    Rust notes:
    - `matches!` takes a pattern, so one test covers four variants:
      https://doc.rust-lang.org/std/macro.matches.html

or `Rust notes: none.`. This script groups the bullets by the document they
link to and writes a Markdown page. Run with `python3 -I`.
"""

import argparse
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

RS = "\x1e"
FS = "\x1f"
LINK = re.compile(r"https?://[^\s<>()\[\]`]+")
TRAILER = re.compile(r"^Rust notes:[ \t]*(.*)$", re.M)
NONE = re.compile(r"^none\b", re.I)
NO_IDIOM = re.compile(r"no idiom", re.I)

SOURCES = [
    ("The Rust book", "doc.rust-lang.org/book"),
    ("The Rust reference", "doc.rust-lang.org/reference"),
    ("The standard library", "doc.rust-lang.org/std"),
    ("The standard library", "doc.rust-lang.org/core"),
    ("The Cargo book", "doc.rust-lang.org/cargo"),
    ("rustdoc and clippy", "doc.rust-lang.org/rustdoc"),
    ("rustdoc and clippy", "doc.rust-lang.org/clippy"),
    ("rustdoc and clippy", "rust-lang.github.io/rust-clippy"),
    ("Crate docs", "docs.rs"),
]
ORDER = [
    "The Rust book",
    "The Rust reference",
    "The standard library",
    "The Cargo book",
    "rustdoc and clippy",
    "Crate docs",
    "Elsewhere",
    "Without a link",
]


def git_log(repo):
    fmt = f"{RS}%H{FS}%s{FS}%b{FS}"
    out = subprocess.run(
        ["git", "-C", str(repo), "log", f"--format={fmt}", "--name-only"],
        check=True,
        capture_output=True,
    ).stdout
    return out.decode("utf-8", "replace")


def parse_log(text):
    """Yield (sha, subject, body, files) for each commit in the log text."""
    for record in text.split(RS):
        parts = record.split(FS)
        if len(parts) < 4:
            continue
        sha, subject, body, tail = parts[0], parts[1], parts[2], parts[3]
        files = [line.strip() for line in tail.splitlines() if line.strip()]
        yield sha.strip(), subject.strip(), body, files


def notes(body):
    """Return the bullets of the Rust notes trailer, or [] when there are none."""
    match = TRAILER.search(body)
    if not match:
        return []
    rest = (match.group(1) + body[match.end():]).strip()
    if not rest or NONE.match(rest):
        return []
    bullets, current = [], []
    after_blank = False
    for line in rest.splitlines():
        text = line.strip()
        if not text:
            after_blank = True
            continue
        if after_blank and not text.startswith("- "):
            break
        after_blank = False
        if text.startswith("- "):
            if current:
                bullets.append(" ".join(current))
            current = [text[2:]]
        else:
            current.append(text)
    if current:
        bullets.append(" ".join(current))
    return [
        b
        for b in bullets
        if not ((NO_IDIOM.search(b) or NONE.match(b)) and not LINK.search(b))
    ]


def link_of(bullet):
    match = LINK.search(bullet)
    return match.group(0).rstrip(".,;:") if match else None


def source_of(link):
    if link is None:
        return "Without a link"
    for name, needle in SOURCES:
        if needle in link:
            return name
    return "Elsewhere"


def title_of(link):
    """A heading for a link: its fragment, else its last path component."""
    tail = link.split("#", 1)[1] if "#" in link else link.rstrip("/").rsplit("/", 1)[-1]
    tail = re.sub(r"\.html$", "", tail)
    return " ".join(part for part in re.split(r"[-_]+", tail) if part) or link


def text_of(bullet, link):
    """The bullet without its first link and the connective left dangling."""
    text = bullet.replace(link, "", 1) if link else bullet
    text = re.sub(r"\s+", " ", text)
    text = re.sub(r"\b[Ss]ee ?\.", "", text)
    text = re.sub(r"\b([Ss]ee) and ", r"\1 ", text)
    text = re.sub(r": and ", " and ", text)
    return re.sub(r"\s+", " ", text).strip(" :,;")


CODE = re.compile(r"`[^`]*`")


def markdown(text):
    """Escape angle brackets outside code spans and mark bare links as links."""

    def plain(segment):
        segment = segment.replace("<", "&lt;").replace(">", "&gt;")
        return LINK.sub(
            lambda m: f"<{m.group(0).rstrip('.,;:')}>"
            + m.group(0)[len(m.group(0).rstrip(".,;:")) :],
            segment,
        )

    out, pos = [], 0
    for match in CODE.finditer(text):
        out += [plain(text[pos : match.start()]), match.group(0)]
        pos = match.end()
    out.append(plain(text[pos:]))
    return "".join(out)


def entries(log_text):
    """Yield dicts with source, link, text, sha, subject and files."""
    for sha, subject, body, files in parse_log(log_text):
        for bullet in notes(body):
            link = link_of(bullet)
            text = text_of(bullet, link)
            yield {
                "source": source_of(link),
                "link": link,
                "text": text,
                "sha": sha,
                "subject": subject,
                "files": files,
            }


def render(items):
    grouped = defaultdict(lambda: defaultdict(list))
    for item in items:
        grouped[item["source"]][item["link"] or ""].append(item)
    out = [
        "# Rust idioms, from the commit log",
        "",
        "Every commit message ends with a `Rust notes:` trailer naming the",
        "idioms the change introduced, each with a link. This page is",
        "generated from those trailers by `book/tools/rust_notes.py` and is",
        "not edited by hand. Entries are grouped by the document they link",
        "to; each one names the commit it came from and the files it touched.",
        "",
    ]
    for source in ORDER:
        if source not in grouped:
            continue
        out += [f"## {source}", ""]
        for link in sorted(grouped[source], key=lambda l: title_of(l) if l else ""):
            if link:
                out += [f"### {title_of(link)}", "", f"<{link}>", ""]
            for item in grouped[source][link]:
                files = ", ".join(f"`{f}`" for f in item["files"][:4])
                where = f"{item['subject']} ({item['sha'][:7]})"
                if files:
                    where += f", in {files}"
                out += [f"- {markdown(item['text'])}", f"  {where}"]
            out.append("")
    return "\n".join(out).rstrip() + "\n"


SELF_TEST_LOG = (
    f"{RS}1aaaaaaa{FS}Add a thing{FS}Why it changed.\n\n"
    "Rust notes:\n"
    "- `matches!` takes a pattern:\n"
    "  https://doc.rust-lang.org/std/macro.matches.html\n"
    "- `insert` accepts any `impl Into<Value>`:\n"
    "  https://doc.rust-lang.org/book/ch10-02-traits.html#traits-as-parameters\n"
    f"{FS}\n\nsrc/a.rs\nsrc/b.rs\n"
    f"{RS}2aaaaaaa{FS}Fix a word{FS}Prose only.\n\nRust notes: none.\n{FS}\n\nREADME.md\n"
    f"{RS}3aaaaaaa{FS}Escape it{FS}Rust notes: `\\u{{...}}` escapes write code points, see\n"
    f"https://doc.rust-lang.org/reference/tokens.html#unicode-escapes.\n{FS}\n\nsrc/c.rs\n"
    f"{RS}4aaaaaaa{FS}Reword{FS}Rust notes:\n- This commit changes prose only and introduces no idiom.\n{FS}\n\nREADME.md\n"
    f"{RS}5aaaaaaa{FS}Share it{FS}Rust notes:\n- Arc<str> shares one allocation; `Vec<u8>` does not.\n"
    f"  https://doc.rust-lang.org/std/sync/struct.Arc.html\n{FS}\n\nsrc/d.rs\n"
    f"{RS}6aaaaaaa{FS}Stop at the blank line{FS}Rust notes:\n- `?` returns early.\n"
    f"  https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html\n\n"
    f"Follow-up: none.\n{FS}\n\nsrc/e.rs\n"
    f"{RS}7aaaaaaa{FS}Tidy{FS}Rust notes:\n- None new.\n{FS}\n\nsrc/f.rs\n"
    f"{RS}8aaaaaaa{FS}Tidy again{FS}Rust notes: none new.\n{FS}\n\nsrc/f.rs\n"
    f"{RS}9aaaaaaa{FS}Link twice{FS}Rust notes:\n"
    "- `impl Trait` in argument position is a generic. See\n"
    "  https://doc.rust-lang.org/book/ch10-02-traits.html#traits-as-parameters.\n"
    "  A call that writes x.into() is E0283. See\n"
    f"  https://doc.rust-lang.org/error_codes/E0283.html.\n{FS}\n\nsrc/g.rs\n"
)


def self_test():
    items = list(entries(SELF_TEST_LOG))
    by_sha = defaultdict(list)
    for item in items:
        by_sha[item["sha"]].append(item)
    assert sorted(by_sha) == ["1aaaaaaa", "3aaaaaaa", "5aaaaaaa", "6aaaaaaa", "9aaaaaaa"], items
    first = by_sha["1aaaaaaa"]
    assert first[0]["source"] == "The standard library"
    assert first[0]["link"] == "https://doc.rust-lang.org/std/macro.matches.html"
    assert first[0]["files"] == ["src/a.rs", "src/b.rs"]
    assert first[1]["source"] == "The Rust book"
    assert first[1]["text"] == "`insert` accepts any `impl Into<Value>`"
    (escaped,) = by_sha["3aaaaaaa"]
    assert escaped["source"] == "The Rust reference"
    assert escaped["link"].endswith("#unicode-escapes")
    assert escaped["text"] == "`\\u{...}` escapes write code points"
    (angles,) = by_sha["5aaaaaaa"]
    assert angles["text"] == "Arc<str> shares one allocation; `Vec<u8>` does not."
    (stopped,) = by_sha["6aaaaaaa"]
    assert stopped["text"] == "`?` returns early."
    (twice,) = by_sha["9aaaaaaa"]
    assert twice["link"].endswith("#traits-as-parameters")
    assert twice["text"] == (
        "`impl Trait` in argument position is a generic. A call that writes"
        " x.into() is E0283. See https://doc.rust-lang.org/error_codes/E0283.html."
    )
    assert title_of("https://doc.rust-lang.org/book/ch19-03-pattern-syntax.html#-bindings") == "bindings"
    assert (
        title_of(
            "https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html"
            "#a-shortcut-for-propagating-errors-the--operator"
        )
        == "a shortcut for propagating errors the operator"
    )
    page = render(items)
    assert "## The Rust book" in page
    assert "### traits as parameters" in page
    assert "- Arc&lt;str&gt; shares one allocation; `Vec<u8>` does not." in page
    assert "See <https://doc.rust-lang.org/error_codes/E0283.html>." in page
    assert "Follow-up" not in page
    assert "1aaaaaa" in page and "2aaaaaa" not in page
    assert "7aaaaaa" not in page and "8aaaaaa" not in page
    print("self-test ok")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repo", default=".", help="repository root")
    parser.add_argument(
        "--out", default="book/src/reference/idioms.md", help="page to write"
    )
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    page = render(entries(git_log(Path(args.repo))))
    out = Path(args.repo) / args.out
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(page, encoding="utf-8")
    print(f"wrote {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
