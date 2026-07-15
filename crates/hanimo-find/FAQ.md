# FAQ

Plain-language positioning answers. Nothing here overrides the normative
contract in [SPEC.md](SPEC.md), the security boundary in [SECURITY.md](SECURITY.md),
or the product overview in [README.md](README.md). Where an answer and a normative
document disagree, the normative document wins.

## Why not just ripgrep?

Use ripgrep. It is a fast, correct literal search tool and a legitimate
baseline — the benchmark protocol lists a frozen ripgrep-style invocation as
comparison arm D (see [BENCHMARK.md](BENCHMARK.md)).

Hanimo Find is not competing on match speed. It adds a structured contract
*around* matching:

- **An evidence contract, not a hit list.** `hanimo find search` emits an
  EvidenceBundle: each block carries a root-relative path, a one-based inclusive
  line range, zero-based byte offsets, the canonical block content, the matched
  literals, and the exact source-file SHA-256 digest. Invalid UTF-8 is rendered
  as base64, never replacement text.
- **Independent verification.** `hanimo find verify` reopens the cited sources
  and rejects invalid, stale, or forged bundles. It is a separate command with
  its own exit codes, so a bundle can be checked by a party who did not produce
  it.
- **A critic gate.** If a candidate, byte, or match budget truncates the ordered
  scan, the bundle records a `budget` gap, the critic rejects it, and the CLI
  exits `1` rather than presenting a partial result as sufficient.
- **A reproducible bundle digest.** `bundle_sha256` lets two people confirm they
  are looking at byte-identical evidence. (See the limits below — it is an
  integrity checksum, not a signature.)
- **Typed security skips.** Secret-like and oversized files are recorded as
  path-addressed typed gaps in `skipped`, never silently included as content.

If you only need to find a string, ripgrep is the right tool. Hanimo Find is for
when a downstream consumer must be able to re-check the evidence without trusting
the search step.

## Is this anti-RAG?

No. Hanimo Find is an evidence compiler, not a claim that retrieval,
vectors, or LLMs are bad.

[RESEARCH.md](RESEARCH.md) states explicitly that the design "does not assume
that RAG is always bad, that vectors are always unnecessary, or that an LLM can
never help. It keeps those mechanisms optional and measurable so the offline
deterministic evidence contract does not depend on them." The roadmap keeps
optional model and vector lanes as labeled, costed benchmark arms rather than
hidden dependencies (see [ROADMAP.md](ROADMAP.md)).

The `rag` term appears in the package keywords for discoverability, and
`imnotrag` names the repository diagnostic — neither is a claim that RAG is
inferior.

## Why byte-exact matching? Why no fuzzy search?

Determinism and verifiability come first. Matching in v0.1 is byte-exact:
case folding, stemming, paraphrase matching, and Unicode normalization are not
performed. NFC and NFD forms of the same text (for example, Korean 조합형 versus
완성형) are different bytes and do not match.

This is a deliberate boundary, not an oversight. Exact bytes are what let a
bundle be re-derived and independently verified against the current source. v0.1
carries no semantic-recall or paraphrase guarantee, and the benchmark protocol
deliberately includes paraphrase and fuzzy-association probes to *quantify* that
lexical boundary instead of hiding it.

The roadmap's v0.2 QueryPlan AST introduces explicitly named case and
normalization modes, with v0.2 initially supporting the v0.1 exact mode (see
[ROADMAP.md](ROADMAP.md)). Any such mode is opt-in and named, so a query's
matching semantics stay explicit.

## What is `imnotrag` versus Hanimo Find?

**Hanimo Find** is the product. The binary is `hanimo`, and it exposes four
`find` subcommands: `search`, `verify`, `diagnose`, and `mcp`.

**`imnotrag`** is the name of the repository diagnostic and campaign exposed by
`hanimo find diagnose`. It is not the product or binary name. `diagnose`
statically inspects a repository for the versioned `imnotrag` diagnostic rules.

## Does it send my code anywhere?

No. Core search, verification, and diagnosis run locally. Hanimo Find
requires no vector database, embeddings, persistent semantic index, LLM, network
service, or daemon.

The one optional server, started with `hanimo find mcp`, is a local MCP stdio
server. It does not phone home: it captures its canonical startup directory as a
trusted base and serves three tools — `search_evidence`, `verify_evidence`, and
`diagnose_repo` — over stdio. A tool's optional `path` argument may only name a
relative directory beneath that base; absolute, parent, and symlinked paths are
rejected, and the filesystem is opened component by component without following
symlinks. See [docs/MCP.md](docs/MCP.md) and [SECURITY.md](SECURITY.md).

## Is it production ready?

No. This repository is a source beta. It is not a production release, an
answer engine, or a claim that lexical search is universally better than
retrieval or LLM-based systems.

Concretely: both crates set `publish = false`, so it is not on crates.io, and no
prebuilt binaries are attached to releases yet — installation is from source. The
project makes no production-readiness, absolute-security, or universal
performance claim.

## What does `bundle_sha256` actually prove?

It proves integrity and self-consistency, and nothing more.
`bundle_sha256` is an unkeyed integrity checksum. It detects accidental changes
and inconsistent bundle contents, but it is not a signature, proof of authorship,
authorization decision, or trusted timestamp.

An actor who can rewrite the source, the bundle, and the digest together is
outside that checksum's protection. Trust against that actor requires a
separately protected digest, an append-only Claim ledger, or a signature — the
roadmap's v0.3 milestone (append-only Claim ledger with Ed25519-signed
checkpoints) is where that trust anchor is planned. Until then, `Verified` means
only that the stored payload is unchanged and matches the current source. See
[SECURITY.md](SECURITY.md).

## What do the exit codes mean?

| Exit | Meaning |
| ---: | --- |
| `0` | Accepted and verified. |
| `1` | No evidence or critic rejection. |
| `2` | Usage error. |
| `3` | Invalid bundle or unsupported schema. |
| `4` | Stale or forged evidence. |
| `5` | Scan, security, or I/O failure. |

The full contract, including the verification boundary, is in
[README.md](README.md) and [SPEC.md](SPEC.md).
