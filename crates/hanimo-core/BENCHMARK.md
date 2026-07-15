# Benchmark protocol

This protocol measures Hanimo Find and the planned Evidence-Compiled Wiki
against original source spans. It prevents a compiled artifact from grading
itself and separates quality from build, query, update, and verification cost.

A small, runnable demonstration of the arms that need no external code or LLM
(hanimo-find vs a ripgrep baseline) lives in
[`bench/`](https://github.com/flykimjiwon/hanimo-find/tree/main/bench). Its
structural results — determinism, verified citations, and forged-evidence
rejection — are real and corpus-independent; its span numbers are explicitly
illustrative, not a superiority claim. This protocol is what a generalizable
study, including the `hanimo-rag`, naive-RAG, and v0.2/v0.3 arms, must follow.

## Preregistration unit

Before running an experiment, seal a manifest containing:

- repository revision, corpus snapshot digest, file count, byte count, language
  mix, and license/provenance record;
- executable revision, schema/compiler version, configuration, dependency lock
  digest, hardware, operating system, filesystem, and cache state;
- query/probe IDs, split assignment, query class, expected original-source path
  plus byte/line spans, and adjudication policy;
- benchmark arms and all optional models, prompts, seeds, API versions, token
  prices, indexes, and stopping rules;
- primary metrics, aggregation rules, exclusion rules, and failure taxonomy.

Development and held-out probes are split before wiki/Claim compilation. A
probe's expected answer must be grounded in the sealed original corpus, not in a
compiled page or system output.

## Arms

At minimum, compare:

| Arm | Description |
| --- | --- |
| A | v0.1 exact lexical EvidenceBundle search. |
| B | Candidate v0.2 QueryPlan AST with eligibility separated from rank. |
| C | Candidate verified Claim ledger plus Evidence-Compiled Wiki. |
| D | A local literal baseline such as ripgrep with a frozen invocation and span adapter. |

Optional BM25, vector, PageIndex, Corpus2Skill, CAG, or model-agent arms are
allowed only when their full dependencies and configurations are available.
They are reported as separate systems; missing arms are not approximated or
described as defeated.

## Probe classes

Include exact identifiers, quoted phrases, Boolean conjunction/disjunction,
negative constraints, path and field filters, duplicate terms, multilingual
text, canonically equivalent but byte-different Unicode, invalid UTF-8 source
bytes, same-line and cross-line evidence, source mutation, and symlink/root
escape attempts.

Include paraphrase and fuzzy-association probes even though v0.1 is expected to
miss many of them. These probes quantify the lexical boundary instead of hiding
it. Multi-source synthesis probes must identify every original span needed for
the answer.

## Original-source grading

For each returned or cited claim, resolve the entire chain to the sealed source
snapshot. Grade:

- **span precision:** returned source bytes inside gold original spans divided
  by returned source bytes;
- **span recall:** gold original bytes covered by returned verified spans divided
  by gold original bytes;
- **evidence-set precision/recall:** exact source-span units returned versus the
  adjudicated required set;
- **verified citation rate:** cited claims whose path, byte range, content,
  source digest, block ID, and bundle digest verify;
- **claim support:** `supported`, `partial`, `contradicted`, `unsupported`, or
  `unverifiable`, judged against original source spans;
- **stale/forged acceptance rate:** mutated or fabricated evidence incorrectly
  accepted;
- **coverage disclosure:** skipped files, budget exhaustion, and critic
  rejections preserved in the reported denominator.

A wiki excerpt may help locate a Claim, but it is never the gold reference. A
claim supported by the wiki and unsupported by the original source is a
compilation failure. Human adjudication is blinded to system identity where
feasible; disagreements and the resolution record are retained. LLM judges may
be exploratory metrics only and must be calibrated against a human sample.

## Quality reporting

Report each probe class and corpus separately before any macro average. Include
exact counts and confidence intervals or paired bootstrap intervals where the
sample supports them. Publish all misses, unsupported claims, stale citations,
security failures, and excluded cases with machine-readable reason codes.

No claim of superiority is made from a single small corpus, an unpaired setup,
or an author-reported external number. The preregistered compiled-wiki study in
[RESEARCH.md](RESEARCH.md) is evidence for evaluation design, not a Hanimo Find
result.

## Cost accounting

Do not collapse the following phases into one headline number.

### Build

Record wall time, CPU time, peak resident memory, bytes read/written, artifact
size, source files processed/skipped, embedding work, LLM input/output/cache
tokens, API cost, and retry/failure count. A zero-build arm records zero rather
than amortizing another arm's preparation away.

### Query

Record cold and warm wall time, TTFT when generation exists, total response
time, generation throughput, CPU time, peak memory, bytes read, candidates
examined, evidence bytes returned, model tokens, API cost, and request count.
Report median and p95 with the number of repetitions and concurrency.

### Update

Apply sealed edits representing add, modify, rename, delete, and source-digest
drift. Record changed source bytes, files rescanned, evidence/Claims/pages
invalidated, rebuild wall/CPU time, bytes read/written, model tokens/cost, and
whether incremental output equals a clean full rebuild.

### Verification

Record bundles/blocks checked, source bytes reopened, wall/CPU time, and failure
class. Verification is not counted as free merely because it is required.

WiCER's `7.3x` figure is TTFT, while that paper reports separate total-response
and throughput results. CAG removes its online retrieval step under a bounded
context assumption, but its published timing table does not establish universal
end-to-end latency against every RAG system. Hanimo Find reports these axes
separately for the same reason.

## Reproducibility and stopping

Use a fresh process for cold runs and a declared cache policy for warm runs.
Randomized arms publish seeds and run order; deterministic arms must reproduce
byte-identical artifacts. Stop only by the preregistered sample/repetition rule,
not when a favorable result appears. Any post-hoc probe, metric, arm, or
refinement is labeled exploratory and cannot replace the sealed primary result.
