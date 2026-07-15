# Mini-benchmark

A small, reproducible benchmark of the arms that need no external code or LLM:

- **Arm A** — hanimo-find v0.1 exact-literal `EvidenceBundle` search.
- **Arm D** — ripgrep with a frozen invocation and a line-to-span adapter.

It measures the **structural properties** that are this project's actual moat
(determinism, machine-verifiable citations, forged-evidence rejection) and, as
illustrative context, span recall/precision across probe classes. It is **not**
a generalizable study: see [../BENCHMARK.md](../BENCHMARK.md) for the
preregistration protocol a real comparison — including `hanimo-rag`, naive-RAG,
and the v0.2/v0.3 arms — must follow.

## Layout

| Path | What it is |
| --- | --- |
| `corpus/` | Sealed demonstration corpus (English + Korean docs, config, Rust, Python). |
| `probes.json` | 12 probes with gold `[path, line]` spans; `expect_hit=false` marks the byte-exact boundary. |
| `manifest.json` | Preregistration unit: sealed commit, corpus digest, arms, metrics. |
| `run.py` | Harness: runs both arms, computes metrics, writes `results.json`. |
| `RESULTS.md` | The recorded, honestly-scoped results. |

## Run it

Build the binary and run the harness from the repository root:

```sh
cargo build --locked --release --bin hanimo
RG=$(command -v rg) HANIMO=target/release/hanimo python3 bench/run.py
```

Requires Python 3 and [ripgrep](https://github.com/BurntSushi/ripgrep) on
`PATH` (or set `RG`). The harness writes `bench/results.json` (git-ignored;
per-probe metrics and timings) and prints the structural summary.

## What the numbers mean

The structural results hold regardless of corpus size. The span results are
illustrative on a six-file corpus and are not a superiority claim — on exact
queries hanimo-find and ripgrep recall the same spans, and on paraphrase,
NFD-Unicode, and case-mismatch queries both miss, which the benchmark reports
rather than hides. The measured difference is the evidence contract, not
retrieval: see [RESULTS.md](RESULTS.md).
