# Changelog

All notable changes to Hanimo Find will be documented in this file. The project
is currently a source beta; entries under Unreleased are not a production
release announcement.

## Unreleased

### Fixed

- Diagnosis now walks sources with the same hermetic ignore policy as search:
  global git ignore files, ancestor ignore files, and git-repository detection
  no longer change findings or the reported digest across environments, and
  root-level ignore files are honored without requiring a git repository.
- Diagnosis now visits sources in canonical root-relative raw path byte order,
  matching the search scanner's one canonical source order, instead of
  platform path-component order.

### Added

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
