# Mini-benchmark results — Arm A (hanimo-find) vs Arm D (ripgrep)

> **Scope, stated up front.** This is a small sealed demonstration corpus (6
> files), not a generalizable study. The **structural** results below
> (determinism, verified citations, forgery rejection) are the honest headline
> and hold regardless of corpus size. The **span** results are illustrative
> only and are **not a claim of superiority** over ripgrep or any other system.
> Arms B/C (v0.2/v0.3, unimplemented) and the `hanimo-rag` / naive-RAG arms
> (external code + LLM) were **not run**. A generalizable comparison must follow
> the preregistration in [../BENCHMARK.md](../BENCHMARK.md).

Sealed at commit `2655912`. Corpus `sha256:5562034b…` (6 files). Arm A =
`hanimo find search … bench/corpus --format json`; Arm D = `ripgrep 15.2.0`
with the frozen invocation `rg -n --no-heading -F -e <term> .` and a
line-to-span adapter. 12 probes; regenerate with `python3 bench/run.py`.

## Structural properties — the measured moat

| Property | hanimo-find (Arm A) | ripgrep (Arm D) |
| --- | --- | --- |
| Determinism (distinct `bundle_sha256` over 3 runs) | **1** → deterministic | no digest emitted |
| Verified-citation rate (each cited block re-verifies against source) | **1.0** | **n/a** — emits no citation contract |
| Forged evidence (tamper one content byte, then `verify`) | **rejected, exit 4** | no verification step |

These are the properties ripgrep — and plain lexical search in general —
structurally cannot provide. They do not depend on the corpus.

## Span recall / precision — illustrative

Aggregated over the 8 exact-match probes and the 4 boundary probes:

| Probe group | Arm A recall | Arm A precision | Arm D recall | Arm D precision |
| --- | --- | --- | --- | --- |
| Exact / phrase / multilingual / boolean (8) | **1.00** | 0.31 | **1.00** | 1.00 |
| Byte-exact boundary — paraphrase, NFD, case (4) | **0.00** | — | **0.00** | — |

Reading these honestly:

- **Lexical recall is table stakes.** On every exact, phrase, multilingual, and
  boolean probe, hanimo-find and ripgrep both recall the gold spans. No-vector
  literal search is not a differentiator; ripgrep matches it.
- **hanimo-find trades line-precision for reviewability.** Its lower precision
  (0.31) is because each evidence block ships ±3 context lines around a hit,
  so a block covers lines that are not themselves the gold line. ripgrep
  returns bare match lines (precision 1.0). This is a design choice
  (reviewable context vs minimal output), not a defect — but it is real and is
  reported, not hidden.
- **The byte-exact boundary is quantified, not hidden.** On paraphrase
  (`rollback` for "undo … ship the previous build"), NFD-decomposed Korean
  (`배포`), and case (`deploy_region` vs `DEPLOY_REGION`), **both arms recall
  0.00**. hanimo-find claims no semantic, Unicode-normalization, or case-folding
  recall, and the benchmark measures exactly that miss instead of omitting it.

## Interpretation

On this corpus the two lexical arms are indistinguishable on what they retrieve.
The measured difference is entirely in the **evidence contract**: hanimo-find's
citations re-verify (1.0), a tampered citation is mechanically rejected (exit 4),
and repeated runs are byte-identical (deterministic digest). ripgrep provides
none of these. That is the thesis this project sells — no-vector search is not
the moat; a verifiable, deterministic evidence contract is — and here it is a
measured fact rather than a slogan.

## Limitations

- Six-file corpus; span numbers are illustrative, not generalizable.
- Not a latency benchmark. Per-probe timings are recorded in `results.json` for
  reference only; no speed claim is made from a corpus this small.
- Arms B, C, `hanimo-rag`, and naive-RAG were not run (unimplemented or external
  dependencies). Their absence is stated, not approximated.
- Precision is measured at line granularity; the ±3 context window is the main
  driver of Arm A's precision and would shrink on larger files.
