#!/usr/bin/env python3
"""Hanimo Find mini-benchmark: Arm A (hanimo-find) vs Arm D (ripgrep baseline).

Runs the deterministic arms that require no external code or LLM, and reports
the structural properties that are the honest headline: determinism (identical
bundle digest across repeated runs), machine-verifiable citations, and forged
evidence rejection. Span recall/precision on this small sealed corpus are
reported as illustrative only, not a superiority claim; see ../BENCHMARK.md for
the preregistration a real study requires.

Usage:
    HANIMO=target/<triple>/release/hanimo python3 bench/run.py

Environment:
    HANIMO   path to the hanimo binary (default: search target/*/release/hanimo)
    RG       path to ripgrep (default: shutil.which("rg"))
"""

from __future__ import annotations

import glob
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unicodedata

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
CORPUS_REL = "bench/corpus"
CORPUS_ABS = os.path.join(REPO, CORPUS_REL)


def find_hanimo() -> str:
    env = os.environ.get("HANIMO")
    if env and os.path.exists(env):
        return env
    hits = sorted(glob.glob(os.path.join(REPO, "target", "*", "release", "hanimo")))
    hits += sorted(glob.glob(os.path.join(REPO, "target", "release", "hanimo")))
    if not hits:
        sys.exit("hanimo binary not found; build with `cargo build --release --bin hanimo` or set HANIMO")
    return hits[0]


def find_rg() -> str:
    env = os.environ.get("RG")
    if env and os.path.exists(env):
        return env
    rg = shutil.which("rg")
    if not rg:
        sys.exit("ripgrep (rg) not found on PATH; set RG")
    return rg


def corpus_files() -> list[str]:
    out = []
    for root, _dirs, files in os.walk(CORPUS_ABS):
        for name in files:
            out.append(os.path.relpath(os.path.join(root, name), CORPUS_ABS))
    return sorted(out)


def corpus_digest() -> str:
    h = hashlib.sha256()
    for rel in corpus_files():
        h.update(rel.encode("utf-8"))
        h.update(b"\0")
        with open(os.path.join(CORPUS_ABS, rel), "rb") as f:
            h.update(hashlib.sha256(f.read()).digest())
    return h.hexdigest()


def decode_path(encoded: dict) -> str | None:
    if encoded.get("encoding") == "utf8":
        return encoded.get("text")
    return None  # base64 path: not used by this corpus


def run_hanimo_search(hanimo: str, query: str) -> tuple[dict | None, int, float]:
    start = time.perf_counter()
    proc = subprocess.run(
        [hanimo, "find", "search", query, CORPUS_REL, "--format", "json"],
        cwd=REPO, capture_output=True, text=True, check=False,
    )
    elapsed = time.perf_counter() - start
    bundle = None
    if proc.stdout.strip():
        try:
            bundle = json.loads(proc.stdout)
        except json.JSONDecodeError:
            bundle = None
    return bundle, proc.returncode, elapsed


def hanimo_spans(bundle: dict | None) -> set[tuple[str, int]]:
    spans: set[tuple[str, int]] = set()
    if not bundle:
        return spans
    for block in bundle.get("blocks", []):
        path = decode_path(block.get("path", {}))
        if path is None:
            continue
        for line in range(block.get("line_start", 0), block.get("line_end", -1) + 1):
            spans.add((path, line))
    return spans


def verify_bundle(hanimo: str, bundle: dict) -> int:
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as tmp:
        json.dump(bundle, tmp)
        tmp_path = tmp.name
    try:
        proc = subprocess.run(
            [hanimo, "find", "verify", tmp_path, "--root", CORPUS_REL],
            cwd=REPO, capture_output=True, text=True, check=False,
        )
        return proc.returncode
    finally:
        os.unlink(tmp_path)


def run_rg(rg: str, patterns: list[str]) -> tuple[set[tuple[str, int]], float]:
    args = [rg, "-n", "--no-heading", "-F"]
    for pat in patterns:
        args += ["-e", pat]
    args += ["."]
    start = time.perf_counter()
    proc = subprocess.run(args, cwd=CORPUS_ABS, capture_output=True, text=True, check=False)
    elapsed = time.perf_counter() - start
    spans: set[tuple[str, int]] = set()
    for raw in proc.stdout.splitlines():
        # format: path:line:text
        parts = raw.split(":", 2)
        if len(parts) < 3:
            continue
        path = parts[0]
        if path.startswith("./"):
            path = path[2:]
        try:
            spans.add((path, int(parts[1])))
        except ValueError:
            continue
    return spans, elapsed


def score(returned: set, gold: set) -> dict:
    gold_hit = len(returned & gold)
    # block/match-level precision: returned units that touch a gold line.
    returned_paths_lines = returned
    covering = {u for u in returned_paths_lines if u in gold}
    recall = gold_hit / len(gold) if gold else 0.0
    precision = len(covering) / len(returned_paths_lines) if returned_paths_lines else (1.0 if not gold else 0.0)
    return {
        "recall": round(recall, 3),
        "precision": round(precision, 3),
        "returned": len(returned),
        "gold": len(gold),
        "gold_covered": gold_hit,
    }


def main() -> int:
    hanimo = find_hanimo()
    rg = find_rg()
    with open(os.path.join(HERE, "probes.json"), encoding="utf-8") as f:
        spec = json.load(f)

    per_probe = []
    for probe in spec["probes"]:
        query = probe.get("query")
        if query is None and "query_nfd_of" in probe:
            query = unicodedata.normalize("NFD", probe["query_nfd_of"])
        rg_patterns = probe.get("rg_patterns", [query])
        gold = {(p, ln) for p, ln in probe["gold"]}

        bundle, a_exit, a_time = run_hanimo_search(hanimo, query)
        a_spans = hanimo_spans(bundle)
        a_score = score(a_spans, gold)
        verify_exit = verify_bundle(hanimo, bundle) if bundle else None
        a_blocks = len(bundle.get("blocks", [])) if bundle else 0
        a_verdict = bundle.get("critic", {}).get("verdict") if bundle else None

        d_spans, d_time = run_rg(rg, rg_patterns)
        d_score = score(d_spans, gold)

        per_probe.append({
            "id": probe["id"], "class": probe["class"], "query": query,
            "expect_hit": probe["expect_hit"],
            "arm_a": {**a_score, "blocks": a_blocks, "verdict": a_verdict,
                      "search_exit": a_exit, "verify_exit": verify_exit,
                      "verified_citation": verify_exit == 0 and a_blocks > 0,
                      "ms": round(a_time * 1000, 2)},
            "arm_d": {**d_score, "ms": round(d_time * 1000, 2)},
        })

    # Structural property 1: determinism (identical bundle digest across 3 runs).
    digests = []
    for _ in range(3):
        bundle, _e, _t = run_hanimo_search(hanimo, "DEPLOY_REGION")
        digests.append(bundle.get("bundle_sha256") if bundle else None)
    determinism = {"runs": 3, "distinct_digests": len(set(digests)),
                   "deterministic": len(set(digests)) == 1 and digests[0] is not None,
                   "digest": digests[0]}

    # Structural property 2: forged evidence is rejected (tamper content -> exit 4).
    bundle, _e, _t = run_hanimo_search(hanimo, "DEPLOY_REGION")
    forged_exit = None
    if bundle and bundle.get("blocks"):
        tampered = json.loads(json.dumps(bundle))
        blk = tampered["blocks"][0]
        if blk.get("content", {}).get("encoding") == "utf8":
            text = blk["content"]["text"]
            blk["content"]["text"] = ("X" + text[1:]) if text else "X"
        forged_exit = verify_bundle(hanimo, tampered)
    forgery = {"tampered_verify_exit": forged_exit,
               "forged_rejected": forged_exit in (3, 4)}

    # Aggregate by expect_hit and by arm.
    def agg(rows, arm, field):
        vals = [r[arm][field] for r in rows]
        return round(sum(vals) / len(vals), 3) if vals else None

    hits = [r for r in per_probe if r["expect_hit"]]
    boundary = [r for r in per_probe if not r["expect_hit"]]
    summary = {
        "expect_hit_probes": len(hits),
        "boundary_probes": len(boundary),
        "arm_a_recall_on_hits": agg(hits, "arm_a", "recall"),
        "arm_d_recall_on_hits": agg(hits, "arm_d", "recall"),
        "arm_a_recall_on_boundary": agg(boundary, "arm_a", "recall"),
        "arm_d_recall_on_boundary": agg(boundary, "arm_d", "recall"),
        "arm_a_verified_citation_rate": round(
            sum(1 for r in per_probe if r["arm_a"]["verified_citation"]) /
            max(1, sum(1 for r in per_probe if r["arm_a"]["blocks"] > 0)), 3),
        "arm_d_verifiable_citations": "none (ripgrep emits no citation contract)",
    }

    results = {
        "corpus_root": CORPUS_REL,
        "corpus_files": len(corpus_files()),
        "corpus_sha256": corpus_digest(),
        "hanimo": os.path.relpath(hanimo, REPO),
        "ripgrep": subprocess.run([rg, "--version"], capture_output=True, text=True).stdout.splitlines()[0],
        "determinism": determinism,
        "forgery": forgery,
        "summary": summary,
        "probes": per_probe,
    }
    with open(os.path.join(HERE, "results.json"), "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
        f.write("\n")

    print(json.dumps({"determinism": determinism, "forgery": forgery, "summary": summary},
                     ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
