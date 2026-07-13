# Provenance Record

## Clean-room boundary

Hanimo Find v0 is an independent, clean-room implementation of the behavior documented in
`SPEC.md`. Source, tests, schemas, and conformance vectors in this repository were authored
from that specification and synthetic fixtures. No proprietary source code, confidential
tests, production datasets, credentials, model outputs, or reverse-engineered binary
artifacts were used as implementation inputs.

The public behavior specification is the compatibility boundary. A contributor who has
access to non-public implementation material must not copy it, quote it, or use it to direct
an implementation change.

## Third-party inputs

Third-party code enters only through dependencies declared in `Cargo.toml` and resolved in
`Cargo.lock`. `deny.toml` defines the allowed source and license policy;
`THIRD_PARTY_LICENSES.md` is generated from the resolved graph. The unmodified Apache License
2.0 text in `LICENSE` is the project's legal text.

## Reproducible release evidence

The release gate uses locked dependencies, MSRV 1.88.0, cross-platform CI, dependency and
license policy checks, source-package inspection, a CycloneDX JSON SBOM, and a directory-mode
secret scan. Tool versions and immutable action revisions are recorded in the workflows and
`RELEASE_GATE.md`. Generated notices and SBOMs must match the current lockfile before release.

Operational publication authority, intended GitHub distribution, naming, and
the Apache-2.0 selection are recorded in
[PUBLICATION_DECISION.md](PUBLICATION_DECISION.md) and
[NAME_REVIEW.md](NAME_REVIEW.md). This technical provenance record does not
turn that explicit project direction into a legal opinion, trademark clearance,
third-party permission, or representation about rights beyond the evidence
stated in those records.
