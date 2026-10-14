#!/usr/bin/env python3
"""Alternate paired native API runs on copied synthetic deployments, never GUI proof.

Both sides start with empty user state. Original deployments remain untouched;
all traces, including semantic differences, are retained in the output directory.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

from measure_runtime import percentiles, sha256

SEMANTIC_FIELDS = ("case", "key", "handled", "page_size", "raw", "preedit", "selection", "head")
COUNTERS = ("dict_lookups", "native_queries", "allocation_calls", "growth_bytes",
            "incremental_collections", "generational_collections")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--plugin", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--trials", type=int, default=5)
    parser.add_argument("--repetitions", type=int, default=10)
    parser.add_argument("--learning", choices=("on", "off"), default="off")
    parser.add_argument("--quick-hint", choices=("on", "off"), default="on")
    parser.add_argument("--baseline-joint", choices=("on", "off", "schema"), default="schema")
    parser.add_argument("--candidate-joint", choices=("on", "off", "schema"), default="schema")
    parser.add_argument("--schema", choices=("xhup_flow", "xhup_flow_static"), default="xhup_flow")
    parser.add_argument("--profile", action="store_true")
    parser.add_argument("--complete-menu", action="store_true")
    parser.add_argument("--sustained", action="store_true")
    parser.add_argument("--require-equivalent", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.trials <= 100 or not 1 <= args.repetitions <= 1000:
        parser.error("trials 1..100 and repetitions 1..1000")
    if args.schema == "xhup_flow_static" and args.learning != "off":
        parser.error("Static has no native learning mode")
    root = Path(__file__).resolve().parent
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    script = Path(__file__).resolve()
    committed = subprocess.check_output(["git", "show", f"{revision}:tests/librime/{script.name}"], cwd=root)
    script_hash = sha256(script)
    if hashlib.sha256(committed).hexdigest() != script_hash:
        raise ValueError("commit the comparison driver before measuring")
    output = args.output.resolve()
    sources = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    for source in sources.values():
        if source == output or source in output.parents:
            parser.error("output must be outside both source deployments")
        if not (source / "build/xhup_flow.schema.yaml").is_file():
            parser.error(f"missing deployed synthetic schema: {source}")
    output.mkdir()
    for label, source in sources.items():
        shutil.copytree(source, output / f"{label}-source",
                        ignore=shutil.ignore_patterns("*.userdb*", "user.yaml"))
    (output / "comparison-provenance.json").write_text(json.dumps({
        "harness_commit": revision, "driver_sha256": script_hash,
        "arguments": {key: str(value) if isinstance(value, Path) else value
                      for key, value in vars(args).items()},
        "user_state": "fresh copies omit *.userdb* and user.yaml on both sides",
        "menu_page_size": 5,
        "gui_acceptance": False,
    }, indent=2) + "\n")
    all_rows = {label: [] for label in sources}
    process_summaries = {label: [] for label in sources}
    differences = []
    for pair in range(args.trials):
        order = ("baseline", "candidate") if pair % 2 == 0 else ("candidate", "baseline")
        pair_rows = {}
        for label in order:
            directory = output / f"pair-{pair}-{label}"
            command = [sys.executable, str(root / "measure_runtime.py"),
                       "--deployed", str(output / f"{label}-source"), "--plugin", str(args.plugin),
                       "--output", str(directory), "--trials", "1", "--repetitions", str(args.repetitions),
                       "--schema", args.schema, "--learning", args.learning,
                       "--quick-hint", args.quick_hint,
                       "--joint-decoder", getattr(args, f"{label}_joint")]
            for enabled, flag in ((args.profile, "--profile"), (args.complete_menu, "--complete-menu"),
                                  (args.sustained, "--sustained")):
                if enabled:
                    command.append(flag)
            print(f"== pair {pair}: {label} ==", flush=True)
            subprocess.run(command, check=True)
            events = [json.loads(line) for line in (directory / "trial-0.jsonl").read_text().splitlines()]
            pair_rows[label] = [event for event in events if "key" in event]
            all_rows[label].extend(pair_rows[label])
            process_summaries[label].append(json.loads((directory / "summary.json").read_text())[0])
        before, after = pair_rows["baseline"], pair_rows["candidate"]
        if [(row["case"], row["key"]) for row in before] != [(row["case"], row["key"]) for row in after]:
            raise ValueError("paired inputs differ; all raw evidence retained")
        for position, (old, new) in enumerate(zip(before, after)):
            changed = [key for key in SEMANTIC_FIELDS if old[key] != new[key]]
            if changed:
                differences.append({"pair": pair, "row": position, "fields": changed})
    metrics = {}
    names = sorted({row["case"] for row in all_rows["baseline"]})
    for label, rows in all_rows.items():
        cases = {}
        for name in ["all", *names]:
            selected = rows if name == "all" else [row for row in rows if row["case"] == name]
            result = {"key_events": len(selected)}
            if args.profile:
                for field in COUNTERS:
                    values = [row["profile_to_first_page"][field] for row in selected]
                    result[field] = {"sum": sum(values), "max": max(values)}
            else:
                for field in ("keypress_ns", "key_to_menu_ns", "first_page_ns", "key_to_menu_cpu_ns"):
                    result[field] = percentiles([row[field] for row in selected])
            cases[name] = result
        metrics[label] = {"cases": cases, "processes": process_summaries[label]}
    report = {"instrumented": args.profile, "gui_acceptance": False, "menu_page_size": 5,
              "pairs": args.trials, "semantic_differences": differences, "metrics": metrics}
    (output / "comparison.json").write_text(json.dumps(report, indent=2) + "\n")
    if sha256(script) != script_hash:
        raise ValueError("comparison driver changed during measurement")
    if args.require_equivalent and differences:
        raise ValueError(f"{len(differences)} observable differences; evidence retained")
    print(json.dumps({"output": str(output), "pairs": args.trials,
                      "semantic_differences": len(differences), "instrumented": args.profile}))


if __name__ == "__main__":
    main()
