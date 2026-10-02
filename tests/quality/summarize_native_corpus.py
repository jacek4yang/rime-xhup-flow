#!/usr/bin/env python3
"""Summarize ALL frozen corpus cases, with sentence-clustered paired intervals."""
import argparse
from collections import defaultdict
import hashlib
import json
import math
from pathlib import Path
import random

METRICS = ("top1", "top3", "top256", "commit_exact", "pending_reachable")
SEED = 20261002
REPLICATES = 2000


def quantile(values, fraction):
    values = sorted(values)
    return values[max(0, math.ceil(len(values) * fraction) - 1)]


def outcomes(row):
    rank = row["rank"]
    return [int(rank == 0), int(0 <= rank < 3), int(rank >= 0),
            int(row["commit_exact"]), int(row["pending_reachable"])]


def read_trace(path, metadata, mode):
    expected = {case["id"] for case in metadata["cases"]}
    rows, timing = {}, defaultdict(list)
    steps = defaultdict(int)
    commits = defaultdict(int)
    for line in path.read_text().splitlines():
        if not line.startswith("{"):
            continue  # audit deployment headings are not JSON records
        row = json.loads(line)
        if row.get("case") not in expected:
            raise ValueError("unknown trace case")
        if row.get("event") == "corpus_result":
            if row["case"] in rows or row["mode"] != mode:
                raise ValueError("duplicate case or wrong mode")
            if type(row["rank"]) is not int or not -1 <= row["rank"] < 256:
                raise ValueError("invalid rank")
            if row["commit_exact"] and row["rank"] < 0:
                raise ValueError("commit without reachable target")
            rows[row["case"]] = row
        elif row.get("event") == "selection_commit":
            commits[row["case"]] += 1
        elif "key" in row:
            steps[row["case"]] += 1
            for field in ("keypress_ns", "key_to_menu_ns", "bounded_probe_ns"):
                if row[field] < 0:
                    raise ValueError("negative latency")
                timing[field].append(row[field] / 1e6)
    if set(rows) != expected or not rows:
        raise ValueError("incomplete trace (never silently drop missing cases)")
    for case in metadata["cases"]:
        case_id = case["id"]
        if rows[case_id]["keys"] != case["characters"] * 2:
            raise ValueError("input length does not match frozen fixture")
        if steps[case_id] != case["characters"] * 2 + 4:
            raise ValueError("missing or duplicated per-key evidence")
        if rows[case_id]["commit_exact"] and commits[case_id] != 1:
            raise ValueError("missing or duplicated commit evidence")
    return rows, {key: {"samples": len(values), "p50_ms": quantile(values, .5),
                       "p95_ms": quantile(values, .95), "p99_ms": quantile(values, .99),
                       "max_ms": max(values)} for key, values in timing.items()}


def rates(rows):
    totals = [sum(v) for v in zip(*(outcomes(row) for row in rows))]
    return {"cases": len(rows), **{key: total / len(rows) for key, total in zip(METRICS, totals)},
            "contract_failures": sum(row["contract_failures"] for row in rows)}


def intervals(cases, planner, baseline):
    grouped = defaultdict(list)
    for case in cases:
        grouped[case["sentence"]].append(case["id"])
    # Per-sentence sums retain the case-weighted estimand when resampling clusters.
    groups = []
    for ids in grouped.values():
        a = [sum(v) for v in zip(*(outcomes(planner[i]) for i in ids))]
        b = [sum(v) for v in zip(*(outcomes(baseline[i]) for i in ids))]
        groups.append((len(ids), a, b))
    rng = random.Random(SEED)
    draws = {label: [[] for _ in METRICS] for label in ("planner", "native-only", "paired_delta")}
    for _ in range(REPLICATES):
        total, a, b = 0, [0]*len(METRICS), [0]*len(METRICS)
        for _ in groups:
            n, x, y = groups[rng.randrange(len(groups))]
            total += n
            a = [u+v for u, v in zip(a, x)]
            b = [u+v for u, v in zip(b, y)]
        for i in range(len(METRICS)):
            draws["planner"][i].append(a[i]/total)
            draws["native-only"][i].append(b[i]/total)
            draws["paired_delta"][i].append((a[i]-b[i])/total)
    return {"clusters_with_eligible_cases": len(groups), "seed": SEED,
            "replicates": REPLICATES, "method": "sentence cluster percentile bootstrap; case-weighted rates",
            **{label: {key: [quantile(samples,.025), quantile(samples,.975)]
                       for key, samples in zip(METRICS, values)} for label, values in draws.items()}}


def summarize(metadata, planner, baseline):
    subsets = {"all": metadata["cases"]}
    for low, high in ((2,4),(5,8),(9,16),(17,64)):
        subsets[f"length_{low}_{high}"] = [c for c in metadata["cases"] if low <= c["characters"] <= high]
    for value in (True, False):
        subsets[f"proper_name_{str(value).lower()}"] = [c for c in metadata["cases"] if c["proper_name"] == value]
    result = {label: {name: rates([rows[c["id"]] for c in cases]) for name, cases in subsets.items() if cases}
              for label, rows in (("planner",planner),("native-only",baseline))}
    result["clustered_95_intervals"] = intervals(metadata["cases"], planner, baseline)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("metadata", type=Path)
    parser.add_argument("planner", type=Path)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    metadata = json.loads(args.metadata.read_text())
    planner, planner_timing = read_trace(args.planner, metadata, "planner")
    baseline, baseline_timing = read_trace(args.baseline, metadata, "native-only")
    report = summarize(metadata, planner, baseline)
    report.update({"schema": 1, "source": metadata["source"], "counts": metadata["counts"],
                   "fixture_sha256": metadata["fixture_sha256"],
                   "encoder_export_sha256": metadata["encoder_export_sha256"],
                   "trace_sha256": {mode: hashlib.sha256(path.read_bytes()).hexdigest()
                                    for mode, path in (("planner",args.planner),("native-only",args.baseline))},
                   "latency": {"planner": planner_timing, "native-only": baseline_timing},
                   "limits": ["source-external wiki, not representative conversation",
                              "not yet established disjoint from all upstream training corpora",
                              "latency includes edit probes under concurrent exhaustive-audit load",
                              "top256 bound, not exhaustive candidate reachability",
                              "quality misses are retained; no post-outcome selection or retuning"]})
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2)+"\n")
    print(json.dumps({label: report[label]["all"] for label in ("planner","native-only")}, indent=2))


if __name__ == "__main__":
    main()
