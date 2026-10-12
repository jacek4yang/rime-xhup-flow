#!/usr/bin/env python3
"""Paired fresh-Lua-VM loading; not librime startup, disk-cold I/O, or key latency."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import platform
import re
import shutil
import statistics
import subprocess


def snapshot(package):
    paths = [package / "xhup_flow.schema.yaml", *sorted((package / "lua").rglob("*.lua"))]
    result = {}
    for path in paths:
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"unsafe module input: {path}")
        data = path.read_bytes()
        result[path.relative_to(package).as_posix()] = {
            "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(),
        }
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--samples", type=int, default=100)
    parser.add_argument("--lua", default="lua5.4")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.samples < 2:
        parser.error("at least two samples are required")
    executable = Path(shutil.which(args.lua) or args.lua).resolve(strict=True)
    script = Path(__file__).with_suffix(".lua")
    packages = {name: getattr(args, name).resolve() for name in ("baseline", "candidate")}
    identities = {name: snapshot(package) for name, package in packages.items()}
    roots = {}
    for name, package in packages.items():
        modules = []
        for line in (package / "xhup_flow.schema.yaml").read_text(encoding="utf-8").splitlines():
            entry = line.strip()
            if not entry.startswith("- lua_"):
                continue
            module = entry.partition("@*")[2]
            if not re.fullmatch(r"xhup_flow(?:\.[a-z_][a-z0-9_]*)+", module):
                raise ValueError(f"unsupported schema module declaration: {entry}")
            if module not in modules:
                modules.append(module)
        if not modules:
            raise ValueError(f"schema has no Lua roots: {package}")
        roots[name] = modules
    if roots["baseline"] != roots["candidate"]:
        raise ValueError("paired loading requires the same schema roots and order")
    samples = {name: [] for name in packages}
    for index in range(args.samples):
        # Counterbalance process order; every sample starts a separate interpreter.
        order = list(packages) if index % 2 == 0 else list(reversed(packages))
        for name in order:
            command = [str(executable), "-E", str(script), str(packages[name]), *roots[name]]
            output = subprocess.check_output(command, text=True)
            samples[name].append(json.loads(output))
    summaries = {}
    for name, rows in samples.items():
        if snapshot(packages[name]) != identities[name]:
            raise ValueError(f"module inputs changed during measurement: {name}")
        summaries[name] = {}
        for metric in ("load_cpu_ms", "live_heap_kib", "warm_require_us_per_pass"):
            values = sorted(row[metric] for row in rows)
            summaries[name][metric] = {
                "median": statistics.median(values),
                "p95": values[math.ceil(len(values) * 0.95) - 1],
                "p99": values[math.ceil(len(values) * 0.99) - 1],
            }
        summaries[name]["all_lua_bytes"] = sum(
            identity["bytes"] for path, identity in identities[name].items() if path.endswith(".lua")
        )
    report = {
        "schema_version": 1,
        "scope": "fresh VM schema-root requires; warm OS cache; no librime or GUI initialization",
        "quantiles": "median; nearest-rank p95/p99",
        "platform": platform.platform(),
        "python": platform.python_version(),
        "lua_command": args.lua,
        "lua_version": subprocess.check_output([str(executable), "-v"], text=True).strip(),
        "lua_executable_sha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
        "measurement_sources": {
            path.name: hashlib.sha256(path.read_bytes()).hexdigest()
            for path in (Path(__file__), script)
        },
        "packages": {name: str(path) for name, path in packages.items()},
        "inputs": identities, "roots": roots, "samples": samples, "summary": summaries,
    }
    with args.output.open("x", encoding="utf-8") as stream:
        json.dump(report, stream, ensure_ascii=False, sort_keys=True, indent=2)
        stream.write("\n")
    print(json.dumps(summaries, ensure_ascii=False, sort_keys=True, indent=2))


if __name__ == "__main__":
    main()
