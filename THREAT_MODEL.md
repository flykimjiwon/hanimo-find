# Threat Model

This document is a structured companion to [SECURITY.md](SECURITY.md). SECURITY.md
states the security invariants and reporting policy; this file organizes the same
behavior into assets, trust boundaries, an attacker-capability table, and explicit
non-goals so a reader can reason about what Hanimo Find v0.1 does and does not
defend.

Scope is the frozen `0.1.0` protocol as implemented in this repository. There is
no production release. Every claim below describes behavior enforced by the
current source; where a defense stops, this document says so plainly rather than
implying more. The normative contract is [SPEC.md](SPEC.md).

## 1. Assets

The things this tool tries to keep sound.

- **Source bytes.** The exact on-disk bytes of files beneath an authorized root.
  Search reads them under a capability, and evidence blocks carry the raw
  root-relative path, one-based inclusive line range, zero-based byte offsets,
  the LF-canonical block content, the matched literals, and a SHA-256 of the
  complete source file (`source_sha256`). Invalid UTF-8 is preserved as base64,
  not replacement text, so bytes are never silently rewritten in transit.
- **Evidence integrity.** An `EvidenceBundle` is internally self-consistent: its
  `bundle_sha256` is an unkeyed digest computed over the immutable payload
  (excluding the digest field itself), and verification recomputes it. Integrity
  here means "the stored bundle is unmodified and its blocks still match the
  current source," not "the bundle is authentic or authored by a trusted party."
- **The consumer's trust in citations.** The end asset is the reader's ability to
  rely on a citation. Every accepted block is addressed by `path:line` with a
  source digest and a `why[]` rationale, and the deterministic critic gates
  whether the bundle is presented as sufficient. The tool emits evidence for a
  human or agent to check; it does not certify that the cited text is true.

## 2. Trust boundaries

Each boundary is an authority a caller grants and the tool refuses to exceed.

### 2.1 The search / diagnose root capability

`hanimo find search` and `hanimo find diagnose` operate under one directory
capability. In `crates/hanimo-core/src/root.rs`, a relative root begins at the
canonicalized current-directory capability and is descended one component at a
time; an absolute root must already be a lexical, symlink-free path. Every
component is opened with `open_dir_nofollow`. `ParentDir`, `RootDir`, and
platform-prefix components are rejected as invalid paths, and a component that is
a symlink (final or intermediate) fails closed. A rejected root cannot become
ambient authority. Within the walk, the walker excludes hidden, ignore-matched,
symlink, and non-regular entries before candidates are discovered, and every
candidate read (`crates/hanimo-core/src/search/filesystem.rs`) reopens the file
no-follow and re-checks that it is a regular file.

### 2.2 The MCP trusted base

The stdio MCP server (`crates/hanimo-find/src/mcp.rs`) captures its trusted base
once at startup as `current_dir().canonicalize()`. Each tool's optional `path`
argument is resolved by `resolve_target`, which accepts only a non-empty relative
path whose every component is a `Normal` component and joins it to the base
**lexically** — no filesystem probe, no symlink resolution, no reopening at the
boundary. Absolute paths, `..`, `.`, platform prefixes, and empty input are
rejected there; the joined path is then handed to the same component-by-component
no-follow core acquisition described above. Omitting `path` targets the base
itself. The boundary widens authority only downward into normal-component nested
directories of the trusted base.

### 2.3 The caller-authorized verify root

`hanimo find verify` separates the bundle's `root` field, which is **display
metadata only**, from the read capability. The caller supplies `--root` (default
`.`); the MCP `verify_evidence` tool supplies the resolved target. In both paths
the resolved root must exactly equal the bundle's recorded display `root`, and a
mismatch is refused before any source read. In `verify_evidence`, that mismatch
returns a structured error rather than a verification report. The verify root
opener (`crates/hanimo-core/src/verify/filesystem.rs`) rejects any `ParentDir`
component and opens the final path component with `open_dir_nofollow`, so a final
symlink is refused; it does not independently re-verify intermediate components
against symlinks the way the search opener does, which is why the caller must
supply a root it already trusts. Block locators inside the bundle are validated
by `path_from_bytes` (no empty path, no NUL, no leading/trailing slash, no empty
segment, no `.` or `..`, not absolute) and read with `read_nofollow`, which opens
each intermediate directory and the final file no-follow.

## 3. Attacker capabilities

The table states each capability, what happens, and whether it is in scope.
"In scope" means the tool is designed to detect or block it; "out of scope" means
it is outside what this version's mechanisms can protect against — stated so a
consumer does not assume a guarantee that is not there.

| Attacker capability | Behavior of the tool | In scope? |
| --- | --- | --- |
| **Tamper with source bytes only**, after a bundle was produced | `verify` reopens the cited source, recomputes digests, and reports drift; changed bytes no longer match the recorded `source_sha256`, and a missing source is reported as stale evidence (exit 4). | In scope — detected. |
| **Tamper with the bundle only** (edit blocks, verdict, or digest inconsistently) | Verification recomputes `bundle_sha256` and the deterministic critic rule. An inconsistent digest or a bundle promoted to accepted while carrying a `budget` skip is treated as forged (a critic-rejected bundle exits 1; an invalid or unsupported bundle exits 3; forged/stale evidence exits 4). | In scope — detected. |
| **Tamper with source + bundle + digest together**, rewriting all three into a new self-consistent artifact | `bundle_sha256` is an *unkeyed* checksum. A self-consistent rewrite recomputes cleanly and will verify against the rewritten source. **This is out of scope of the unkeyed digest.** Defense against this actor requires a separately protected digest, an append-only Claim ledger, or a signature — none of which exist in v0.1. | Out of scope — stated plainly. |
| **Symlink or path-traversal escape** (symlinked root component, `..`, absolute locator, platform-root, symlinked cited file) | Root and locator acquisition is component-by-component no-follow; parent, absolute, empty, NUL-containing, and dot-segment paths fail closed (exit 5 without opening the locator; MCP boundary returns invalid params). See §2. | In scope — mitigated. |
| **Exfiltrate secrets through evidence content** (induce a match inside a key, `.env`, or credential file) | `is_secret_like` skips dotfiles, `secrets`, `.env*`, `*.key`, `*.pem`, common private-key/keystore names (`id_rsa`, `id_dsa`, `id_ecdsa`, `id_ed25519`, `*.p12`, `*.pfx`, `*.ppk`, `*.jks`, `*.keystore`), and any path whose component contains `secret`, `token`, `credential`, `password`, `api_key`, or `apikey`. Such files become path-addressed typed gaps in `skipped` with reason `secret`; their bytes never enter a block. | In scope — mitigated by a fixed name/path policy. This is a filename/path heuristic, not content secret-scanning; a secret in an unmatched filename is not detected. |
| **RAG-style data poisoning of retrieved content** (plant misleading text so a downstream model treats it as fact) | Matching is byte-exact, so planted text can be returned verbatim as a block. The tool does not judge whether block content is true — it attaches provenance (`path:line`, `source_sha256`, `why[]`) so the consumer can locate and check it. **Block content is untrusted input the consumer must treat as data, not instructions.** | Out of scope for content truth; provenance is provided so the consumer can verify. |
| **Contaminate the MCP stdout protocol** with human-readable output | The MCP server communicates over JSON-RPC framing on stdio and must not mix diagnostics into stdout; see SECURITY.md and [docs/MCP.md](docs/MCP.md). | In scope — invariant. |
| **Exhaust resources** via a large query, tree, or bundle | Hard admission ceilings (query bytes, literals, candidate files, discovery entries/depth) and per-file and aggregate byte budgets bound work; a truncating budget forces critic rejection rather than presenting a partial result as sufficient, and over-envelope bundles are rejected. Exact ceilings are in [README.md](README.md) and [SPEC.md](SPEC.md). | In scope — bounded. |

### 3.1 On the `accepted` signal

The MCP `verify_evidence` tool returns `{accepted, report}`. `accepted` is true
only when verification status is `verified` **and** the bundle's critic verdict
is `accepted`, which is the same condition as CLI exit 0. It means: the recorded
attested bytes are unchanged, they still match the current source under the
authorized root, and the deterministic critic found the required literals
covered. It does **not** mean the content is correct, authorized, or authored by
a trusted party. A consumer that treats `accepted` as a truth or authorization
signal is relying on something the tool does not claim.

## 4. Non-goals

These are explicitly out of scope for v0.1. They are named so no consumer builds
a guarantee on top of them.

- **A compromised trust root.** If the machine, the current working directory the
  server or CLI is launched from, the checked-out source, or the toolchain is
  under attacker control, the tool's guarantees do not hold. Authority flows from
  the launch capability; a poisoned root produces faithfully attested evidence of
  poisoned bytes.
- **Authorship, identity, and authorization.** `bundle_sha256` is not a
  signature, trusted timestamp, proof of authorship, or authorization decision.
  v0.1 has no signing key, no ledger, and no identity binding. The roadmap's
  append-only Claim ledger and Ed25519-signed checkpoints ([ROADMAP.md](ROADMAP.md),
  v0.3) are where such an anchor would be introduced; until it exists, `Verified`
  means only "unchanged and matching the current source."
- **Semantic truth or recall.** The tool asserts that cited bytes exist at a
  location and match a digest — not that a statement is true, complete, or the
  best answer. Matching is byte-exact with no case folding, stemming, paraphrase,
  or Unicode normalization; NFC and NFD forms of the same text (for example
  Korean 조합형 vs. 완성형) are different bytes and do not match. There is no
  semantic-recall or paraphrase guarantee.
- **Absolute security or production readiness.** This is a source beta. It makes
  no production-readiness, absolute-security, or universal-performance claim.

## 5. Assurance foundation

The properties above rest on a small set of implementation constraints, not on
runtime configuration:

- Filesystem access goes through a capability-based API (`cap-std`); root and
  path components are opened no-follow.
- `unsafe` code is forbidden crate-wide, and lints deny `unwrap`, `expect`,
  `panic`, and slice indexing so untrusted input cannot trigger a panic through
  those paths.
- Governance is documented and honest about its limits: a solo maintainer, a
  documented `enforce_admins=false` bypass, a ruleset protecting `refs/tags/v*`,
  and enabled GitHub private vulnerability reporting. No version tag has been
  published yet; this is a source beta. See [SECURITY.md](SECURITY.md),
  [MAINTAINERS.md](MAINTAINERS.md), [PROVENANCE.md](PROVENANCE.md), and
  [RELEASE_GATE.md](RELEASE_GATE.md).
- Supply-chain artifacts (CycloneDX SBOM, `cargo-deny`, `cargo-about`,
  `gitleaks`) support review, but both crates set `publish = false`; the tool is
  not on crates.io and ships no prebuilt binaries yet.

Report suspected vulnerabilities through the process in [SECURITY.md](SECURITY.md).
Do not publish secret material, exploit payloads, or private repository contents
in a public issue.
