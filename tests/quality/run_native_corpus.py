#!/usr/bin/env python3
"""Replay a hash-bound public corpus on an isolated, already deployed test package.

Never pass a real user's Rime directory. Rank misses remain observations, not PASS;
this command enforces native input/commit invariants, not quality or GUI acceptance.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ("tests/quality/run_native_corpus.py", "tests/librime/runtime_corpus.c",
           "tests/librime/runtime_replay.c", "tests/librime/candidate_match.cc",
           "tests/librime/candidate_match.h", "tests/librime/runtime_module.h",
           "tests/librime/profile_runtime.h")


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--deployed", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--fixture-sha256", required=True)
    parser.add_argument("--plugin", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--joint-decoder", choices=("on", "off"), required=True)
    parser.add_argument("--shared", type=Path, default=Path("/usr/share/rime-data"))
    args = parser.parse_args()
    fixture, deployed, plugin, output = (path.resolve() for path in
                                        (args.fixture, args.deployed, args.plugin, args.output))
    if digest(fixture) != args.fixture_sha256:
        raise ValueError("frozen fixture SHA256 mismatch")
    if not (deployed / "build/xhup_flow.schema.yaml").is_file() or not plugin.is_file():
        raise ValueError("missing deployed test package or plugin")
    if any(path.is_symlink() for path in deployed.rglob("*")):
        raise ValueError("test deployment must not contain symlinks")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    for name in SOURCES:
        committed = subprocess.check_output(["git", "show", f"{revision}:{name}"], cwd=ROOT)
        if hashlib.sha256(committed).hexdigest() != digest(ROOT / name):
            raise ValueError(f"commit native corpus harness first: {name}")
    output.mkdir(parents=True, exist_ok=False)
    user = output / "deployed"
    shutil.copytree(deployed, user, ignore=shutil.ignore_patterns("*.userdb*", "user.yaml", "sync"))
    flags = ["-O2", "-Wall", "-Wextra", "-Werror"]
    cflags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "rime"], text=True))
    libs = shlex.split(subprocess.check_output(["pkg-config", "--libs", "rime"], text=True))
    commands = [
        ["cc", *flags, "-std=c11", "-c", "tests/librime/runtime_corpus.c", *cflags,
         "-o", str(output / "corpus.o")],
        ["c++", *flags, "-std=c++17", "tests/librime/candidate_match.cc", str(output / "corpus.o"),
         *cflags, *libs, "-ldl", "-o", str(output / "corpus")],
    ]
    for command in commands:
        subprocess.run(command, cwd=ROOT, check=True)
    command = [str(output / "corpus"), str(user), str(args.shared.resolve()), str(fixture), "planner"]
    provenance = {
        "harness_commit": revision, "source_sha256": {name: digest(ROOT / name) for name in SOURCES},
        "fixture_sha256": args.fixture_sha256, "plugin_sha256": digest(plugin),
        "binary_sha256": digest(output / "corpus"), "build_commands": commands, "command": command,
        "joint_decoder": args.joint_decoder, "quick_hint": False, "learning": False,
        "rank_contract": "complete_raw_span_v1", "gui_acceptance": False,
        "timing_scope": "observational corpus run, not paired performance qualification",
        "platform": platform.platform(), "cpu_affinity": sorted(os.sched_getaffinity(0)),
        "compiler": subprocess.check_output(["cc", "--version"], text=True),
        "rime_version": subprocess.check_output(["pkg-config", "--modversion", "rime"], text=True).strip(),
        "deployment_sha256": {path.relative_to(user).as_posix(): digest(path)
                              for path in sorted(user.rglob("*")) if path.is_file()},
    }
    (output / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
    started = time.monotonic()
    with (output / "trace.jsonl").open("w") as stdout, (output / "stderr.log").open("w") as stderr:
        result = subprocess.run(command, cwd=ROOT, stdout=stdout, stderr=stderr,
                                env=dict(os.environ, XHUP_LUA_LIBRARY=str(plugin),
                                         XHUP_JOINT_DECODER=args.joint_decoder))
    databases = [path.name for path in user.glob("*.userdb*")]
    report = {"returncode": result.returncode, "elapsed_seconds": time.monotonic() - started,
              "unexpected_user_databases": databases, "output": str(output)}
    (output / "run.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report), flush=True)
    if databases:
        raise RuntimeError("learning-off corpus replay created a user database")
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
