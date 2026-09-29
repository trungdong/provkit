# provkit

W3C PROV data model with PROV-JSON and PROV-N read and write.

Status: pre-release

## Contributing

Three checks run in CI only: the `wasm32-unknown-unknown` build, `cargo deny`
for supply-chain audits, and `typos` for spell checking. To run these locally, install
them with `cargo install cargo-deny typos-cli` and add the wasm32 target with `rustup`.
