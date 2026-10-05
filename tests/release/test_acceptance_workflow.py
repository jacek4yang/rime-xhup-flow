#!/usr/bin/env python3
"""Execute the actual prepare run block with fake GitHub and the real CLI.

No network, real release, signing, or platform acceptance is performed.
Cargo is only a shim to the already built CLI. GH supplies isolated fixture bytes.
"""
import hashlib
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


class PromotionWorkflow(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="xhup-promotion-")
        self.addCleanup(self.tmp.cleanup)
        self.work = Path(self.tmp.name)
        self.remote = self.work / "remote"
        self.remote.mkdir()
        self.cli = os.environ["XHUP_CLI"]
        self.lines = WORKFLOW.read_text().splitlines()
        scripts = [s for _, _, s in run_blocks(self.lines)
                   if 'artifact_version="$VERSION"' in s]
        self.assertEqual(len(scripts), 1)
        self.script = scripts[0]
        self.names = [f"xhup-flow-rime-v{RC}.zip"]
        self.names += [f"xhup-flow-trainer-v{RC}-{suffix}" for suffix in (
            "windows-x64-setup.exe", "windows-x64.msi", "macos-universal.dmg",
            "linux-amd64.deb", "linux-x86_64.rpm", "android-arm64.apk",
            "android-universal.apk")]
        self.names += ["SHA256SUMS.txt", "CANONICAL-SHA256SUMS.txt", "BUILD-INFO.txt"]
        for name in self.names:
            (self.remote / name).write_bytes(b"synthetic payload")
        result = subprocess.run([self.cli, "seal-build", "--version", RC,
                                 "--source-commit", SOURCE, "--artifacts-dir",
                                 str(self.remote)], capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        raw = (self.remote / "BUILD-MANIFEST.json").read_bytes()
        build = json.loads(raw)
        keys = ["clean_install", "upgrade_from_v1", "schema_deployment", "static_smoke",
                "flow_sentence_input", "learning_persistence", "oov_reachability",
                "context_ranker_neutrality", "joint_decoder_neutrality", "static_fallback",
                "trainer_lifecycle", "privacy"]
        self.manifest = dict(schema_version=2, version="2.0.0", accepted_rc=RC,
                             source_commit=SOURCE, build_manifest_sha256=hashlib.sha256(raw).hexdigest(),
                             artifacts=build["artifacts"], platforms=[
                                 dict(platform=p, frontend="fixture 1", os="fixture OS",
                                      architecture="fixture", runtime="fixture librime/Lua 1",
                                      artifact=self.names[0], checks=dict.fromkeys(keys, "PASS"),
                                      evidence="synthetic, not actual acceptance",
                                      verified_at="2026-09-27T00:00:00Z", exemptions={})
                                 for p in ["windows", "linux", "macos", "android"]])
        (self.work / "release").mkdir()
        self.bin = self.work / "bin"
        self.bin.mkdir()
        # A call log verifies stable never invokes seal-build or product compilation.
        cargo = self.bin / "cargo"
        cargo.write_text('#!/bin/bash\nset -eu\nprintf "%s\\n" "$*" >> "$CALL_LOG"\n'
                         'while [[ "$1" != "--" ]]; do shift; done\nshift\n'
                         'exec "$XHUP_CLI" "$@"\n')
        cargo.chmod(0o700)
        gh = self.bin / "gh"
        gh.write_text('#!/bin/bash\nset -eu\nprintf "%s\\n" "$*" >> "$GH_LOG"\n'
                      'case "$1 $2" in\n'
                      '  "release view") printf "%s\\n" \'{"isDraft":false,"isPrerelease":true}\' ;;\n'
                      '  "release download") cp "$REMOTE/"* artifacts/ ;;\n'
                      '  api*) printf "%s\\n" "$RESOLVED_SOURCE" ;;\n'
                      '  *) exit 99 ;;\nesac\n')
        gh.chmod(0o700)
        self.env = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}",
                        XHUP_CLI=self.cli, CALL_LOG=str(self.work / "cargo.log"),
                        GH_LOG=str(self.work / "gh.log"), REMOTE=str(self.remote),
                        RESOLVED_SOURCE=SOURCE, GITHUB_SHA="b" * 40,
                        GITHUB_OUTPUT=str(self.work / "output"), GH_REPO="fixture/repo",
                        CORE="2.0.0", VERSION="2.0.0", IS_RC="false", PUBLISH="true",
                        QUALIFICATION="full-platform", GITHUB_ACTOR="fixture")

    def run_block(self):
        (self.work / "release/acceptance-v2.0.0.json").write_text(json.dumps(self.manifest))
        return subprocess.run(["bash", "-c", self.script], cwd=self.work, env=self.env,
                              capture_output=True, text=True)

    def test_stable_promotes_exact_bytes_and_rc_source(self):
        result = self.run_block()
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in self.names + ["BUILD-MANIFEST.json"]:
            self.assertEqual((self.work / "artifacts" / name).read_bytes(),
                             (self.remote / name).read_bytes())
        self.assertIn(f"source_commit={SOURCE}", (self.work / "output").read_text())
        self.assertIn(f"artifact_version={RC}", (self.work / "output").read_text())
        self.assertNotIn("seal-build", (self.work / "cargo.log").read_text())
        self.assertEqual(json.loads((self.work / "artifacts/ACCEPTANCE.json").read_text()), self.manifest)

    def test_default_mode_still_rejects_unverified(self):
        self.manifest["platforms"][0]["checks"]["clean_install"] = "UNVERIFIED"
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_qualified_mode_requires_real_collector_and_decision(self):
        self.env["QUALIFICATION"] = "runtime-qualified-user-platform-testing-v1"
        self.manifest["platforms"][0]["checks"]["clean_install"] = "UNVERIFIED"
        self.assertNotEqual(self.run_block().returncode, 0)
        self.assertFalse((self.work / "artifacts/QUALIFICATION.json").exists())

    def qualified_fixture(self):
        # Replace only network/receipt collection with synthetic proof bytes;
        # the real CLI still verifies policy + exact RC payloads in the run block.
        self.env["QUALIFICATION"] = "runtime-qualified-user-platform-testing-v1"
        self.manifest["platforms"][0]["checks"]["clean_install"] = "UNVERIFIED"
        decision = dict(schema_version=1, policy=self.env["QUALIFICATION"], version="2.0.0",
                        accepted_rc=RC, source_commit=SOURCE,
                        build_manifest_sha256=self.manifest["build_manifest_sha256"],
                        repository="fixture/repo", approved_by="fixture", approved_at="2026-10-05T00:00:00Z",
                        rationale="SYNTHETIC TEST", limitations=["Pending user testing"],
                        user_testing_platforms=["windows"], ci_run=11, runtime_run=12, package_run=13)
        (self.work / "release/qualification-v2.0.0.json").write_text(json.dumps(decision))
        proofs = self.work / "synthetic-proofs"
        proofs.mkdir()
        for label, run_id, workflow in [("ci", 11, "ci"), ("runtime", 12, "full-regression"),
                                         ("package", 13, "xhup-flow-rc-release")]:
            run = dict(id=run_id, run_attempt=1, status="completed", conclusion="success",
                       head_sha=SOURCE, head_branch="main", path=f".github/workflows/{workflow}.yml",
                       event="workflow_dispatch", repository=dict(full_name="fixture/repo", owner=dict(login="fixture")))
            (proofs / f"{label}-run.json").write_text(json.dumps(run))
        jobs = [dict(name=name, status="completed", conclusion="success", head_sha=SOURCE, run_id=11)
                for name in ["Rust workspace", "trainer 前端", "librime runtime smoke",
                             "原生冒烟(ubuntu-latest)", "原生冒烟(macos-latest)", "原生冒烟(windows-latest)"]]
        (proofs / "ci-jobs.json").write_text(json.dumps(dict(total_count=6, jobs=jobs)))
        (proofs / "coverage.json").write_text(json.dumps(dict(version=1, revision=SOURCE,
            run="12.1", plan_sha256="a" * 64, partitions_completed=16, rows=dict(static=1, extended=1, open=1000))))
        shim = self.bin / "python3"
        shim.write_text(f'#!{sys.executable}\nimport os,shutil,sys\n'
                        'if sys.argv[1] == "tests/release/collect_qualification.py":\n'
                        ' shutil.copytree("synthetic-proofs", "qualification-proofs")\n'
                        f'else: os.execv({sys.executable!r}, [{sys.executable!r}, *sys.argv[1:]])\n')
        shim.chmod(0o700)
        return proofs

    def test_qualified_scope_promotes_same_bytes_without_turning_unverified_to_pass(self):
        self.qualified_fixture()
        result = self.run_block()
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in self.names + ["BUILD-MANIFEST.json"]:
            self.assertEqual((self.work / "artifacts" / name).read_bytes(), (self.remote / name).read_bytes())
        self.assertEqual(json.loads((self.work / "artifacts/ACCEPTANCE.json").read_text()), self.manifest)
        self.assertTrue((self.work / "artifacts/QUALIFICATION-PROOFS.tar.gz").is_file())
        self.assertIn("--runtime-qualification", (self.work / "cargo.log").read_text())
        self.assertNotIn("seal-build", (self.work / "cargo.log").read_text())

    def test_qualified_scope_does_not_bypass_run_proof_verification(self):
        proofs = self.qualified_fixture()
        path = proofs / "runtime-run.json"
        run = json.loads(path.read_text())
        run["head_sha"] = "c" * 40
        path.write_text(json.dumps(run))
        self.assertNotEqual(self.run_block().returncode, 0)
        self.assertFalse((self.work / "artifacts/QUALIFICATION.json").exists())

    def test_changed_download_is_blocked(self):
        (self.remote / self.names[0]).write_bytes(b"tampered")
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_real_tag_source_mismatch_is_blocked(self):
        self.env["RESOLVED_SOURCE"] = "c" * 40
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_all_na_is_blocked(self):
        for p in self.manifest["platforms"]:
            p["checks"] = dict.fromkeys(p["checks"], "N/A")
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_malformed_rc_stops_before_download(self):
        self.manifest["accepted_rc"] = "unrelated-rc.foo"
        self.assertNotEqual(self.run_block().returncode, 0)
        self.assertFalse((self.work / "gh.log").exists())

    def test_legacy_rc_without_sealed_manifest_is_blocked(self):
        (self.remote / "BUILD-MANIFEST.json").unlink()
        self.assertNotEqual(self.run_block().returncode, 0)

    def test_rc_seals_only_current_build_not_old_acceptance(self):
        self.env.update(IS_RC="true", VERSION=RC, GITHUB_SHA=SOURCE)
        (self.remote / "BUILD-MANIFEST.json").unlink()
        shutil.copytree(self.remote, self.work / "artifacts")
        result = self.run_block()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("seal-build", (self.work / "cargo.log").read_text())
        self.assertFalse((self.work / "gh.log").exists())

    def test_rime_producer_inventory_is_sealable(self):
        packaging = (ROOT / ".github/workflows/product-packaging.yml").read_text().splitlines()
        blocks = [script for _, _, script in run_blocks(packaging)
                  if "rime_archive.py create" in script]
        self.assertEqual(len(blocks), 1)
        script = blocks[0].replace("${{ steps.meta.outputs.rime_version }}", RC)
        # Only the archive operation is substituted; execute the actual producer
        # shell so generated sidecars and uploaded directory inventory are real.
        python = self.bin / "python3"
        python.write_text('#!/bin/bash\nset -eu\n'
                          '[[ "$1" == tests/release/rime_archive.py ]]\n'
                          'case "$2" in\n'
                          ' create) printf "synthetic rime archive" > "$4" ;;\n'
                          ' check) test -s "$4" ;;\n'
                          ' *) exit 99 ;;\nesac\n')
        python.chmod(0o700)
        produced = subprocess.run(["bash", "-c", script], cwd=self.work,
                                  env=self.env, text=True, capture_output=True)
        self.assertEqual(produced.returncode, 0, produced.stderr)
        directory = self.work / "artifacts"
        for name in self.names[1:]:
            shutil.copyfile(self.remote / name, directory / name)
        sealed = subprocess.run([self.cli, "seal-build", "--version", RC,
                                 "--source-commit", SOURCE, "--artifacts-dir", str(directory)],
                                text=True, capture_output=True)
        self.assertEqual(sealed.returncode, 0, sealed.stderr)
        self.assertEqual(sorted(p.name for p in directory.iterdir()),
                         sorted(self.names + ["BUILD-MANIFEST.json"]))

    def test_rc_unknown_extra_file_still_blocks_sealing(self):
        self.env.update(IS_RC="true", VERSION=RC, GITHUB_SHA=SOURCE)
        (self.remote / "BUILD-MANIFEST.json").unlink()
        shutil.copytree(self.remote, self.work / "artifacts")
        (self.work / "artifacts/unexpected.txt").write_text("must not be published")
        self.assertNotEqual(self.run_block().returncode, 0)
        self.assertFalse((self.work / "artifacts/BUILD-MANIFEST.json").exists())

    def test_release_notes_disclose_exclusive_default_configuration(self):
        text = WORKFLOW.read_text()
        self.assertIn("该 ZIP 包含独占方案列表", text)
        self.assertIn("安装会替换原文件中的方案列表", text)
        self.assertNotIn("该 ZIP **刻意不含**", text)
        self.assertNotIn("追加 \\`xhup_flow\\`", text)

    def test_rehearsal_does_not_create_promotable_manifest(self):
        self.env.update(IS_RC="true", VERSION=RC, PUBLISH="false")
        (self.remote / "BUILD-MANIFEST.json").unlink()
        shutil.copytree(self.remote, self.work / "artifacts")
        self.assertEqual(self.run_block().returncode, 0)
        self.assertFalse((self.work / "artifacts/BUILD-MANIFEST.json").exists())

    def test_publish_graph_requires_verified_payload(self):
        text = WORKFLOW.read_text()
        self.assertIn("if: needs.validate.outputs.is_rc == 'true'\n    uses:", text)
        self.assertIn("needs.prepare.result == 'success'", text)
        self.assertIn("needs: [validate, prepare]", text)
        self.assertIn("name: release-verified-payload", text)
        self.assertIn('--target "$SOURCE_COMMIT"', text)
        publish = text.split("\n  publish:", 1)[1]
        self.assertNotIn('xhup-flow-trainer-v$VERSION-', publish)
        self.assertNotIn('--target "$GITHUB_SHA"', publish)


if __name__ == "__main__":
    unittest.main()
