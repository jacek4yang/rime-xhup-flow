#!/usr/bin/env python3
"""Execute publication preparation with real CLI/collector and synthetic remote bytes.

Only network and full-audit collection are substituted; this is not real platform
acceptance, signing, CI coverage, or a GitHub publication.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from check_workflow_shell import run_blocks

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/xhup-flow-rc-release.yml"
SOURCE = "0123456789abcdef0123456789abcdef01234567"
RC = "2.0.0-rc.3"
VERSION = "2.0.0"


class PublicationWorkflow(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="xhup-publication-")
        self.addCleanup(self.tmp.cleanup)
        self.work = Path(self.tmp.name)
        self.cli = os.environ["XHUP_CLI"]
        scripts = [s for _, _, s in run_blocks(WORKFLOW.read_text().splitlines())
                   if 'artifact_version="$VERSION"' in s]
        self.script, = scripts
        self.remote = self.work / "remote"
        self.names = self.payload(VERSION, self.remote)
        initial = self.command("init-acceptance", "--version", VERSION,
                               "--source-commit", SOURCE, "--artifacts-dir", str(self.remote))
        self.assertEqual(initial.returncode, 0, initial.stderr)
        self.manifest = json.loads(initial.stdout)
        self.manifest["accepted_rc"] = RC
        tested = {k: self.manifest[k] for k in ("version", "source_commit", "build_manifest_sha256")}
        for platform in self.manifest["platforms"]:
            platform.update(runtime="SYNTHETIC librime/Lua/frontend", evidence="SYNTHETIC, NOT ACCEPTANCE",
                            verified_at="2026-10-05T00:00:00Z", tested_build=dict(tested))
            platform["checks"] = dict.fromkeys(platform["checks"], "PASS")
        (self.work / "release").mkdir()
        self.proofs = self.work / "synthetic-proofs"
        self.proofs.mkdir()
        self.decision = dict(schema_version=2, policy="full-platform-rebuilt-release-v1", version=VERSION,
                             accepted_rc=RC, source_commit=SOURCE,
                             build_manifest_sha256=self.manifest["build_manifest_sha256"],
                             repository="fixture/repo", approved_by="fixture", approved_at="2026-10-05T00:00:00Z",
                             rationale="SYNTHETIC TEST ONLY", limitations=["Synthetic unsigned desktop bytes"],
                             ci_run=11, runtime_run=12, package_run=13)
        for label, run_id, workflow in (("ci", 11, "ci"), ("runtime", 12, "full-regression"),
                                         ("package", 13, "product-packaging")):
            run = dict(id=run_id, run_attempt=1, status="completed", conclusion="success", head_sha=SOURCE,
                       head_branch="main", path=f".github/workflows/{workflow}.yml", event="workflow_dispatch",
                       repository=dict(full_name="fixture/repo", owner=dict(login="fixture")))
            (self.proofs / f"{label}-run.json").write_text(json.dumps(run))
        jobs = [dict(name=name, status="completed", conclusion="success", head_sha=SOURCE, run_id=11)
                for name in ("Rust workspace", "trainer 前端", "librime runtime smoke", "原生冒烟(ubuntu-latest)",
                             "原生冒烟(macos-latest)", "原生冒烟(windows-latest)")]
        (self.proofs / "ci-jobs.json").write_text(json.dumps(dict(total_count=6, jobs=jobs)))
        (self.proofs / "coverage.json").write_text(json.dumps(dict(version=1, revision=SOURCE, run="12.1",
            plan_sha256="a" * 64, partitions_completed=16, rows=dict(static=1, extended=1, open=1000))))
        scripts_dir = self.work / "tests/release"
        scripts_dir.mkdir(parents=True)
        shutil.copyfile(ROOT / "tests/release/collect_qualification.py", scripts_dir / "collect_qualification.py")
        self.bin = self.work / "bin"
        self.bin.mkdir()
        cargo = self.bin / "cargo"
        cargo.write_text('#!/bin/bash\nset -eu\nwhile [[ "$1" != "--" ]]; do shift; done\nshift\nexec "$XHUP_CLI" "$@"\n')
        cargo.chmod(0o700)
        python = self.bin / "python3"
        python.write_text(f'#!{sys.executable}\nimport os,shutil,sys\n'
                          'if sys.argv[1] == "tests/release/collect_qualification.py" and "--decision" in sys.argv:\n'
                          ' shutil.copytree("synthetic-proofs", sys.argv[sys.argv.index("--out")+1])\n'
                          f'else: os.execv({sys.executable!r}, [{sys.executable!r}, *sys.argv[1:]])\n')
        python.chmod(0o700)
        gh = self.bin / "gh"
        gh.write_text(f'#!{sys.executable}\n' + '''import json,os,shutil,sys
from pathlib import Path
args=sys.argv[1:]
if args[:2] == ["release", "view"]:
 print(json.dumps({"isDraft":False,"isPrerelease":True}))
elif args[0] == "api" and "/actions/runs/" in args[1]:
 run=json.loads(Path("synthetic-proofs/package-run.json").read_text())
 if "RESOLVED_SOURCE" in os.environ: run["head_sha"]=os.environ["RESOLVED_SOURCE"]
 print(json.dumps(run))
elif args[0] == "api" and "/commits/" in args[1]:
 print("d"*40)
elif args[0] == "api" and "/compare/" in args[1]:
 print(os.environ.get("RELATION", "ahead"))
elif args[:2] == ["run", "download"]:
 root=Path(args[args.index("--dir")+1])
 for group in ("rime","windows","macos","linux","android","manifest"):
  (root/f"product-packaging-{group}").mkdir()
 for file in Path(os.environ["REMOTE"]).iterdir():
  group="rime" if file.suffix==".zip" else next((p for p in ("windows","macos","linux","android") if p in file.name),"manifest")
  shutil.copyfile(file,root/f"product-packaging-{group}"/file.name)
 if os.environ.get("DUPLICATE"):
  file=next((root/"product-packaging-rime").iterdir())
  shutil.copyfile(file,root/"product-packaging-windows"/file.name)
else: sys.exit(99)
''')
        gh.chmod(0o700)
        self.env = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}", XHUP_CLI=self.cli,
                        REMOTE=str(self.remote), GITHUB_SHA="b" * 40, RUNNER_TEMP=str(self.work),
                        GITHUB_OUTPUT=str(self.work / "output"), GH_REPO="fixture/repo", CORE=VERSION,
                        VERSION=VERSION, IS_RC="false", PUBLISH="true", PACKAGE_RUN="13", GITHUB_ACTOR="fixture")

    def command(self, *args):
        return subprocess.run([self.cli, *args], capture_output=True, text=True)

    def payload(self, version, directory):
        directory.mkdir()
        names = [f"xhup-flow-rime-v{version}.zip"]
        names += [f"xhup-flow-trainer-v{version}-{suffix}" for suffix in (
            "windows-x64-setup.exe", "windows-x64.msi", "macos-universal.dmg", "linux-amd64.deb",
            "linux-x86_64.rpm", "android-arm64.apk", "android-universal.apk")]
        names += ["SHA256SUMS.txt", "CANONICAL-SHA256SUMS.txt", "BUILD-INFO.txt"]
        for name in names:
            (directory / name).write_bytes(b"SYNTHETIC PAYLOAD, NOT AN INSTALLER")
        sealed = self.command("seal-build", "--version", version, "--source-commit", SOURCE,
                              "--artifacts-dir", str(directory))
        self.assertEqual(sealed.returncode, 0, sealed.stderr)
        return names

    def run_block(self):
        (self.work / f"release/acceptance-v{VERSION}.json").write_text(json.dumps(self.manifest))
        (self.work / f"release/qualification-v{VERSION}.json").write_text(json.dumps(self.decision))
        return subprocess.run(["bash", "-c", self.script], cwd=self.work, env=self.env,
                              capture_output=True, text=True)

    def rc(self):
        source = self.work / "rc-source"
        self.payload(RC, source)
        shutil.copytree(source, self.work / "artifacts")
        self.env.update(IS_RC="true", VERSION=RC, GITHUB_SHA=SOURCE, PACKAGE_RUN="")

    def test_stable_uses_fresh_version_and_build_source_not_receipt_commit(self):
        result = self.run_block()
        self.assertEqual(result.returncode, 0, result.stderr)
        output = dict(line.split("=", 1) for line in (self.work / "output").read_text().splitlines())
        self.assertEqual(output["source_commit"], SOURCE)
        self.assertEqual(output["artifact_version"], VERSION)
        self.assertEqual(output["acceptance_windows"], "PASS")
        self.assertEqual(json.loads((self.work / "artifacts/ACCEPTANCE.json").read_text()), self.manifest)

    def test_unverified_is_never_waived(self):
        self.manifest["platforms"][0]["checks"]["clean_install"] = "UNVERIFIED"
        self.assertNotEqual(self.run_block().returncode, 0)
        self.assertFalse((self.work / "artifacts/QUALIFICATION.json").exists())

    def test_old_policy_is_not_authority_for_new_publication(self):
        self.decision.update(schema_version=1, policy="runtime-qualified-user-platform-testing-v1",
                             user_testing_platforms=["windows"])
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_missing_decision_run_cannot_be_replaced_by_other_build(self):
        self.decision["package_run"] = 14
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_current_source_proofs_are_required(self):
        path = self.proofs / "runtime-run.json"
        run = json.loads(path.read_text()); run["head_sha"] = "c" * 40
        path.write_text(json.dumps(run))
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_changed_download_is_blocked(self):
        (self.remote / self.names[0]).write_bytes(b"tampered")
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_independently_resolved_build_source_mismatch_is_blocked(self):
        self.env["RESOLVED_SOURCE"] = "c" * 40
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_duplicate_download_group_file_is_not_silently_overwritten(self):
        self.env["DUPLICATE"] = "1"
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_non_packaging_run_is_rejected(self):
        path = self.proofs / "package-run.json"
        run = json.loads(path.read_text()); run["path"] = ".github/workflows/xhup-flow-rc-release.yml"
        path.write_text(json.dumps(run))
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_all_na_is_blocked(self):
        for platform in self.manifest["platforms"]:
            platform["checks"] = dict.fromkeys(platform["checks"], "N/A")
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_malformed_predecessor_is_rejected(self):
        self.manifest["accepted_rc"] = "unrelated-rc.foo"
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_rc_source_cannot_be_relabelled_as_stable_source(self):
        self.env["RELATION"] = "identical"
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_unsealed_payload_is_rejected(self):
        (self.remote / "BUILD-MANIFEST.json").unlink()
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_rc_receipt_does_not_copy_old_pass_evidence(self):
        self.rc()
        result = self.run_block()
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = json.loads((self.work / "artifacts/ACCEPTANCE.json").read_text())
        self.assertEqual(receipt["version"], RC)
        self.assertEqual(receipt["source_commit"], SOURCE)
        for platform in receipt["platforms"]:
            self.assertIsNone(platform["tested_build"])
            self.assertIsNone(platform["evidence"])
            for state in platform["checks"].values():
                self.assertEqual(state, "UNVERIFIED")

    def test_rc_unknown_extra_file_blocks_publication(self):
        self.rc()
        (self.work / "artifacts/unexpected.txt").write_text("must not be published")
        self.assertNotEqual(self.run_block().returncode, 0)
        self.assertFalse((self.work / "artifacts/ACCEPTANCE.json").exists())

    def test_rehearsal_does_not_create_promotable_manifest(self):
        self.rc()
        self.env["PUBLISH"] = "false"
        (self.work / "artifacts/BUILD-MANIFEST.json").unlink()
        self.assertEqual(self.run_block().returncode, 0)
        self.assertFalse((self.work / "artifacts/BUILD-MANIFEST.json").exists())
        self.assertFalse((self.work / "artifacts/ACCEPTANCE.json").exists())


if __name__ == "__main__":
    unittest.main()
