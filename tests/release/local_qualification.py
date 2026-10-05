#!/usr/bin/env python3
"""Complete local qualification; never represents skipped cloud tests as passed.

The owner-operated host is the trust boundary, just as the runner is for CI.
This receipt is source/log-bound evidence, not a signature or proof against a
compromised host. Cloud packaging must separately bind the exact same source.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import tempfile
import subprocess

ROOT = Path(__file__).resolve().parents[2]
POLICY = "local-runtime-qualified-user-platform-testing-v1"
BASE_CHECKS = {"rust", "security", "python", "frontend", "native-prepare", "native-controls", "native-collect"}
CHECKS = BASE_CHECKS | {f"native-shard-{i}" for i in range(16)}


def sha(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def inventory(directory):
    result = {}
    for path in sorted(directory.rglob("*")):
        if path.is_symlink():
            raise ValueError("evidence must not contain symlinks")
        if path.is_file() and path != directory / "LOCAL-VALIDATION.json":
            result[path.relative_to(directory).as_posix()] = sha(path)
    return result


def verify(directory, source):
    report = json.loads((directory / "LOCAL-VALIDATION.json").read_text())
    if (report["schema_version"], report["policy"], report["source_commit"], report["result"]) != (1, POLICY, source, "PASS"):
        raise ValueError("wrong qualification policy/source/result")
    if not re.fullmatch(r"[0-9a-f]{40}", source):
        raise ValueError("expected full independent source SHA")
    if sorted(report["user_platform_testing"]) != ["android", "linux", "macos", "windows"]:
        raise ValueError("platform pending scope changed")
    checks = report["checks"]
    if len(checks) != len(CHECKS) or {c["name"] for c in checks} != CHECKS:
        raise ValueError("missing/duplicate qualification checks")
    for check in checks:
        if type(check["exit_code"]) is not int or check["exit_code"] != 0:
            raise ValueError(f"failed check: {check['name']}")
        log = directory / f"{check['name']}.log"
        if not log.is_file() or sha(log) != check["log_sha256"]:
            raise ValueError(f"changed log: {check['name']}")
    if report["files"] != inventory(directory):
        raise ValueError("evidence inventory changed")
    with tempfile.TemporaryDirectory(prefix="xhup-local-recollect-") as temp:
        collected = Path(temp)
        for path in (directory / "native-results").iterdir():
            if path.name != "coverage.json":
                shutil.copy2(path, collected / path.name)
        subprocess.run(["python3", str(ROOT / "tests/librime/audit_shards.py"), "collect",
                        "--root", str(directory / "native-input"), "--out", str(collected),
                        "--revision", source, "--run", report["run"]], check=True, cwd=ROOT, stdout=subprocess.DEVNULL)
        original = json.loads((directory / "native-results/coverage.json").read_text())
        if json.loads((collected / "coverage.json").read_text()) != original:
            raise ValueError("recollected evidence differs")
    if report["files"] != inventory(directory):
        raise ValueError("evidence changed during recollection")
    return report


def verify_build(directory, source, artifacts, version):
    """Bind local evidence to exact cloud-built RC bytes; no rebuild or fake PASS."""
    report = verify(directory, source)
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+-rc\.[1-9][0-9]*", version):
        raise ValueError("expected RC artifact version; stable promotion preserves embedded version")
    expected = {f"xhup-flow-rime-v{version}.zip", "SHA256SUMS.txt", "CANONICAL-SHA256SUMS.txt", "BUILD-INFO.txt"}
    for suffix in ("windows-x64-setup.exe", "windows-x64.msi", "macos-universal.dmg", "linux-amd64.deb", "linux-x86_64.rpm", "android-arm64.apk", "android-universal.apk"):
        expected.add(f"xhup-flow-trainer-v{version}-{suffix}")
    build = json.loads((artifacts / "BUILD-MANIFEST.json").read_text())
    if (build["schema_version"], build["source_commit"], build["version"]) != (1, source, version):
        raise ValueError("cloud build source/version mismatch")
    records = build["artifacts"]
    if len(records) != len(expected) or {a["name"] for a in records} != expected:
        raise ValueError("cloud artifact inventory mismatch")
    actual = {p.name for p in artifacts.iterdir()}
    if actual not in (expected | {"BUILD-MANIFEST.json"}, expected | {"BUILD-MANIFEST.json", "ACCEPTANCE.json"}):
        raise ValueError("missing or unexpected downloaded artifacts")
    for item in records:
        path = artifacts / item["name"]
        if path.is_symlink() or not path.is_file() or not path.stat().st_size or sha(path) != item["sha256"]:
            raise ValueError(f"cloud artifact bytes mismatch: {item['name']}")
    return dict(policy=POLICY, source_commit=source, artifact_version=version,
                local_validation_sha256=sha(directory / "LOCAL-VALIDATION.json"),
                build_manifest_sha256=sha(artifacts / "BUILD-MANIFEST.json"),
                user_platform_testing=report["user_platform_testing"])


def run(args):
    source = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    def clean():
        if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT).strip():
            raise ValueError("qualification requires a clean checkout")
        if subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() != source:
            raise ValueError("source changed during qualification")
    clean()
    output = args.out.resolve()
    target = args.target.resolve()
    if output.is_relative_to(ROOT) or target.is_relative_to(output) or output.is_relative_to(target):
        raise ValueError("evidence and build target must be separate, outside checkout")
    output.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS=str(args.jobs))
    env["PATH"] = str(target / "debug") + os.pathsep + env.get("PATH", "")
    env["XHUP_CLI"] = str(target / "debug/xhup-cli")
    env["EVIDENCE"] = str(output)
    env["PLUGIN"] = str(args.plugin.resolve())
    env["SOURCE"] = source
    env["AUDIT_RUN"] = "local-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    checks = []
    def step(name, command, native=False):
        log = output / f"{name}.log"
        step_env = dict(env)
        if native:
            step_env["LD_PRELOAD"] = env["PLUGIN"]
        started = datetime.now(timezone.utc).isoformat()
        with log.open("wb") as stream:
            result = subprocess.run(["bash", "-euo", "pipefail", "-c", command], cwd=ROOT,
                                    env=step_env, stdout=stream, stderr=subprocess.STDOUT)
        record = dict(name=name, command=command, exit_code=result.returncode,
                      started_at=started, log_sha256=sha(log))
        checks.append(record)
        print(f"{name}: exit={result.returncode}", flush=True)
        if result.returncode:
            raise RuntimeError(f"{name} failed; see {log}")
    report = dict(schema_version=1, policy=POLICY, source_commit=source, run=env["AUDIT_RUN"],
                  result="FAIL", checks=checks, user_platform_testing=["windows", "linux", "macos", "android"])
    try:
        step("rust", """rustc -Vv
cargo fmt --all -- --check
cargo rustc -p glib --lib --locked -- -D warnings
cargo rustc -p gtk3-macros --lib --locked -- -D warnings
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build -p xhup-cli -p xhup-analyzer --bins --locked""")
        step("security", """python3 tests/security/check_glib_backport.py
python3 tests/security/check_rust_maintenance.py
python3 tests/security/check_macro_diagnostic.py
cargo audit --deny warnings""")
        step("python", """for directory in tests/security tests/release tests/quality tests/librime data/hanzi; do
  python3 -m unittest discover -s "$directory" -p 'test_*.py'
done
python3 tests/release/check_workflow_shell.py""")
        step("frontend", """node --version
pnpm --version
pnpm install --frozen-lockfile
pnpm --filter trainer build
pnpm -r typecheck
pnpm -r test
pnpm --filter miniapp build:weapp
node tests/security/frontend-audit.cjs
mkdir "$EVIDENCE/frontend-security"
cp artifacts/security/pnpm-audit.json artifacts/security/frontend-mitigation.json "$EVIDENCE/frontend-security/"
node --test tests/security/*.test.cjs""")
        step("native-prepare", """mkdir "$EVIDENCE/native-input"
xhup-cli generate rime --output "$EVIDENCE/native-input/package"
python3 tests/release/check_distribution_sources.py --depfile "$CARGO_TARGET_DIR/debug/xhup-cli.d"
python3 tests/release/check_generated_runtime_sources.py "$EVIDENCE/native-input/package"
python3 tests/release/rime_archive.py verify "$EVIDENCE/native-input/package"
static-shortcut-audit --dump-static-menu-manifest "$EVIDENCE/native-input/static.manifest"
for kind in extended open; do
  if [[ "$kind" == extended ]]; then option=extended-reachability; else option=open-composition; fi
  static-shortcut-audit "--dump-$option-manifest" "$EVIDENCE/native-input/$kind.manifest"
  static-shortcut-audit "--dump-$option-manifest" "$EVIDENCE/$kind.second.manifest"
  cmp "$EVIDENCE/native-input/$kind.manifest" "$EVIDENCE/$kind.second.manifest"
done
cp "$XHUP_CLI" "$EVIDENCE/native-input/xhup-cli"
cp "$PLUGIN" "$EVIDENCE/native-input/librime-lua.so"
python3 tests/librime/audit_shards.py prepare --root "$EVIDENCE/native-input" --revision "$SOURCE" --run "$AUDIT_RUN" --count 16""")
        step("native-controls", """pkg-config --modversion rime
lua5.4 -v
package="$EVIDENCE/native-input/package"
for script in run-exclusive-schema run-runtime-smoke run-deploy-audit run-ime-controls run-lua-audit run-context-ranker-audit; do
  bash "tests/librime/$script.sh" "$package"
done
xhup-cli doctor --user-data-dir "$package""", native=True)
        with ThreadPoolExecutor(max_workers=args.jobs) as executor:
            futures = [executor.submit(step, f"native-shard-{i}",
                'python3 tests/librime/audit_shards.py run --root "$EVIDENCE/native-input" --out "$EVIDENCE/native-results" '
                f'--revision "$SOURCE" --run "$AUDIT_RUN" --index {i}', True) for i in range(16)]
            for future in futures:
                future.result()
        step("native-collect", 'python3 tests/librime/audit_shards.py collect --root "$EVIDENCE/native-input" '
             '--out "$EVIDENCE/native-results" --revision "$SOURCE" --run "$AUDIT_RUN"')
        clean()
        report["result"] = "PASS"
    finally:
        report["finished_at"] = datetime.now(timezone.utc).isoformat()
        report["files"] = inventory(output)
        (output / "LOCAL-VALIDATION.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    verify(output, source)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    execute = sub.add_parser("run")
    execute.add_argument("--out", type=Path, required=True)
    execute.add_argument("--target", type=Path, required=True)
    execute.add_argument("--plugin", type=Path, required=True)
    execute.add_argument("--jobs", type=int, choices=range(1, 17), default=8)
    check = sub.add_parser("verify")
    check.add_argument("--out", type=Path, required=True)
    check.add_argument("--source", required=True)
    build = sub.add_parser("verify-build")
    build.add_argument("--out", type=Path, required=True)
    build.add_argument("--source", required=True)
    build.add_argument("--artifacts", type=Path, required=True)
    build.add_argument("--artifact-version", required=True)
    args = parser.parse_args()
    if args.command == "run":
        run(args)
    elif args.command == "verify-build":
        print(json.dumps(verify_build(args.out.resolve(), args.source, args.artifacts.resolve(), args.artifact_version), indent=2))
    else:
        verify(args.out.resolve(), args.source)
