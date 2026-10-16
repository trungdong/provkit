#!/bin/sh
# Build the book: the idiom index, the library the doctests link against,
# the rustdoc under book/book/api, the HTML, and the doctests.
set -eu
cd "$(dirname "$0")/.."

python3 -I book/tools/rust_notes.py --self-test
python3 -I book/tools/rust_notes.py
cargo build -p provkit --target-dir book/target
cargo doc -p provkit --no-deps --target-dir book/target
mdbook build book
rm -rf book/book/api
cp -R book/target/doc book/book/api
mdbook test book -L book/target/debug/deps
