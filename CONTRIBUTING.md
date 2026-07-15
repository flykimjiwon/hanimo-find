# Contributing

This repository contains the frozen v0 contract and its Rust implementation.
Participation is governed by the [Code of Conduct](CODE_OF_CONDUCT.md).

## Contract rules

1. Read `SPEC.md` and preserve every `0.1.0` constant unless a deliberate schema
   version change is proposed.
2. Keep JSON authoritative; rendered Markdown must be a pure projection.
3. Add or update conformance cases for every observable contract change.
4. Use synthetic fixtures. Never commit credentials, private source, generated
   build output, dependency caches, or unrelated worktree artifacts.
5. Validate every JSON file with `jq`, run the exact-constant cross-check, and
   record the commands and results in task evidence.
6. Keep changes narrowly scoped and explain security and compatibility effects.

## Local development

The toolchain is pinned by `rust-toolchain.toml` (stable channel with `rustfmt`,
`clippy`, and `rust-src`). The **minimum supported Rust version is 1.88.0**;
changes must build on it.

Run the same checks the gate enforces before opening a pull request:

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

When you change a packaged distribution document (for example `README.md` or
`SPEC.md`), synchronize the per-crate copies:

```sh
./scripts/sync-package-assets.sh --write
```

When you change supply-chain, packaging, or distribution inputs (dependencies,
`Cargo.lock`, `deny.toml`, `about.toml`, SBOMs, or release assets), run the full
fail-closed gate after installing the pinned tools listed in
[RELEASE_GATE.md](RELEASE_GATE.md):

```sh
./scripts/release-gate.sh
```

Regenerate committed SBOMs with `./scripts/generate-sboms.sh` after changing
`Cargo.lock` or package metadata.

## Provenance and sign-off

By contributing you certify that you have the right to submit your work under
the [Apache License 2.0](LICENSE) and that your provenance claims are accurate.
As recorded in [PROVENANCE.md](PROVENANCE.md), this is a clean-room
implementation of `SPEC.md`: do not copy, quote, or use non-public
implementation material to direct a change.

Sign off each commit with the [Developer Certificate of Origin](https://developercertificate.org)
using `git commit --signoff`, which appends a `Signed-off-by` trailer asserting
that certification.
