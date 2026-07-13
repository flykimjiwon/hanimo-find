# Research notes and corrections

This document records the external ideas considered for Hanimo Find and the
limits that must travel with them. Sources were checked on 2026-07-14. Numeric
results below are author-reported results from the named setup, not Hanimo Find
benchmarks and not general performance claims.

## Primary-source findings

| Source | Transferable idea | Dependency or measurement boundary | Consequence for Hanimo Find |
| --- | --- | --- | --- |
| [LogicalRAG](https://arxiv.org/abs/2605.27123) | Let a planner express Boolean and phrase constraints while the retrieval layer executes those constraints faithfully; keep candidate eligibility separate from BM25 ranking. | The paper's experiments use OpenSearch as the sparse engine and Qwen3.5-Plus as the agent and judge. Reported comparisons are against the paper's controlled baselines, not a proof that logical lexical retrieval always wins. | Adopt a versioned deterministic QueryPlan AST in v0.2, without making OpenSearch or an LLM mandatory. |
| [Corpus2Skill paper](https://arxiv.org/abs/2604.14572) and [official implementation](https://github.com/dukesun99/Corpus2Skill) | Compile a corpus into an explicit hierarchy, then navigate from coarse topics to source documents. | The implementation embeds and clusters documents at compile time, uses an LLM to summarize and label the tree, and uses an LLM agent at serving time. “No retrieval system” therefore does not mean embedding-free or LLM-free end to end. | Treat hierarchy as an optional future derived layer; preserve a deterministic non-LLM evidence core. |
| [PageIndex](https://github.com/VectifyAI/PageIndex) | Tree navigation can preserve document structure and let a query progressively narrow to relevant leaves. | Its standard workflow builds and searches the tree with an LLM and asks for a model API key; “vectorless” is not the same as model-free. | v1 may add deterministic filesystem, heading, and symbol trees. Optional model proposals cannot become evidence without leaf verification. |
| [WiCER](https://arxiv.org/abs/2605.07068) | Compile, evaluate with diagnostic probes, and refine when compilation drops facts. | The reported `7.3x` is warm-cache time to first token in one curated-corpus comparison. The paper separately reports total-response latency and generation throughput, and also finds full-context quality below RAG at larger scale plus severe blind-compilation loss. | Freeze held-out probes before compilation, measure TTFT and total latency separately, and never accept a blind wiki build as faithful by construction. |
| [Vector RAG vs LLM-Compiled Wiki](https://arxiv.org/abs/2605.18490) | Organization, synthesis, claim support, and cost are separate capabilities. | The preregistered study covers 13 questions over 24 papers. Its exploratory wiki citation check grades claims against cited wiki excerpts, which are compiled artifacts, rather than checking those excerpts against original PDF passages. | Grade every wiki claim against canonical original-source spans; a compiled page is never benchmark ground truth. |
| [AnnoRetrieve](https://arxiv.org/abs/2604.02690) | Structured annotations and executable constraints can reduce query-time work. | The method replaces vector matching with induced schemas but reports non-zero LLM token cost and an offline annotation/schema pipeline. It reduces LLM use; it is not evidence of a zero-build, zero-index, zero-LLM system. | Borrow explicit fields and constraints only after their deterministic semantics and build/update costs are specified. |
| [SUQL paper](https://arxiv.org/abs/2311.09818) and [official implementation](https://github.com/stanford-oval/suql) | A formal query language can compose structured filters with free-text operations. | SUQL includes `SUMMARY` and `ANSWER` primitives and uses an in-context-learning LLM semantic parser. It is not a model-free local-search implementation. | Keep v0.2's grammar smaller, typed, local, and executable without natural-language-to-query translation. |
| [STORM](https://github.com/stanford-oval/storm) | Multi-stage research and outline construction can help pre-writing and knowledge curation. | The official repository says generated articles are not publication-ready and often require significant editing. | Generated synthesis, if ever added, must remain downstream of verified evidence and carry explicit review status. |
| [Cache-Augmented Generation](https://arxiv.org/abs/2412.15605) | A stable, manageable corpus can amortize context prefill with a KV cache. | The paper limits the proposal to corpora that fit a model context. Its latency table compares cached with uncached full-context generation; it does not establish universal end-to-end superiority over RAG across dynamic or large corpora. | Record build, cache, update, TTFT, total-response, token, and memory costs separately; do not headline retrieval-step savings as total-system savings. |

## Synthesis for the roadmap

The compatible direction is an evidence compiler with optional derived
structure:

1. Canonical evidence remains raw-byte-addressed and independently verifiable.
2. A typed QueryPlan determines candidate eligibility before deterministic
   ranking.
3. A Claim ledger may refer only to verified evidence IDs and source digests.
4. An Evidence-Compiled Wiki is a rebuildable view over Claims, never source
   authority.
5. Source digest drift invalidates dependent Claims and wiki materializations.
6. Held-out probes evaluate compilation against original source spans and drive
   versioned compiler improvements.
7. Progressive navigation may reduce search work, but every accepted leaf still
   resolves to canonical evidence.

This design does not assume that RAG is always bad, that vectors are always
unnecessary, or that an LLM can never help. It keeps those mechanisms optional
and measurable so the offline deterministic evidence contract does not depend
on them.
