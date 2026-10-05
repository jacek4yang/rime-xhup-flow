#!/usr/bin/env python3
"""Fetch authenticated run proofs and independently recollect all native receipts.

Run only inside the publishing workflow using its GH_REPO/GITHUB_ACTOR context.
Never execute binaries from the downloaded artifact. CLI verification remains a
separate mandatory step; this collector alone does not authorize publication.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


def gh(*args):
    return subprocess.check_output(["gh", *args], text=True)


def collect(decision, destination, source):
    repo = os.environ["GH_REPO"]
    actor = os.environ["GITHUB_ACTOR"]
    if not re.fullmatch(r"[0-9a-f]{40}", source):
        raise ValueError("invalid independently resolved source")
    if decision["repository"] != repo or decision["approved_by"] != actor:
        raise ValueError("decision does not match independent workflow context")
    if decision["source_commit"] != source:
        raise ValueError("decision source mismatch")
    owner = json.loads(gh("api", f"repos/{repo}"))["owner"]["login"]
    if actor != owner:
        raise ValueError("only the repository owner can select this policy")
    destination.mkdir(exist_ok=False)
    runs = {}
    for label in ("ci", "runtime", "package"):
        run_id = decision[f"{label}_run"]
        if type(run_id) is not int or run_id <= 0:
            raise ValueError("run IDs must be positive integers")
        run = json.loads(gh("api", f"repos/{repo}/actions/runs/{run_id}"))
        if (run["status"], run["conclusion"], run["head_sha"], run["head_branch"]) != (
                "completed", "success", source, "main"):
            raise ValueError(f"{label} run is not a successful same-source main run")
        runs[label] = run
        (destination / f"{label}-run.json").write_text(json.dumps(run), encoding="utf-8")
    ci = runs["ci"]
    jobs = gh("api", f"repos/{repo}/actions/runs/{ci['id']}/attempts/{ci['run_attempt']}/jobs?per_page=100")
    (destination / "ci-jobs.json").write_text(jobs, encoding="utf-8")
    runtime = runs["runtime"]
    with tempfile.TemporaryDirectory(prefix="xhup-release-coverage-") as temp:
        root = Path(temp)
        gh("run", "download", str(runtime["id"]), "--repo", repo,
           "--pattern", "native-audit-*", "--dir", str(root))
        results = root / "merged-results"
        results.mkdir()
        expected = {f"native-audit-result-{i}" for i in range(16)}
        actual = {p.name for p in root.glob("native-audit-result-*")}
        if actual != expected:
            raise ValueError("missing or unexpected native partitions")
        for name in sorted(expected):
            for file in (root / name).iterdir():
                if not file.is_file() or file.is_symlink() or (results / file.name).exists():
                    raise ValueError("unexpected or duplicate receipt/log")
                file.rename(results / file.name)
        subprocess.run([
            "python3", str(Path(__file__).parents[1] / "librime/audit_shards.py"),
            "collect", "--root", str(root / "native-audit-input"), "--out", str(results),
            "--revision", source, "--run", f"{runtime['id']}.{runtime['run_attempt']}",
        ], check=True)
        coverage = (results / "coverage.json").read_bytes()
        if json.loads(coverage) != json.loads((root / "native-audit-complete-coverage/coverage.json").read_bytes()):
            raise ValueError("independent coverage differs from published collector")
        (destination / "coverage.json").write_bytes(coverage)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--decision", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--source", required=True)
    args = parser.parse_args()
    collect(json.loads(args.decision.read_text(encoding="utf-8")), args.out, args.source)
