#!/usr/bin/env python3
"""Deterministic complete native-audit partitions and fail-closed CI receipts.

No sampling: every non-comment row goes to exactly one partition. Receipts are
CI completion records, not signatures or proof against a compromised runner.
The collector independently reconstructs every partition from original inputs.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

KINDS = ("static", "extended", "open")
BANNERS = (
    "PASS native audit: complete supplied static manifest before/after learning, composition/learning/restart",
    "PASS complete supplied extended reachability manifest",
    "PASS complete supplied open-composition sample manifest",
    "PASS CLI learning management",
)


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def identity(root):
    package = root / "package"
    files = {}
    for path in sorted(package.rglob("*")):
        if path.is_symlink():
            raise ValueError("package symlink")
        if path.is_file():
            files[path.relative_to(package).as_posix()] = digest(path)
    if not files or not (package / "xhup_flow.schema.yaml").is_file():
        raise ValueError("missing generated package")
    return {"package": files, "cli": digest(root / "xhup-cli"),
            "lua_plugin": digest(root / "librime-lua.so")}


def partitions(root, count, write=False):
    if not 1 <= count <= 64:
        raise ValueError("partition count must be 1..64")
    result = {}
    for kind in KINDS:
        source = root / f"{kind}.manifest"
        hashes = [hashlib.sha256() for _ in range(count)]
        rows = [0] * count
        outputs = []
        try:
            if write:
                for index in range(count):
                    directory = root / "parts" / str(index)
                    directory.mkdir(parents=True, exist_ok=True)
                    outputs.append((directory / f"{kind}.manifest").open("xb"))
            with source.open("rb") as stream:
                for line in stream:
                    if line.startswith(b"#"):
                        continue
                    if (not line.endswith(b"\n") or b"\0" in line
                            or b"\r" in line or b"\t" not in line):
                        raise ValueError(f"invalid {kind} row")
                    code, text = line[:-1].split(b"\t", 1)
                    if not re.fullmatch(b"[a-z]+", code) or not text:
                        raise ValueError(f"invalid {kind} code/text")
                    text.decode("utf-8")
                    index = sum(rows) % count
                    hashes[index].update(line)
                    rows[index] += 1
                    if write:
                        outputs[index].write(line)
        finally:
            for stream in outputs:
                stream.close()
        if not all(rows):
            raise ValueError(f"empty {kind} partition")
        result[kind] = {
            "sha256": digest(source), "rows": sum(rows),
            "parts": [{"sha256": h.hexdigest(), "rows": n}
                      for h, n in zip(hashes, rows)],
        }
    return result


def expected_plan(root, count, revision, run):
    if not re.fullmatch(r"[0-9a-f]{40}", revision) or not run:
        raise ValueError("full source revision and run identity required")
    return {"version": 1, "revision": revision, "run": run, "count": count,
            "inputs": partitions(root, count), **identity(root)}


def validate(root, revision, run):
    plan = json.loads((root / "plan.json").read_text())
    expected = expected_plan(root, plan["count"], revision, run)
    if plan != expected:
        raise ValueError("plan/input/source/run identity mismatch")
    for kind in KINDS:
        for i, part in enumerate(plan["inputs"][kind]["parts"]):
            path = root / "parts" / str(i) / f"{kind}.manifest"
            if digest(path) != part["sha256"]:
                raise ValueError("partition bytes mismatch")
    return plan


def write_json(path, value):
    # Refuse stale output reuse (including receipts from an earlier attempt).
    with path.open("x") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


def expected_receipt(root, plan, index, log):
    if not 0 <= index < plan["count"]:
        raise ValueError("invalid partition index")
    lines = log.read_text().splitlines()
    if any(lines.count(banner) != 1 for banner in BANNERS):
        raise ValueError("missing or duplicate full-audit completion")
    if any(line.startswith("FAIL") or "NOT RUN" in line for line in lines):
        raise ValueError("failed/skipped audit evidence")
    return {"version": 1, "plan_sha256": digest(root / "plan.json"),
            "revision": plan["revision"], "run": plan["run"], "index": index,
            "count": plan["count"], "exit_code": 0,
            "log_sha256": digest(log),
            "coverage": {kind: plan["inputs"][kind]["parts"][index]
                         for kind in KINDS}}


def execute(root, out, revision, run, index):
    plan = validate(root, revision, run)
    if not 0 <= index < plan["count"]:
        raise ValueError("invalid partition index")
    repo = Path(__file__).resolve().parents[2]
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo,
                                   text=True).strip()
    if head != revision:
        raise ValueError("checkout revision mismatch")
    # A contaminated environment must never silently select focused mode.
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(("XHUP_AUDIT_", "XHUP_REPLAY_"))}
    env["XHUP_AUDIT_SHARDS"] = "2"
    out.mkdir(parents=True, exist_ok=True)
    log = out / f"audit-{index}.log"
    parts = root / "parts" / str(index)
    command = ["bash", str(repo / "tests/librime/run-flow-audit.sh"),
               str(root / "package"), str(parts / "static.manifest"),
               str(root / "xhup-cli"), str(parts / "extended.manifest"),
               str(parts / "open.manifest")]
    with log.open("x") as stream:
        subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT,
                       env=env, cwd=repo, check=True)
    write_json(out / f"receipt-{index}.json",
               expected_receipt(root, plan, index, log))


def collect(root, out, revision, run):
    plan = validate(root, revision, run)
    wanted = {f"receipt-{i}.json" for i in range(plan["count"])}
    if {p.name for p in out.glob("receipt-*.json")} != wanted:
        raise ValueError("missing/extra receipt")
    if {p.name for p in out.glob("audit-*.log")} != {
            f"audit-{i}.log" for i in range(plan["count"])}:
        raise ValueError("missing/extra log")
    for i in range(plan["count"]):
        receipt = json.loads((out / f"receipt-{i}.json").read_text())
        if receipt != expected_receipt(root, plan, i, out / f"audit-{i}.log"):
            raise ValueError("receipt does not match complete partition")
    return {"version": 1, "revision": revision, "run": run,
            "plan_sha256": digest(root / "plan.json"),
            "partitions_completed": plan["count"],
            "rows": {kind: plan["inputs"][kind]["rows"] for kind in KINDS}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("prepare", "run", "collect"))
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--run", required=True)
    parser.add_argument("--count", type=int, default=16)
    parser.add_argument("--index", type=int)
    args = parser.parse_args()
    root = args.root.resolve()
    if args.mode == "prepare":
        partitions(root, args.count, write=True)
        write_json(root / "plan.json",
                   expected_plan(root, args.count, args.revision, args.run))
        validate(root, args.revision, args.run)
    else:
        if args.out is None:
            parser.error("--out is required")
        if args.mode == "run":
            if args.index is None:
                parser.error("--index is required")
            execute(root, args.out, args.revision, args.run, args.index)
        else:
            report = collect(root, args.out, args.revision, args.run)
            write_json(args.out / "coverage.json", report)
            print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
