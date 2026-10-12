#!/usr/bin/env python3
"""Linux native API measurements, not GUI latency or correctness qualification.

Copy an isolated, already deployed test package for every independent process.
Never point --deployed at a user's Rime directory. Instrumented runs measure work
and allocation/GC counters only; compare latency using uninstrumented runtimes.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def percentiles(values):
    values = sorted(values)
    if not values:
        raise ValueError("empty measurement")
    return {f"p{p}": values[math.ceil(len(values) * p / 100) - 1] for p in (50, 95, 99)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--deployed", type=Path, required=True)
    parser.add_argument("--plugin", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--shared", type=Path, default=Path("/usr/share/rime-data"))
    parser.add_argument("--schema", choices=("xhup_flow", "xhup_flow_static"), default="xhup_flow")
    parser.add_argument("--learning", choices=("on", "off"), default="off")
    parser.add_argument("--quick-hint", choices=("on", "off"), default="on")
    parser.add_argument("--profile", action="store_true")
    parser.add_argument("--complete-menu", action="store_true", help="separate final-state full enumeration, capped at 100000 candidates")
    parser.add_argument("--sustained", action="store_true", help="retain one native session across all cases/rounds")
    parser.add_argument("--trials", type=int, default=5)
    parser.add_argument("--repetitions", type=int, default=10)
    args = parser.parse_args()
    if sys.platform != "linux" or not 1 <= args.trials <= 100 or not 1 <= args.repetitions <= 1000:
        parser.error("Linux only; trials 1..100 and repetitions 1..1000")
    if args.schema == "xhup_flow_static" and args.learning != "off":
        parser.error("Static has no native learning mode")
    root = Path(__file__).resolve().parent
    deployed, plugin, output = args.deployed.resolve(), args.plugin.resolve(), args.output.resolve()
    if not (deployed / "build/xhup_flow.schema.yaml").is_file() or not plugin.is_file():
        parser.error("missing deployed test schema or plugin")
    source_files = [root / name for name in ("runtime_performance.c", "runtime_replay.c", "runtime_module.h", "profile_runtime.h", "measure_runtime.py")]
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    for path in source_files:
        committed = subprocess.check_output(["git", "show", f"{revision}:tests/librime/{path.name}"], cwd=root)
        if hashlib.sha256(committed).hexdigest() != sha256(path):
            raise ValueError(f"commit measurement sources first: {path}")
    output.mkdir()
    provenance = {
        "scope": "native-api-complete-menu" if args.complete_menu else "native-api-visible-page",
        "instrumented": args.profile,
        "gui_acceptance": False,
        "session_lifetime": "process" if args.sustained else "case",
        "rss_source": "Linux /proc/self/status VmHWM; /proc/self/statm round snapshots",
        "harness_commit": revision,
        "schema": args.schema, "learning": args.learning,
        "quick_hint": args.quick_hint,
        "trials": args.trials, "repetitions": args.repetitions,
        "platform": platform.platform(),
        "cpu_affinity": sorted(os.sched_getaffinity(0)),
        "source_sha256": {p.name: sha256(p) for p in source_files},
        "plugin_sha256": sha256(plugin),
        "dynamic_dependencies": subprocess.check_output(["ldd", str(plugin)], text=True),
        "compiler": subprocess.check_output(["cc", "--version"], text=True),
        "rime_version": subprocess.check_output(["pkg-config", "--modversion", "rime"], text=True).strip(),
        "deployment_sha256": {str(p.relative_to(deployed)): sha256(p) for p in sorted(deployed.rglob("*")) if p.is_file()},
    }
    if args.profile:
        provenance["instrumentation_sha256"] = {
            p.name: sha256(p) for p in (plugin.parent / "liblua5.4.so.0", plugin.parent / "provenance.txt")
        }
    (output / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    flags = subprocess.check_output(["pkg-config", "--cflags", "--libs", "rime"], text=True).split()
    executable = output / "runtime_performance"
    subprocess.run(["cc", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", str(root / "runtime_performance.c"), *flags, "-ldl", "-o", str(executable)], check=True)
    env = dict(os.environ, LD_PRELOAD=str(plugin), XHUP_LUA_LIBRARY=str(plugin))
    env["XHUP_MEASURE_QUICK_HINT"] = args.quick_hint
    if args.profile:
        env["XHUP_REQUIRE_PROFILE"] = "1"
    else:
        env.pop("XHUP_REQUIRE_PROFILE", None)
    if args.complete_menu:
        env["XHUP_MEASURE_COMPLETE_MENU"] = "1"
    else:
        env.pop("XHUP_MEASURE_COMPLETE_MENU", None)
    if args.sustained:
        env["XHUP_MEASURE_SUSTAINED"] = "1"
    else:
        env.pop("XHUP_MEASURE_SUSTAINED", None)
    summaries = []
    for trial in range(args.trials):
        directory = output / f"trial-{trial}"
        shutil.copytree(deployed, directory)
        enabled = "true" if args.learning == "on" else "false"
        (directory / "xhup_flow.custom.yaml").write_text(f"patch:\n  flow/enable_user_dict: {enabled}\n  learn/enable_user_dict: {enabled}\n")
        with (output / f"deploy-{trial}.log").open("w") as log:
            subprocess.run(["rime_deployer", "--compile", str(directory / f"{args.schema}.schema.yaml"), str(directory), str(args.shared)], env=env, stdout=log, stderr=log, check=True)
        started = time.monotonic_ns()
        with (output / f"trial-{trial}.jsonl").open("w") as out, (output / f"trial-{trial}.stderr").open("w") as err:
            subprocess.run([str(executable), str(directory), str(args.shared), args.schema, str(args.repetitions)], env=env, stdout=out, stderr=err, check=True)
        elapsed = time.monotonic_ns() - started
        events = [json.loads(line) for line in (output / f"trial-{trial}.jsonl").read_text().splitlines()]
        rows, menus, last_key = [], [], None
        for event in events:
            if "key" in event:
                rows.append(event)
                last_key = event
            elif event.get("event") == "complete_menu":
                if not args.complete_menu or last_key is None or last_key["case"] != event["case"]:
                    raise ValueError("complete-menu event is not bound to its final key")
                if not event["exhausted"]:
                    raise ValueError("complete-menu cap reached; preserved trace is not a complete measurement")
                event["key_to_complete_menu_work_ns"] = last_key["key_to_menu_ns"] + event["enumeration_ns"]
                menus.append(event)
            else:
                raise ValueError("unknown measurement event")
        if len(menus) != (11 * args.repetitions if args.complete_menu else 0):
            raise ValueError("incomplete final-state menu measurements")
        if not rows or any(("profile_to_first_page" in row) != args.profile for row in rows):
            raise ValueError("instrumentation identity disagrees with requested run")
        stderr = (output / f"trial-{trial}.stderr").read_text().splitlines()
        sessions = [int(line.split()[-1]) for line in stderr if line.startswith("XHUP_SESSION_START_NS ")]
        rss = [int(line.split()[-1]) for line in stderr if line.startswith("XHUP_PEAK_RSS_KIB ")]
        expected_sessions = 1 if args.sustained else args.repetitions * 9
        if len(sessions) != expected_sessions or len(rss) != 1:
            raise ValueError("incomplete process/session measurements")
        memory_rounds = [
            [int(value) for value in line.split()[1:]]
            for line in stderr if line.startswith("XHUP_ROUND_RSS_KIB ")
        ]
        lua_rounds = [
            [int(value) for value in line.split()[1:]]
            for line in stderr if line.startswith("XHUP_ROUND_LUA_BYTES ")
        ]
        if [row[0] for row in memory_rounds] != list(range(args.repetitions)):
            raise ValueError("missing resident-memory round")
        if [row[0] for row in lua_rounds] != (list(range(args.repetitions)) if args.profile else []):
            raise ValueError("missing Lua-memory round")
        if args.schema == "xhup_flow":
            statuses = [line.split()[-1] for line in stderr if line.startswith("XHUP_LEARNING_STATUS ")]
            expected_status = "ready" if args.learning == "on" else "off"
            if statuses != [expected_status] * len(sessions):
                raise ValueError(f"runtime did not enter requested learning state {expected_status}")
        hint_statuses = [line.split()[-1] for line in stderr if line.startswith("XHUP_QUICK_HINT ")]
        if hint_statuses != [args.quick_hint] * len(sessions):
            raise ValueError("runtime did not enter requested quick-hint state")
        cases = {}
        for name in sorted({row["case"] for row in rows}):
            selected = [row for row in rows if row["case"] == name]
            result = {"key_events": len(selected)}
            for field in ("keypress_ns", "key_to_menu_ns", "first_page_ns", "key_to_menu_cpu_ns"):
                result[field] = percentiles([row[field] for row in selected])
            if args.profile:
                for field in ("dict_lookups", "native_queries", "allocation_calls", "growth_bytes", "incremental_collections", "generational_collections"):
                    values = [row["profile_to_first_page"][field] for row in selected]
                    result[field] = {"sum": sum(values), "max": max(values), **percentiles(values)}
            if args.complete_menu:
                complete = [event for event in menus if event["case"] == name]
                result["complete_samples"] = len(complete)
                result["key_to_complete_menu_work_ns"] = percentiles([event["key_to_complete_menu_work_ns"] for event in complete])
                result["complete_candidates"] = percentiles([event["candidates"] for event in complete])
            cases[name] = result
        summaries.append({
            "trial": trial, "elapsed_ns": elapsed, "peak_rss_kib": rss[0],
            "first_session_ns": sessions[0],
            "subsequent_session_ns": percentiles(sessions[1:]) if len(sessions) > 1 else None,
            "rss_kib_by_round": memory_rounds, "lua_bytes_by_round": lua_rounds,
            "cases": cases,
        })
    if any(sha256(path) != provenance["source_sha256"][path.name] for path in source_files):
        raise ValueError("measurement source changed during run")
    (output / "summary.json").write_text(json.dumps(summaries, indent=2) + "\n")
    print(json.dumps({"output": str(output), "trials": len(summaries), "instrumented": args.profile}))


if __name__ == "__main__":
    main()
