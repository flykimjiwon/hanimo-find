<!--
Read CONTRIBUTING.md and SPEC.md first. Keep changes narrowly scoped.
Do not report security fixes for undisclosed vulnerabilities in a public PR;
coordinate through SECURITY.md first.
-->

## Summary

<!-- What changed and why. Link any issue. -->

## Contract and compatibility

- [ ] Preserved every `0.1.0` constant, or this PR proposes a deliberate schema
      version change with a migration rule.
- [ ] JSON remains authoritative; any rendered Markdown is a pure projection.
- [ ] Added or updated conformance cases for every observable contract change.
- [ ] Explained the security and compatibility effects below.

## Evidence and hygiene

- [ ] Synthetic fixtures only; no credentials, private source, generated build
      output, dependency caches, or unrelated worktree artifacts.
- [ ] Validated changed JSON with `jq` and ran the exact-constant cross-check.
- [ ] Ran `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`,
      and `cargo test --locked` (and `./scripts/release-gate.sh` when supply-chain,
      packaging, or distribution documents changed). Results recorded below.
- [ ] Ran `./scripts/sync-package-assets.sh --write` if a packaged distribution
      document (README/SPEC/etc.) changed.

## Provenance

- [ ] I have the right to submit this work under the Apache License 2.0 and my
      provenance claims are accurate (see PROVENANCE.md). No non-public
      implementation material was copied, quoted, or used to direct this change.

## Security and compatibility effects

<!-- Root/symlink handling, evidence boundary, exit codes, determinism, budgets. -->

## Verification evidence

<!-- Paste the commands you ran and their key results. -->
