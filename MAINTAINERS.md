# Maintainers

## Current maintainers

| Maintainer | GitHub | Areas |
| --- | --- | --- |
| Jiwon Kim | [@flykimjiwon](https://github.com/flykimjiwon) | All (source, release, security, supply-chain policy) |

Ownership is recorded normatively in [`.github/CODEOWNERS`](.github/CODEOWNERS):
every path, and specifically `/.github/`, `Cargo.lock`, `Cargo.toml`,
`deny.toml`, `about.toml`, `PROVENANCE.md`, `THIRD_PARTY_LICENSES.md`, and
`sbom/`, requires owner review.

## Bus factor

This project currently has a **bus factor of one**. A single maintainer holds
review, release, and security-response authority. This is disclosed honestly
rather than implied away:

- There is no second reviewer and no separate enforcement or security committee.
- Branch protection on `main` requires status checks, linear history, and
  resolved conversations, but the sole-maintainer administrator bypass
  (`enforce_admins: false`) remains enabled. This is a documented operational
  choice for a single-maintainer project, not a claim of two-person control.
- The `refs/tags/v*` ruleset blocks tag deletion and non-fast-forward tag
  movement; the owner retains an always-available bypass.

Adopters who require multi-party review, guaranteed response times, or
segregated release duties should account for this in their own risk assessment.
See [docs/VERIFYING_RELEASES.md](docs/VERIFYING_RELEASES.md) for consumer-side
verification that does not depend on trusting the maintainer.

## Release authority

Releases follow [RELEASE_GATE.md](RELEASE_GATE.md). The maintainer runs the
fail-closed gate locally and via hosted CI (Linux, macOS, Windows, and the
declared MSRV) before tagging. Version tags are protected by the ruleset above.

## Security response

Report vulnerabilities through the route in [SECURITY.md](SECURITY.md); do not
open a public issue for a suspected vulnerability. Because this is a
single-maintainer project, security response is **best effort with no
guaranteed timeline**. Reports are triaged as promptly as the maintainer can,
and adopters requiring a contractual SLA should not assume one exists.

## Code of Conduct

The maintainer is also responsible for [Code of Conduct](CODE_OF_CONDUCT.md)
enforcement. There is no separate committee.

## Decision record

Publication authority, naming, and license selection are recorded in
[PUBLICATION_DECISION.md](PUBLICATION_DECISION.md) and
[NAME_REVIEW.md](NAME_REVIEW.md). Clean-room provenance is recorded in
[PROVENANCE.md](PROVENANCE.md).
