#!/bin/sh
# Build the book: the idiom index, the check that the New to Rust table
# matches the stop pages' idiom lists, the check that the three-views doctest
# repeats the two example files, the library the doctests link against,
# the rustdoc under book/book/api, the HTML, and the doctests.
set -eu
cd "$(dirname "$0")/.."

python3 -I book/tools/rust_notes.py --self-test
python3 -I book/tools/rust_notes.py
python3 -I book/tools/check_new_to_rust.py
python3 -I - book/src/start/three-views.md book/examples/edit.provn book/examples/edit.json <<'EOF'
import re, sys
copies = re.findall(r'r#"(.*?)"#', open(sys.argv[1], encoding="utf-8").read(), re.S)
files = [open(p, encoding="utf-8").read() for p in sys.argv[2:]]
sys.exit(0 if copies == files else "three-views.md: a raw string differs from its file under book/examples")
EOF
cargo build -p provkit --target-dir book/target
cargo doc -p provkit --no-deps --target-dir book/target
mdbook build book
rm -rf book/book/api
cp -R book/target/doc book/book/api
mdbook test book -L book/target/debug/deps
