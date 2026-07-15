# Consuming evidence without citation theater

This guide is for a downstream consumer — an LLM, an agent harness, or any
program — that reads Hanimo Find `EvidenceBundle`s and uses them to answer,
edit, review, or publish. It describes how to read a bundle, what a citation is
allowed to contain, and how to re-verify before trusting the result.

"Citation theater" is the failure this tool is built to prevent: prose that
*looks* cited — footnotes, quotation marks, a plausible file path — but whose
quoted text does not actually exist at the claimed location. A consumer that
paraphrases evidence and then attaches a path is producing citation theater
even when the path is real. The contract below removes that possibility by
making the citation itself checkable byte-for-byte.

Nothing here weakens or restates the normative v0.1 contract. The authoritative
definitions live in [SPEC.md](../SPEC.md); the trust boundary is in
[SECURITY.md](../SECURITY.md); the MCP tool contracts are in
[MCP.md](MCP.md).

## What Hanimo Find gives a consumer, and what it does not

An `EvidenceBundle` is a deterministic record of exact literal matches in local
source. It preserves, per block, the root-relative path, the one-based
inclusive line range, the zero-based byte offsets, the canonical block content,
the query literals present in that content, an explainable score, and two
digests. `hanimo find verify` (or `verify_evidence` over MCP) can later reopen
the cited sources and confirm the stored bytes still match.

It does **not** generate answers, summaries, or publication-ready prose; it does
not establish that a source is *true* or that its author is trusted; and it does
not perform semantic, paraphrase, or fuzzy matching. Matching is byte-exact: no
case folding, no stemming, no Unicode normalization. NFC and NFD forms of the
same text (for example Korean 조합형 vs. 완성형) are different bytes and do not
match. Treat the bundle as *retrieved evidence you must still reason over*, not
as a conclusion.

## EvidenceBundle anatomy from a consumer's view

JSON is the authoritative interchange form. Markdown is a **deterministic view**
of that same JSON — `hanimo find search --format md` wraps the exact
pretty-printed authoritative JSON in a fenced code block under a fixed heading,
adding no data of its own. Parse the JSON; never scrape the Markdown as if it
were a separate data model.

Top-level fields (all required; see
[`schema/evidence-bundle.schema.json`](../schema/evidence-bundle.schema.json)):

| Field | Type | What a consumer does with it |
| --- | --- | --- |
| `schema_version` | string, `"0.1.0"` | Reject anything you do not recognize. Only `0.1.0` is frozen. |
| `bundle_sha256` | 64 hex chars | An **unkeyed** integrity checksum over the immutable payload. Use it to detect accidental corruption or an inconsistent bundle. It is **not** a signature (see below). |
| `query` | string | The original query text, for display and audit. |
| `root` | string | **Display metadata only.** It records what root the producer named. It does not grant or select filesystem access, and you must not treat it as a trusted path. |
| `budget` | object | The resource limits used to produce this bundle (`max_blocks`, `context_lines`, `max_file_bytes`, `max_total_bytes`, `max_matches`). Useful context for *why* a scan may have truncated. |
| `blocks[]` | array | The evidence. Each element is one citable block (below). |
| `skipped[]` | array | Deterministically reported source gaps — path-addressed, never evidence content (below). |
| `critic` | object | The sufficiency decision. **Gate on `critic.verdict`.** |

### A block (the citable unit)

Each element of `blocks[]` carries:

| Field | Meaning for a consumer |
| --- | --- |
| `path` | Root-relative path as lossless encoded bytes (see encoding note). This is where the content lives. |
| `line_start`, `line_end` | One-based **inclusive** line range of the block. |
| `byte_start`, `byte_end` | Zero-based byte offsets; `byte_end` is one past the last selected content byte. |
| `content` | The canonical (LF-normalized) block bytes, encoded losslessly. This is the *only* text you are allowed to quote. |
| `matched_terms` | The query literals actually present in `content`, in query-plan order. |
| `score`, `score_components` | An explainable integer score and its named parts (`exact_phrase`, `identifier`, `all_terms`, `heading`, `path`, `proximity`). `score` equals the sum of the components. |
| `why` | Stable human-readable reasons for the score, for display. |
| `block_id` | `sha256:` + 64 hex — a domain-separated digest binding `path`, the line range, and `content` together. This is the stable citation handle. |
| `source_sha256` | The SHA-256 of the exact, complete raw source file the block was drawn from. |

**Encoding note.** `path` and `content` are `EncodedBytes`: either
`{"encoding":"utf8","text":...}` or `{"encoding":"base64","bytes":...}`. Invalid
UTF-8 is represented as base64, never as a replacement character. A consumer
that renders content must decode this envelope; do not assume `text` is always
present.

### `skipped[]` — typed gaps, not content

`skipped[]` reports *discovered* source gaps by path and a typed `reason`
(`secret`, `oversized`, `budget`, `non_regular`, and other frozen variants). A
skip is an honest statement that something exists but was deliberately not read.
Secret-like files (dotfiles, `.env*`, `*.key`, `*.pem`, private-key files,
paths containing `secret`/`token`/`credential`/`password`/`api_key`, and
similar) are **path-addressed here and their bytes never appear as evidence** —
so a bundle can tell you a credential file exists without ever disclosing it.
A `skipped` entry is metadata about the search, not something to quote.

### `critic` — the sufficiency gate

```json
"critic": {
  "verdict": "accepted" | "rejected",
  "covered_quoted_phrases": [...],
  "covered_identifiers": [...],
  "uncovered": [...]
}
```

The critic is **accepted iff** all three hold: at least one block exists,
`uncovered` is empty (every required quoted phrase and identifier is covered by
some block), **and** no `skipped` entry has `reason: "budget"`. Any other state
is `rejected`. This exact rule is recomputed at verification time, so you cannot
launder a hand-edited `verdict`.

## The citation contract

When you cite evidence to a user, to another tool, or into a published document,
a citation **is** the tuple:

```
block_id + path + line_start–line_end + source_sha256
```

and, if you quote text, the quoted text must be an exact substring of that
block's decoded `content`.

**Cite the block, never restyled prose.** Do not paraphrase the content and
attach a path. Do not "clean up" or summarize a line and present it as a quote.
Do not merge two blocks' text into one quotation. If you need to explain the
evidence in your own words, keep that explanation clearly separate from the
citation and do not let it inherit the citation's authority.

Why this is the whole point: `block_id` is a digest over `path`, the line range,
and the exact `content`. A later `verify` reconstructs `block_id` from the live
file and rejects the block if any of those inputs changed. So a citation that
names `block_id` + `path` + line range + `source_sha256` is a claim a machine
can re-check. A paraphrase is not — it has no verifiable anchor, and that is
exactly the citation theater this tool exists to eliminate.

## The re-verify loop

Never trust or publish a citation on the strength of the search call alone.
`search` tells you what matched *at scan time*; files change, and a bundle can
be hand-forged. Close the loop:

1. **search** — `hanimo find search '<query>' <path> --format json`
   (or `search_evidence` over MCP). Hold the returned bundle. Cite only
   `block_id`, `path`, line ranges, and `source_sha256`.
2. **act** — answer, edit, or review using the block content.
3. **verify** — `hanimo find verify <bundle.json> --root <root>`
   (or `verify_evidence` with the original bundle JSON). Require the accepted
   condition **before** trusting or publishing.

The accepted condition is a single gate:

- **CLI:** exit code `0`.
- **MCP:** `verify_evidence` returns `{"accepted": true, "report": {...}}`, and
  `accepted` is `true` **only** when the live status is `verified` *and* the
  bundle's `critic.verdict` is `accepted` — the exact equivalent of CLI exit 0.

`verify` reopens every cited source under a capability-scoped, no-follow root,
recomputes each `block_id` and `source_sha256` from the live bytes, and
independently recomputes the critic rule. `--root` supplies the read capability
and must exactly equal the bundle's recorded display `root`; it is never taken
from artifact data, and it rejects parent traversal and a final symlink. Over
MCP, the resolved target must likewise equal the bundle's recorded display root.

### Exit / status contract to branch on

| Exit | Meaning | Consumer action |
| ---: | --- | --- |
| `0` | Accepted and verified. | Safe to trust/publish the citation. |
| `1` | No evidence, or critic rejection. | Do not publish. The evidence is insufficient. |
| `2` | Usage error. | Fix the invocation. |
| `3` | Invalid bundle or unsupported schema. | Do not trust; the artifact is malformed or a future schema. |
| `4` | Stale or forged evidence. | The citation died — a source is missing (stale) or the bundle is internally inconsistent (forged). Discard it. |
| `5` | Scan, security, or I/O failure. | Fail closed; do not guess. |

Over MCP, `stale`, `forged`, and `source_drift` outcomes are returned **in-band**
with `accepted: false` and a `report`, so an agent can see *why* its evidence
died rather than only that the call failed. Malformed or schema-unsupported
bundles and root mismatches surface as errors, not as `accepted: false`.

## Why a `budget` skip forces rejection even when the bytes still match

This is the subtle case a consumer must respect. Suppose a scan hit a candidate,
byte, or match budget and could not read the entire discovered set. The blocks
it *did* collect are real, their bytes verify, and `uncovered` may even be empty
— every required literal appears in some block it managed to read. It is still
`rejected`, because the bundle also carries a `skipped` entry with
`reason: "budget"`.

The reason is that **partial is not sufficient**. A `budget` skip means the tool
knows it stopped early and did not observe the full candidate set. Presenting
that as sufficient evidence would be an unverifiable completeness claim — the
tool cannot promise the omitted suffix contained nothing more relevant. So the
critic refuses to call it accepted, and the verifier recomputes that same rule.
A hand-edited bundle that flips `verdict` to `accepted` while keeping a `budget`
skip is detected as **forged** and returns exit 4.

Concretely, this bundle has real, byte-matching blocks and empty `uncovered`,
yet is correctly `rejected` (from `conformance/evidence-bundle.budget-rejected.json`):

```json
"blocks": [ /* two real, verifiable blocks */ ],
"skipped": [ { "path": { "encoding": "utf8", "text": "omitted.txt" }, "reason": "budget" } ],
"critic": { "verdict": "rejected", "covered_quoted_phrases": [...], "covered_identifiers": [...], "uncovered": [] }
```

A consumer that only checked `uncovered` would wrongly treat this as complete.
Gate on `critic.verdict` and on the verify exit code — not on `uncovered` alone.

## A worked example

Search a checkout for a phrase, an identifier, and a Korean term. Using the
project fixtures:

```sh
hanimo find search 'Find "safe rollout" FEATURE_FLAG DEPLOY_REGION 배포' \
  fixtures/multilingual --format json > bundle.json
```

An accepted bundle looks like this (abridged from
`conformance/evidence-bundle.accepted.json`; field names are exact):

```json
{
  "schema_version": "0.1.0",
  "bundle_sha256": "3600642768008bbbb0c555c4652f4b9086702dd09fa1e762ab33ad3dc1e57580",
  "query": "Find \"safe rollout\" FEATURE_FLAG DEPLOY_REGION 배포",
  "root": ".",
  "budget": { "max_blocks": 8, "context_lines": 3, "max_file_bytes": 1048576, "max_total_bytes": 16777216, "max_matches": 1000 },
  "blocks": [
    {
      "path": { "encoding": "utf8", "text": "fixtures/multilingual/config/feature-flags.txt" },
      "line_start": 1,
      "line_end": 3,
      "byte_start": 0,
      "byte_end": 120,
      "content": { "encoding": "utf8", "text": "FEATURE_FLAG enables safe rollout.\n기능 플래그는 단계적 배포를 지원합니다.\nDEPLOY_REGION=ap-northeast-2" },
      "matched_terms": ["safe rollout", "FEATURE_FLAG", "DEPLOY_REGION", "배포"],
      "score": 780,
      "score_components": { "exact_phrase": 300, "identifier": 250, "all_terms": 150, "heading": 0, "path": 50, "proximity": 30 },
      "why": ["exact phrase", "identifier", "all terms", "path match", "proximity"],
      "block_id": "sha256:f84129bf3ddb191fd4315317a34fcf684121e3915f41c98ce2e59d697fd4e0bd",
      "source_sha256": "84dda1450952dcb7c8221d843610a4b443b37c8a668eaade0b5c20fadd19bb65"
    }
  ],
  "skipped": [],
  "critic": {
    "verdict": "accepted",
    "covered_quoted_phrases": ["safe rollout"],
    "covered_identifiers": ["FEATURE_FLAG", "DEPLOY_REGION"],
    "uncovered": []
  }
}
```

A **correct** citation derived from this bundle:

> `FEATURE_FLAG enables safe rollout.` —
> `fixtures/multilingual/config/feature-flags.txt`, lines 1–3
> (`block_id` `sha256:f84129bf…`, `source_sha256` `84dda145…`)

The quoted sentence is an exact substring of the block's decoded `content`. The
path, line range, and both digests come straight from the block.

Now close the loop before publishing:

```sh
hanimo find verify bundle.json --root fixtures/multilingual
echo "exit: $?"
```

`--root` must equal the bundle's recorded `root`. If the command exits `0`, the
live sources still reproduce every cited `block_id` and `source_sha256` and the
critic still accepts — trust it. If a source changed, a block would come back
`source_drift` or the file would be `stale`, and the command would exit `4`;
discard the citation and re-search.

Over MCP the same loop is `search_evidence` → hold bundle → `verify_evidence`
with `bundle_json` set to that exact JSON → require `accepted: true`.

## What a consumer must NOT do

- **Do not quote prose the bundle does not contain.** Every quotation must be an
  exact substring of a block's decoded `content`. Paraphrase-plus-path is
  citation theater and defeats the entire tool.
- **Do not treat `bundle_sha256` as a signature.** It is an unkeyed integrity
  checksum. It proves the payload is internally self-consistent and unmodified
  by accident; it proves nothing about authorship, authorization, or time, and
  it gives you no protection against an actor who rewrites the source, the
  bundle, and the digest together. Trust against that actor needs a separately
  protected digest, an append-only ledger, or a real signature — none of which
  v0.1 provides.
- **Do not ignore `critic.verdict`.** A `rejected` bundle is insufficient
  evidence, even when its blocks individually verify and `uncovered` is empty.
- **Do not promote a rejected bundle.** Do not "accept" a bundle by editing its
  `verdict`, dropping its `budget` skip, or otherwise reshaping it. Verification
  recomputes the critic rule and the block identities; a tampered bundle returns
  `forged` (exit 4), not accepted.
- **Do not skip re-verification.** A bundle that verified yesterday can be stale
  today. Require exit `0` / `accepted: true` at the moment you act.
- **Do not treat `root` as a filesystem capability.** It is display metadata.
  The read capability is the `--root` you pass to `verify` (or the trusted base
  the MCP server was launched in), and it must match the recorded `root`.
- **Do not scrape the Markdown view as a separate model.** The Markdown embeds
  the exact authoritative JSON; parse the JSON.

## See also

- [SPEC.md](../SPEC.md) — the normative v0.1 contract.
- [SECURITY.md](../SECURITY.md) — the trust and verification boundary,
  including the `bundle_sha256` limits and fail-closed rules.
- [MCP.md](MCP.md) — `search_evidence`, `verify_evidence`, and `diagnose_repo`
  tool contracts and the evidence loop over MCP.
- [`schema/evidence-bundle.schema.json`](../schema/evidence-bundle.schema.json)
  — the machine-readable bundle schema.
