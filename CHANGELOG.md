# Changelog

All notable changes to Hanimo Find will be documented in this file. The project
is currently a source beta; entries under Unreleased are not a production
release announcement.

## Unreleased

### Fixed

- Corrected the SECURITY.md reporting section to the live public-repository
  GitHub private vulnerability reporting route, replacing the stale pre-launch
  checklist.
- Removed the dead `documentation` crate-metadata links; docs.rs is not built
  for unpublished crates, and the SBOMs were regenerated accordingly.
- Diagnosis now walks sources with the same hermetic ignore policy as search:
  global git ignore files, ancestor ignore files, and git-repository detection
  no longer change findings or the reported digest across environments, and
  root-level ignore files are honored without requiring a git repository.
- Diagnosis now visits sources in canonical root-relative raw path byte order,
  matching the search scanner's one canonical source order, instead of
  platform path-component order.

### Added

- Consumer, threat-model, release-verification, and positioning guides
  (`docs/CONSUMING_EVIDENCE.md`, `THREAT_MODEL.md`, `docs/VERIFYING_RELEASES.md`,
  `FAQ.md`).
- A Code of Conduct, a maintainer and bus-factor disclosure (`MAINTAINERS.md`),
  and issue and pull-request templates.
- A tool-selection comparison table, a "when not to use" section, and a
  CJK/Unicode-normalization caveat in the README; an expanded contributing guide
  with the minimum supported Rust version and DCO sign-off.
- MCP stdio tools `verify_evidence` and `diagnose_repo` alongside
  `search_evidence`, closing the search → act → re-verify loop for MCP
  clients with the same acceptance condition as CLI exit 0.
- An MCP client integration guide (`docs/MCP.md`) with per-tool contracts and
  Claude Code configuration.
- Source installation instructions with the minimum supported Rust version.
- v0.1 `search`, `verify`, `diagnose`, and MCP stdio command documentation.
- Public research corrections for logical retrieval, compiled wikis, structured
  retrieval, long-context caching, and publication-oriented synthesis systems.
- A decision-complete v0.2-to-v1 roadmap for QueryPlan ASTs, verified Claims,
  trusted attestation, a single-path Evidence-Compiled Wiki, held-out refinement,
  and progressive structure navigation.
- An original-source-span benchmark protocol with separate build, query, update,
  and verification cost accounting.

### Security

- The secret-path policy additionally excludes common key-material and
  credential-named sources (`id_rsa`-family private keys,
  `.p12`/`.pfx`/`.ppk`/`.jks`/`.keystore` containers, and
  `password`/`api_key`/`apikey` path substrings) from evidence content.
- Documented that `bundle_sha256` is an unkeyed consistency digest, not a
  signature, authorization decision, proof of authorship, or trusted timestamp.

### Limitations

- Documented that v0.1 is exact lexical search without a semantic-recall,
  paraphrase, production-readiness, or universal performance claim.
