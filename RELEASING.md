# Releasing

Both crates share one version. It is set twice in the root `Cargo.toml`,
as `version` under `[workspace.package]` and as the `version` of the
`provkit` entry under `[workspace.dependencies]`. A release is a tag
`v<version>` on `main`. `release.yml` checks that the tag matches the
version of `provkit-cli` and builds the binaries.

The repository must be public before the tag is pushed. GitHub's artifact
attestations fail in a private repository on a free personal account.

## Steps

1. Set the date of the entry in `CHANGELOG.md` to the release day. Set both
   versions in `Cargo.toml` and the Status line of the README if they are not
   already the one to release.
2. Run the main checks CI runs: `cargo fmt --check`, `cargo clippy --workspace
   --all-targets --all-features -- -D warnings`, `cargo test --workspace
   --all-features`, `cargo doc --workspace --no-deps` with
   `RUSTDOCFLAGS=-D warnings`, `cargo deny check`, `typos` and
   `sh book/build.sh`. Then run
   `cargo clean` and `cargo package --workspace`. Cargo reuses an earlier
   packaged build of the same version, and the verify step then fails on stale
   code.
3. Commit, and fast-forward `main` to the release commit.
4. Set the Pages source of the repository to "GitHub Actions" (Settings,
   Pages, Build and deployment). Do this once, after the repository is public
   and before the next push to `main`. `pages.yml` deploys with
   `actions/deploy-pages`, which fails otherwise, and the README and
   CHANGELOG link the site.
5. Tag with `git tag -a v<version> -m "provkit <version>"`. Push `main` and
   the tag.
6. Wait for `release.yml` to finish. It checks the tag, creates the GitHub
   release with the changelog entry as its body, and builds six archives:
   `provkit-<target>.tar.gz`, or `.zip` on Windows. Each archive holds
   `THIRD-PARTY.md`. Each has a digest file named `provkit-<target>.sha256`,
   and the attestation step in its matrix job covers the archive.
7. Run `cargo publish -p provkit`. Wait until `cargo info provkit` shows the
   version.
8. Run `cargo publish -p provkit-cli`.
9. On one machine, run `cargo install --locked provkit-cli` and
   `cargo binstall provkit-cli`. Run `provkit --version` after each.

## If `release.yml` fails after the tag

Fix the cause on `main`. Delete the tag locally and remotely
(`git tag -d v<version>`; `git push origin :refs/tags/v<version>`). Delete the
GitHub release if one was created without assets, then tag again. Nothing has
been published to crates.io, so the version number can stay.

## If `provkit` is published and `provkit-cli` is not

A published version cannot be replaced. Fix the cause in `provkit-cli` alone.
Bump both places in `Cargo.toml` to the next patch version and release again
from step 1. The `provkit` version already on crates.io stays, and the new
`provkit-cli` depends on the new `provkit` version.

## Moving the toolchain

The pinned toolchain appears in `ci.yml`, `release.yml`, `pages.yml`,
`rust-version` in `Cargo.toml`, the minimum supported Rust version in both
READMEs and chapter 1 of the book. Move them together. A higher minimum is a minor release.
