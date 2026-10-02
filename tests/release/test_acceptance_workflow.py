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
                        CORE="2.0.0", VERSION="2.0.0", IS_RC="false", PUBLISH="true")

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
