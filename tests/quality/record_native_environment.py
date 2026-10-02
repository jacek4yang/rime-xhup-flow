#!/usr/bin/env python3
"""Record explicit artifact/runtime identities; no raw corpus or host/user names."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("lua_plugin", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--package-source", required=True)
    args = parser.parse_args()
    commit = subprocess.check_output(["git", "-C", ROOT, "rev-parse", "--verify",
                                     args.package_source+"^{commit}"], text=True).strip()
    subprocess.run(["python3", str(ROOT/"tests/release/check_generated_runtime_sources.py"),
                    str(args.package)], check=True)
    artifacts = {}
    for path in sorted(args.package.rglob("*")):
        if path.is_symlink():
            raise ValueError("symlink in evaluation package")
        if path.is_file():
            artifacts[path.relative_to(args.package).as_posix()] = {
                "bytes": path.stat().st_size, "sha256": digest(path)}
    report = {
        "schema": 1, "declared_package_generation_source_commit": commit,
        "source_claim_limit": "caller-declared generation revision; byte gate binds current runtime sources, not an independent full rebuild proof",
        "runtime_source_binding": "8 Lua modules and 2 schemas match the evaluating checkout",
        "artifacts": artifacts,
        "harness_sources": {name: digest(ROOT/name) for name in (
            "tests/librime/runtime_corpus.c", "tests/librime/runtime_replay.c",
            "tests/librime/run-flow-audit.sh", "tests/quality/corpus_native_only.lua")},
        "librime_pkg_config_version": subprocess.check_output(["pkg-config","--modversion","rime"], text=True).strip(),
        "lua_plugin_source_commit": "68f9c364a2d25a04c7d4794981d7c796b05ab627",
        "lua_plugin_binary_sha256": digest(args.lua_plugin),
        "lua_plugin_installation": "bwrap private replacement of system plugin; NOT concurrent LD_PRELOAD",
        "compiler": subprocess.check_output(["cc","--version"], text=True).splitlines()[0],
        "harness_flags": "-O2 -Wall -Wextra -Werror -std=c11",
        "system": platform.system(), "kernel": platform.release(), "architecture": platform.machine(),
        "load_warning": "concurrent exhaustive native audit and workspace tests; timing is NOT an idle paired performance comparison",
        "learning": "compiled flow/learn/flow_readonly user dictionaries verified disabled by native harness",
    }
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2)+"\n")


if __name__ == "__main__":
    main()
