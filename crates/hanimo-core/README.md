# Hanimo Find

Hanimo Find is an evidence-first local source-search tool. Version 0.1 compiles
exact literal matches into deterministic evidence bundles that preserve source
paths, line ranges, raw byte offsets, source SHA-256 digests, and an
independently checkable bundle digest.

`imnotrag` is the name of the repository diagnostic/campaign exposed by
`hanimo find diagnose`; it is not the product or binary name.

This repository is a source beta. It is not a production release, an answer
engine, or a claim that lexical search is universally better than retrieval or
LLM-based systems.

## Current surface

The `hanimo` binary currently exposes four `find` subcommands:

| Command | Purpose |
| --- | --- |
| `hanimo find search` | Scan a local root for exact UTF-8 byte literals and emit an EvidenceBundle as JSON or Markdown. |
| `hanimo find verify` | Reopen the cited sources and reject invalid, stale, or forged bundle evidence. |
| `hanimo find diagnose` | Statically inspect a repository for the versioned `imnotrag` diagnostic rules. |
| `hanimo find mcp` | Serve the `search_evidence`, `verify_evidence`, and `diagnose_repo` tools over MCP stdio using `rmcp`. |

The v0.1 scanner is bounded. Its defaults select at most 8 evidence blocks,
read at most 1 MiB per file and 16 MiB in total, accept at most 1,000 literal
occurrences, and include 3 context lines on each side of a hit. Hard admission
ceilings additionally allow at most 4,096 UTF-8 query bytes, 64 exact literals,
4,096 aggregate typed-literal bytes, and 64 eligible candidate files. Query
ceilings are enforced before filesystem scanning; one-over-limit input is a
usage error (exit 2), never silently truncated. Before reading source content,
the actual ignore-aware walk streams unsorted and admits at most 256 yielded
entries through depth 64. Entry or depth overflow discards the entire observed
enumeration and returns one root-level `budget` gap with an empty path. Only a
complete within-envelope stream is globally sorted by canonical root-relative
raw path bytes before its candidate prefix is selected. The walker policy-excludes
hidden, ignore-matched, symlink, and non-regular entries before candidate
discovery; these exclusions are not enumerated in `skipped`.
Discovered secret-like and oversized regular files are path-addressed in
`skipped`. If a candidate, byte, or match budget truncates the ordered scan, the
bundle records `reason: "budget"`; the critic rejects it and the CLI exits 1.
Every supplied search-root component is opened no-follow; a final or
intermediate symlink is rejected with exit 5 before its target is opened.

Diagnosis independently admits at most 4,096 regular candidate files, reads at
most 1 MiB from one file and 16 MiB in total, and processes sorted sources one at
a time. It reopens every path beneath a root capability without following
symlinks. Exceeding a diagnosis limit fails closed with exit 5 and no partial
JSON result.

## Install

The project is a source beta: it is not published to crates.io and no version
has been tagged yet, so both crates set `publish = false` and installation is
from source. The minimum supported Rust version is **1.88.0**
(`rust-toolchain.toml` pins the stable channel).

Install straight from the repository with the locked dependency graph:

```sh
cargo install --git https://github.com/flykimjiwon/hanimo-find hanimo-find --locked
```

or build from a checkout:

```sh
cargo build --locked --release
```

Both produce one `hanimo` binary. The examples below assume `target/release`
is on `PATH` or the built binary has been installed as `hanimo`.

### Prebuilt binaries

Each tagged release attaches per-platform archives — for
`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, and
`x86_64-pc-windows-msvc` — each with a SHA-256 checksum, built by the release
workflow (`.github/workflows/release.yml`). No version has been tagged yet, so
build from source for now. When a release is published, verify a download
before use:

```sh
sha256sum -c hanimo-<version>-<target>.tar.gz.sha256
```

```sh
hanimo find --help
hanimo find search '"safe rollout" FEATURE_FLAG DEPLOY_REGION 배포' fixtures/multilingual --format json
hanimo find search 배포 fixtures/multilingual --format md
hanimo find verify conformance/evidence-bundle.accepted.json --root .
hanimo find diagnose fixtures/diagnose/positive-repo --format json
hanimo find diagnose . --format md
```

`search` and `diagnose` default to path `.` and format `json`. A search query is
parsed into quoted phrases, uppercase identifiers, and ordinary terms; each
unquoted token is trimmed to its alphanumeric/`_`/`-` core, so a token made only
of operator or punctuation bytes (for example `!=` or `->`) is dropped — quote it
(`'"!="'`) to search for it literally. Matching is byte-exact: case folding,
stemming, paraphrase matching, and Unicode normalization are not performed.

Because matching is byte-exact, Unicode-equivalent but byte-different text does
not match. In particular, NFC and NFD forms of the same characters — precomposed
(완성형) versus conjoining (조합형) Hangul, or accented Latin letters — are
different byte sequences and do not match each other. Normalize the query and the
corpus to the same form when you need them to compare equal.

Start the MCP stdio server with:

```sh
hanimo find mcp
```

Launch the server from the directory it is authorized to search. It captures
that canonical startup directory as its trusted base and serves three tools:
`search_evidence`, `verify_evidence`, and `diagnose_repo`. Each tool's optional
`path` argument is a relative nested directory beneath that base. The MCP
boundary accepts only normal relative components and joins them lexically
without resolving or reopening the request path; core search then acquires the
directory component by component without following symlinks. Absolute, parent,
dot, platform-prefix, unavailable, non-directory, and symlinked paths are
rejected. Omitting `path` targets the trusted base. `verify_evidence` accepts
the authoritative bundle JSON, requires the resolved target to equal the
bundle's recorded display root, and returns the live verification report with
an `accepted` flag equivalent to CLI exit 0. An MCP client should use normal
JSON-RPC framing and must not mix human-readable output into stdout. Client
configuration and per-tool contracts are documented in [docs/MCP.md](docs/MCP.md).

## Evidence and verification boundary

JSON is the authoritative interchange form. Markdown is a deterministic human
view. Evidence blocks contain root-relative paths, one-based inclusive line
ranges, zero-based byte offsets, canonical block content, matched literals, and
the exact source-file SHA-256 digest. Invalid UTF-8 bytes are represented as
base64 instead of replacement text.

`bundle_sha256` is an unkeyed integrity checksum. It detects accidental changes
and inconsistent bundle contents, but it is not a signature, proof of
authorship, authorization decision, or trusted timestamp. An actor able to
rewrite the source, bundle, and digest together is outside that checksum's
protection. See [SECURITY.md](SECURITY.md) for the complete boundary.

Verification recomputes the deterministic critic rule, so an accepted bundle
with a `budget` skip is forged even when its unkeyed checksum was recomputed.
An internally consistent critic-rejected bundle still emits its live integrity
report, but exits 1 rather than promoting verified bytes to accepted evidence.
The bundle's `root` is display metadata only. `verify --root` supplies the
caller-authorized read capability (default `.`), must exactly match that recorded
display value, rejects parent traversal and a final symlink, and is never selected
from artifact data.
The CLI reads at most 16 MiB of bundle JSON before deserialization. Runtime
ceilings admit at most 64 blocks, 65,536 skips, 4,096 nested list items, 16 MiB
per source file, 64 MiB in artifact-reported total bytes, and 128 MiB of actual
source reads across both verification passes. Over-envelope or over-limit
artifacts exit 3; absolute, empty, or root-traversing evidence locators exit 5
without opening the locator. A missing recorded source is stale evidence (exit
4); permission denial, symlink refusal, invalid file policy, and every other
source open/read failure are security/I/O failures (exit 5) with no JSON report.

## Exit status contract

| Exit | Meaning |
| ---: | --- |
| `0` | Accepted and verified. |
| `1` | No evidence or critic rejection. |
| `2` | Usage error. |
| `3` | Invalid bundle or unsupported schema. |
| `4` | Stale or forged evidence. |
| `5` | Scan, security, or I/O failure. |

## When to use Hanimo Find, and when not

Reach for Hanimo Find when a downstream consumer — a person, an audit, or an
agent — must independently re-check evidence without trusting the search step:
regulated or compliance contexts, agent tool-calls that cite sources, and reviews
where a citation must resolve to exact bytes.

Choose a different tool when you need interactive literal search (use ripgrep),
semantic or paraphrase recall over a large fuzzy corpus (use an embedding or
retrieval system), or generated answers (Hanimo Find emits evidence, not prose).

| Need | Hanimo Find | ripgrep | Vector RAG |
| --- | --- | --- | --- |
| Byte-exact literal match | yes | yes | approximate |
| Semantic / paraphrase recall | no | no | yes |
| Verifiable citation contract (re-check `path:line` and bytes) | yes | no | no |
| Independent post-hoc verification (`verify`) | yes | no | no |
| Deterministic, reproducible output | yes | yes, per invocation | typically no |
| Zero build, no index, always current | yes | yes | no, needs an index |
| Offline, no model required | yes | yes | usually no |
| Generates answers | no | no | yes |

See [FAQ.md](FAQ.md) for "Why not just ripgrep?" and "Is this anti-RAG?", and
[docs/CONSUMING_EVIDENCE.md](docs/CONSUMING_EVIDENCE.md) for how a downstream
consumer cites and re-verifies evidence.

## Deliberate limits

- v0.1 has no semantic-recall or paraphrase guarantee.
- It requires no vector database, embeddings, persistent semantic index, LLM,
  network service, or daemon.
- It emits evidence, not generated answers or publication-ready prose.
- Verification checks current local bytes and bundle consistency; it does not
  establish that a source is true or that its author is trusted.
- `skipped` enumerates discovered policy gaps, not hidden or ignore-matched
  walker exclusions. A metadata-envelope gap uses an empty path to identify the
  root-level truncation and is emitted before any candidate content is read.
- Exhausted byte or match budgets preserve already collected blocks, mark the
  omitted discovered suffix as budget gaps, and force critic rejection rather
  than presenting the partial result as sufficient.
- The project makes no production-readiness, absolute-security, or universal
  performance claim.

## Documentation

- [SPEC.md](SPEC.md) — the normative v0.1 contract.
- [docs/MCP.md](docs/MCP.md) — MCP client configuration and the per-tool contracts.
- [docs/CONSUMING_EVIDENCE.md](docs/CONSUMING_EVIDENCE.md) — how a downstream
  consumer cites and re-verifies evidence without citation theater.
- [SECURITY.md](SECURITY.md) and [THREAT_MODEL.md](THREAT_MODEL.md) — the trust
  boundary, invariants, and attacker-capability model.
- [docs/VERIFYING_RELEASES.md](docs/VERIFYING_RELEASES.md) — consumer-side
  verification of a checkout, its SBOM, and its license posture.
- [FAQ.md](FAQ.md) — positioning questions.
- [RESEARCH.md](RESEARCH.md) — research corrections and transferable ideas.
- [ROADMAP.md](ROADMAP.md) — the decision-complete advancement plan.
- [BENCHMARK.md](BENCHMARK.md) — the evaluation protocol.
- [CONTRIBUTING.md](CONTRIBUTING.md), [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md),
  and [MAINTAINERS.md](MAINTAINERS.md) — participation and governance.
