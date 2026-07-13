# Release Gate

Run `./scripts/release-gate.sh` from the repository root after installing the pinned tools:

```text
cargo-deny 0.20.2
cargo-about 0.9.1
cargo-cyclonedx 0.5.9
gitleaks 8.30.1
```

Install cargo-about with its CLI feature:
`cargo install --locked cargo-about --version 0.9.1 --features cli`.

The gate fails closed on formatting, compilation, Clippy warnings, tests, RustSec advisories,
source or license policy violations, stale notices, missing release assets, malformed or stale
CycloneDX SBOMs, source-package contents, or discovered secrets. CI also builds on Linux,
macOS, Windows, and the declared MSRV (Rust 1.88.0).

The Cargo members are intentionally `publish = false`; therefore registry publish dry-runs
are not applicable. Run `./scripts/sync-package-assets.sh --write` after changing a required
distribution document. The release gate runs the script in check mode, inspects the exact file
list for both members, and creates both archives. The CLI archive uses a command-local Cargo
patch for its unpublished `hanimo-core` dependency; no registry or manifest state is changed.
Both archives must contain synchronized copies of the legal, security, specification,
research, roadmap, benchmark, provenance, name-review, and evidence-schema assets. Set
`RELEASE_ALLOW_DIRTY=1` only for a local pre-commit check; CI never uses that override.

Tool sources verified on 2026-07-14:

- https://github.com/EmbarkStudios/cargo-deny/releases/tag/0.20.2
- https://github.com/EmbarkStudios/cargo-about/releases/tag/0.9.1
- https://github.com/CycloneDX/cyclonedx-rust-cargo/releases/tag/cargo-cyclonedx-0.5.9
- https://github.com/gitleaks/gitleaks/releases/tag/v8.30.1
- https://github.com/actions/checkout/releases/tag/v7.0.0
- https://github.com/actions/upload-artifact/releases/tag/v7.0.1

GitHub Actions are pinned to immutable full commit SHAs in the workflow files. Gitleaks is
downloaded from its official release and verified against the release checksum before use.
