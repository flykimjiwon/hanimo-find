# Security Policy

## Supported version

The only frozen protocol version is `0.1.0`. There is no production release yet.

## Reporting

Do not publish suspected vulnerabilities, secret material, exploit payloads, or
private repository contents in a public issue. As of 2026-07-14, the intended
`flykimjiwon/hanimo-find` GitHub repository has not been created and no monitored
remote security-reporting channel exists. GitHub private vulnerability
reporting is available only for public repositories, so keep the staging
repository private through hosted CI and the other private-stage gates. Then
change the repository to public and immediately enable and verify private
vulnerability reporting, including the visible `Report a vulnerability` route,
before announcing the repository or creating any tag or release. Update this
section with the confirmed route. If enablement or verification fails, return
the repository to private visibility when possible; otherwise halt all
announcement, tagging, and release activity and document a private fallback
reporting route before proceeding. Until the GitHub route is verified, retain
the report locally and contact the owner through an independently known private
channel before sharing sensitive material.

Include the affected schema version, minimal reproduction, expected and actual
exit status, and whether a root escape, symlink traversal, stale-evidence bypass,
stdout protocol contamination, or secret disclosure is involved. Use synthetic
fixtures whenever possible.

## Security invariants

Hanimo Find must not follow symlinks, escape its scan root, expose ignored or
secret content, accept stale or forged evidence, normalize source bytes, or mix
diagnostics into MCP stdout. See `SPEC.md` for normative behavior.

The stdio MCP server captures its canonical startup directory as its trusted
search base. Tool callers may select only normal-component relative nested
directories beneath it. The MCP boundary joins that path lexically without
resolving or reopening it, and core search performs component-by-component
no-follow acquisition. Absolute, parent, platform-root, missing, non-directory,
and final or intermediate symlink targets fail closed and cannot become ambient
authority.

Direct search and diagnosis roots use the same component-wise no-follow opener.
Relative roots begin at the trusted current-directory capability; absolute roots
must already be lexical, symlink-free paths. Final and intermediate root
symlinks are rejected before discovery or source reads.

`bundle_sha256` provides unkeyed integrity and self-consistency only. It is not a
signature, trusted timestamp, or proof of authorship. Anyone able to replace the
source, bundle payload, and digest can create a new self-consistent artifact.
Trust against that actor requires a separately protected digest, append-only
Claim ledger, or signature. Until that later trust anchor exists, `Verified`
means only that the stored attested payload is unchanged and matches the current
source.

The verifier treats the bundle as untrusted input even when `bundle_sha256` is
self-consistent. It bounds serialized input before deserialization, validates
array and numeric-budget ceilings before attestation or live reads, recomputes
the critic completeness rule, and caps aggregate live source bytes independently
of artifact-supplied budgets. Absolute, empty, NUL-containing, dot-segment, and
root-traversing block locators are security failures and are never opened.
The artifact's display-only root never selects ambient authority: the caller
supplies a matching root capability, and mismatch, parent traversal, or a final
symlink fails before source reads. Only a missing source is stale evidence;
permission denial and every other source open/read error fail closed.
