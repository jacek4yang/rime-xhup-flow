#!/usr/bin/env python3
"""Run the same behavioral stubs on source and compiled Lua, plus real data loading.

This is not native librime or GUI acceptance. Native deployment/replay gates remain
separate and mandatory.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests/release"))
from check_generated_runtime_sources import MANIFEST, verify

PRODUCTION_TESTS = (
    "test_contract.lua", "test_quick_hint.lua", "test_context_ranker.lua",
    "test_ranker_stream.lua", "test_native_tail.lua", "test_full_span.lua",
    "test_session_memory.lua", "test_native_edge_index.lua", "test_joint_search.lua",
    "test_joint_readings.lua",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("--lua", default="lua5.4")
    args = parser.parse_args()
    package = args.package.resolve()
    verify(package, ROOT)
    manifest = json.loads((package / MANIFEST).read_text(encoding="utf-8"))
    modules = [item["module"] for item in manifest["artifacts"] if item["module"]]
    subprocess.run([args.lua, "tests/lua/check_compiled_modules.lua", str(package),
                    manifest["compiler"]["generator_version"], *modules], cwd=ROOT, check=True)

    oracle = json.loads((ROOT / "data/benchmarks/v1-quick-hint-values.json").read_text(encoding="utf-8"))
    historical = dict(line.split("\t") for line in (ROOT / oracle["source_identity_oracle"])
                      .read_text(encoding="utf-8").splitlines() if line and not line.startswith("#"))
    if historical[oracle["source_artifact"]] != oracle["source_artifact_sha256"]:
        raise ValueError("hint-value oracle is not bound to the frozen v1 release identity")
    values = subprocess.check_output([args.lua, "tests/lua/dump_hint_values.lua",
                                      str(package / oracle["source_artifact"])], cwd=ROOT)
    if (hashlib.sha256(values).hexdigest() != oracle["values_sha256"]
            or values.count(b"\n") != oracle["entries"]):
        raise ValueError("compiled hint mapping differs from the complete frozen v1 mapping")
    print(f"PASS frozen hint values: {oracle['entries']} entries; {oracle['values_sha256']}", flush=True)

    for profile in ("source", "compiled"):
        environment = os.environ.copy()
        environment.pop("XHUP_LUA_ROOT", None)
        environment.pop("LUA_PATH", None)
        if profile == "compiled":
            environment["XHUP_LUA_ROOT"] = str(package / "lua")
        for name in PRODUCTION_TESTS:
            print(f"== {profile}: {name} ==", flush=True)
            subprocess.run([args.lua, str(ROOT / "tests/lua" / name), str(package)], cwd=ROOT,
                           env=environment, check=True)
    print("PASS source/compiled behavioral suites; native and platform gates remain separate")


if __name__ == "__main__":
    main()
