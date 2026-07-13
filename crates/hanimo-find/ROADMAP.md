# Roadmap: verified evidence to Evidence-Compiled Wiki

This roadmap advances the current v0.1 source beta without weakening its
evidence boundary. Versions are capability milestones, not release-date
promises.

## Invariants across every milestone

- Original source bytes are authoritative; generated or compiled text is not.
- Raw sources are immutable, versioned inputs. A change creates a new
  digest-identified source version while every prior version remains
  addressable; the compiler never edits or overwrites raw source bytes.
- Every accepted evidence leaf has a root-relative path, byte/line address,
  source digest, and deterministic identity.
- Core search, verification, and rebuild remain usable offline without a vector
  database, embedding service, LLM, or daemon.
- Candidate eligibility and ranking are separate operations.
- Derived artifacts are reproducible, invalidatable, and schema-versioned.
- Optional model output may propose a query, claim, or edit but may not commit it
  without the same deterministic validation path used for human proposals.
- Security-sensitive trust is external to the v0.1 unkeyed bundle digest.

## v0.1 — deterministic evidence compiler (current)

The current surface scans a local root for exact byte literals, ranks bounded
evidence blocks deterministically, renders JSON/Markdown EvidenceBundles,
verifies cited source bytes, diagnoses versioned repository patterns, and
exposes `search_evidence` through MCP stdio.

Completion boundary: the v0.1 schema, exit codes, canonical serialization,
security skips, byte handling, and conformance fixtures remain frozen unless a
new schema version and migration rule are introduced.

## v0.2 — QueryPlan AST and eligibility semantics

Replace the flat query lists with a versioned typed expression tree:

```text
QueryPlanV2 {
  root,
  expression: And | Or | Not | Phrase | Term | Identifier | Path | Field,
  budget,
  schema_version
}
```

Decision rules:

- `Phrase`, `Term`, and `Identifier` match exact bytes under explicitly named
  case and normalization modes; v0.2 initially supports the v0.1 exact mode.
- `And`, `Or`, and `Not` determine candidate eligibility. `Not` only removes
  candidates and contributes no positive rank score.
- `Path` constrains normalized root-relative path bytes. It cannot escape the
  declared root or follow a symlink.
- `Field` names a registered deterministic extractor such as Markdown heading,
  file extension, or source path. Unknown fields are usage errors, not silently
  ignored hints.
- Ranking is applied only after eligibility and emits explainable integer score
  components with a stable tie-break order.
- Empty nodes, invalid arity, contradictory root selectors, unknown schema
  versions, and budgets outside the supported envelope are rejected.
- v0.1 plans can be read through an explicit adapter; v0.2 serialization never
  reinterprets an old plan in place.

Done when conformance tests cover nested Boolean logic, exclusion, exact
phrases, path/field filters, deterministic ordering, malformed ASTs, migration,
and critic coverage independently of rank.

## v0.3 — verified Claim ledger and trusted attestation

Add an append-only Claim ledger above EvidenceBundles. A Claim record contains:

- `claim_id` derived from canonical claim bytes and schema version;
- the canonical claim statement and optional structured predicate;
- referenced evidence block IDs and precise original-source spans;
- source SHA-256 and EvidenceBundle SHA-256 values;
- compiler/rule version, creation event, and supersession links;
- state: `active`, `stale`, `retracted`, or `replaced`.

Contradictory supported Claims are retained as parallel records. A versioned
`conflicts_with` relation links both sides, and the conflict state is
`unresolved` until an explicit policy or owner decision appends a resolution
event. Compilation, ranking, or model output must not silently merge the Claims,
choose a winner, delete either side, or rewrite their evidence. A resolution may
append replacement, retraction, or policy-selection transitions while preserving
the original Claims, relation, evidence, and decision history.

Only evidence that passes `verify` can activate a Claim. A source digest change
marks every dependent Claim stale before any downstream rebuild. Corrections
append a transition; they do not erase history.

Ledger events have a monotonic sequence number, the prior event SHA-256, and a
canonical event hash. A `ClaimCheckpointV1` covers `ledger_id`, latest sequence,
latest event hash, and issuance time. It is signed with Ed25519 over
domain-separated canonical checkpoint bytes; `key_id` is the SHA-256 of the raw
public key. Verification uses a local allowlist with validity windows and an
append-only revocation record. A Claim is `trusted` only when its evidence still
verifies, its latest state is active, it is included at or before a valid signed
checkpoint, and the checkpoint key is allowed and unrevoked under the verifier's
policy. Otherwise it is reported as verified-but-unattested, stale, retracted,
or invalid—never silently upgraded.

The v0.1 `bundle_sha256` remains an integrity checksum and is never relabeled as
a signature. This attestation format is a new versioned layer; it does not
reinterpret old bundles.

Done when mutation tests prove source drift, evidence forgery, ledger rewrite,
signature failure, revocation, and stale dependency propagation cannot yield an
active trusted Claim.

## v0.4 — Evidence-Compiled Wiki derived view

Materialize navigable pages from active Claims. Every factual sentence or
structured cell must resolve to one or more Claim IDs and, through them, to
verified original-source spans. Wiki text is disposable output: deleting it and
rebuilding from the same ledger must produce identical bytes.

There is exactly one mutation path:

```text
external append of an immutable source version
  -> search and verify evidence
  -> propose and validate Claim transition
  -> append ledger event and invalidate dependents
  -> rebuild affected wiki pages
```

Source creation and revision occur outside the compiler. The compiler can read
and register a new version but can never mutate, replace, or delete raw source
versions. Direct wiki edits cannot mutate source evidence or active Claims. A
human or model edit to a wiki page becomes a proposal that enters the same
path. If it cannot cite verified original spans, it remains an uncommitted
annotation. There is no second source, Claim, or wiki write path.

The dependency graph maps source digest -> evidence block -> Claim -> page and
section. Rebuild scope is the transitive affected set, with a full deterministic
rebuild available as the reference oracle.

Done when incremental and full rebuilds are byte-identical, every rendered
claim has a resolvable provenance chain, stale sources remove or visibly stale
dependent text, unresolved conflicts retain every supported side, raw source
versions remain immutable and addressable, and there is no second write path.

## v0.5 — held-out compile, evaluate, refine loop

Before compilation, partition versioned source-grounded probes into development
and held-out sets. The compiler may inspect development probes; held-out prompts
and expected original spans stay frozen until the evaluation run is sealed.

Each run records corpus and probe digests, compiler version, configuration,
failures, build/query/update cost, and all optional model/token details. Failure
classes include omitted fact, unsupported claim, contradicted claim, wrong
source, stale citation, over-broad span, navigation miss, and budget exhaustion.

Refinement changes a versioned compiler, query rule, or source annotation. It
must not patch a held-out answer or wiki sentence directly. After a refinement,
all prior probes are rerun to detect regressions and a new untouched holdout is
required before making a generalized claim.

Done when blind compilation has a recorded baseline, every iteration is
reproducible, leakage checks pass, and quality and cost deltas are reported with
failures rather than only aggregate scores.

## v1 — progressive structure navigation

Add deterministic trees for filesystem layout, document headings, and supported
language symbols. A query begins at a coarse node, expands explicit child
branches, and terminates only at canonical evidence leaves. Navigation traces
are serializable and replayable.

Optional semantic or LLM planners may propose branch expansions, but their
proposals are labeled, costed, and executed through the same QueryPlan and
verification boundary. The required core remains vectorless and model-free;
optional lanes are benchmark arms, not hidden dependencies.

Done when navigation preserves v0 evidence identity, never promotes summaries
to evidence, handles stale-tree invalidation, and demonstrates its tradeoffs on
the preregistered protocol in [BENCHMARK.md](BENCHMARK.md).

## Explicit non-goals

The roadmap does not promise autonomous truth determination, publication-ready
generation, universal semantic recall, immunity to a compromised trust root, or
superiority over every RAG/vector/long-context configuration. Those are distinct
claims requiring their own threat models and evidence.
