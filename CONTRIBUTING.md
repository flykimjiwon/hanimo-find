# Contributing

This repository contains the frozen v0 contract and its Rust implementation.

1. Read `SPEC.md` and preserve every `0.1.0` constant unless a deliberate schema
   version change is proposed.
2. Keep JSON authoritative; rendered Markdown must be a pure projection.
3. Add or update conformance cases for every observable contract change.
4. Use synthetic fixtures. Never commit credentials, private source, generated
   build output, dependency caches, or unrelated worktree artifacts.
5. Validate every JSON file with `jq`, run the exact-constant cross-check, and
   record the commands and results in task evidence.
6. Keep changes narrowly scoped and explain security and compatibility effects.

Contributors certify that they have the right to submit their work under the
Apache License 2.0 and that provenance claims are accurate.
