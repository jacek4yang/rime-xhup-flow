#!/usr/bin/env python3
"""Paired legal-code quality by input mode; retain every failure and overlap."""
import argparse
import hashlib
import json
from pathlib import Path

from prepare_legal_corpus import MODES
from summarize_native_corpus import intervals, rates, read_trace


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def fixture_rows(path):
    rows = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        identity, keys, text = line.split("\t")
        if identity in rows:
            raise ValueError("duplicate fixture identity")
        rows[identity] = (keys, text)
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metadata", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--partition", choices=("development", "confirmation"), required=True)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--known-fixture", type=Path, action="append", default=[])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    metadata = json.loads(args.metadata.read_text(encoding="utf-8"))
    expected = metadata["source"][args.partition]
    if digest(args.fixture) != expected["sha256"] or metadata["partitions"][args.partition] != expected:
        raise ValueError("frozen partition identity mismatch")
    cases = [case for case in metadata["cases"] if case["partition"] == args.partition]
    fixture = fixture_rows(args.fixture)
    if (len(cases) != expected["cases"] or len(fixture) != len(cases)
            or set(fixture) != {case["id"] for case in cases}):
        raise ValueError("partition fixture and metadata do not match")
    for case in cases:
        keys, text = fixture[case["id"]]
        if case["keys"] != len(keys) or case["characters"] != len(text):
            raise ValueError("frozen key/text length mismatch")
        if (keys != "".join(segment["keys"] for segment in case["segments"])
                or text != "".join(segment["text"] for segment in case["segments"])):
            raise ValueError("frozen segmentation does not reconstruct input and target")
    subset = {"cases": cases}
    baseline, _ = read_trace(args.baseline, subset, "planner")
    candidate, _ = read_trace(args.candidate, subset, "planner")
    if (any(row.get("joint_decoder") is not False for row in baseline.values())
            or any(row.get("joint_decoder") is not True for row in candidate.values())):
        raise ValueError("comparison did not run baseline-off/candidate-on")
    known_texts = set()
    for path in args.known_fixture:
        known_texts.update(text for _, text in fixture_rows(path).values())
    paired, summaries = [], {}
    for case in cases:
        identity = case["id"]
        keys, text = fixture[identity]
        paired.append({"id": identity, "mode": case["mode"], "keys": keys, "target": text,
                       "previously_observed_text": text in known_texts,
                       "baseline": baseline[identity], "candidate": candidate[identity]})
    for mode in MODES:
        group = [case for case in cases if case["mode"] == mode]
        if not group:
            raise ValueError(f"missing input mode: {mode}")
        ids = [case["id"] for case in group]
        unseen = [identity for identity in ids if fixture[identity][1] not in known_texts]
        summaries[mode] = {
            "baseline": rates([baseline[i] for i in ids]),
            "candidate": rates([candidate[i] for i in ids]),
            "paired_delta_clustered_95": intervals(group, candidate, baseline)["paired_delta"],
            "baseline_top1_lost": [i for i in ids if baseline[i]["rank"] == 0 and candidate[i]["rank"] != 0],
            "baseline_first_page_lost": [i for i in ids if baseline[i]["first_page"] and not candidate[i]["first_page"]],
            "same_input_as_full": sum(case["same_input_as_full"] for case in group),
            "known_text_overlap_cases": len(ids) - len(unseen),
            "without_supplied_known_texts": {
                "baseline": rates([baseline[i] for i in unseen]) if unseen else None,
                "candidate": rates([candidate[i] for i in unseen]) if unseen else None,
            },
        }
    report = {
        "partition": args.partition, "source": metadata["source"], "modes": summaries,
        "limits": ["Translated news/wiki and limited legal-code coverage, not conversation prevalence",
                   "Known overlap refers only to the explicitly supplied fixtures; other overlap is unknown",
                   "No pooled accuracy across correlated input modes; all failures retained",
                   "Pending reachability is observed candidate text, not per-case pending-selection proof",
                   "Corpus timings are not paired performance or GUI latency evidence"],
        "identities": {str(path): digest(path) for path in
                       (args.metadata, args.fixture, args.baseline, args.candidate, *args.known_fixture)},
        "cases": paired,
    }
    with args.output.open("x", encoding="utf-8") as stream:
        json.dump(report, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print(json.dumps({mode: {"baseline": row["baseline"], "candidate": row["candidate"],
                            "top1_lost": len(row["baseline_top1_lost"]),
                            "first_page_lost": len(row["baseline_first_page_lost"])}
                      for mode, row in summaries.items()}, ensure_ascii=False, sort_keys=True))
    if any(row["contract_failures"] for row in (*baseline.values(), *candidate.values())):
        raise SystemExit("native contract failures retained in report")


if __name__ == "__main__":
    main()
