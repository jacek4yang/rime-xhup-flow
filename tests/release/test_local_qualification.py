"""Synthetic receipts only; these tests are not product runtime acceptance."""
import importlib.util
import json
from pathlib import Path
import tempfile
import subprocess
import unittest
import local_qualification as local

spec = importlib.util.spec_from_file_location("local_audit_contract", local.ROOT / "tests/librime/audit_shards.py")
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)
SOURCE = "a" * 40


class LocalQualification(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        inputs = self.root / "native-input"
        (inputs / "package").mkdir(parents=True)
        (inputs / "package/xhup_flow.schema.yaml").write_text("synthetic test only")
        (inputs / "xhup-cli").write_bytes(b"synthetic-not-executable")
        (inputs / "librime-lua.so").write_bytes(b"synthetic-not-loadable")
        for kind in audit.KINDS:
            (inputs / f"{kind}.manifest").write_text("aa\t测试\n" * 1008)
        audit.partitions(inputs, 16, write=True)
        plan = audit.expected_plan(inputs, 16, SOURCE, "synthetic-test")
        audit.write_json(inputs / "plan.json", plan)
        results = self.root / "native-results"
        results.mkdir()
        for i in range(16):
            log = results / f"audit-{i}.log"
            log.write_text("\n".join(audit.BANNERS) + "\n")
            audit.write_json(results / f"receipt-{i}.json", audit.expected_receipt(inputs, plan, i, log))
        audit.write_json(results / "coverage.json", audit.collect(inputs, results, SOURCE, "synthetic-test"))
        checks = []
        for name in sorted(local.CHECKS):
            path = self.root / f"{name}.log"
            path.write_text("synthetic log only")
            checks.append(dict(name=name, exit_code=0, log_sha256=local.sha(path)))
        self.report = dict(schema_version=1, policy=local.POLICY, source_commit=SOURCE, result="PASS",
                           checks=checks, run="synthetic-test", files=local.inventory(self.root),
                           user_platform_testing=["windows", "linux", "macos", "android"])
        self.save()

    def save(self):
        (self.root / "LOCAL-VALIDATION.json").write_text(json.dumps(self.report))

    def test_cloud_payload_bound_to_actual_source_and_bytes(self):
        version = "2.0.0-rc.3"
        with tempfile.TemporaryDirectory() as temp:
            artifacts = Path(temp)
            names = [f"xhup-flow-rime-v{version}.zip", "SHA256SUMS.txt", "CANONICAL-SHA256SUMS.txt", "BUILD-INFO.txt"]
            names += [f"xhup-flow-trainer-v{version}-{suffix}" for suffix in ("windows-x64-setup.exe", "windows-x64.msi", "macos-universal.dmg", "linux-amd64.deb", "linux-x86_64.rpm", "android-arm64.apk", "android-universal.apk")]
            records = []
            for name in names:
                path = artifacts / name
                path.write_text("SYNTHETIC NON-INSTALLABLE FIXTURE " + name)
                records.append(dict(name=name, sha256=local.sha(path)))
            build = dict(schema_version=1, source_commit=SOURCE, version=version, artifacts=records)
            manifest = artifacts / "BUILD-MANIFEST.json"
            manifest.write_text(json.dumps(build))
            result = local.verify_build(self.root, SOURCE, artifacts, version)
            self.assertEqual(result["build_manifest_sha256"], local.sha(manifest))
            build["source_commit"] = "b" * 40
            manifest.write_text(json.dumps(build))
            with self.assertRaises(ValueError): local.verify_build(self.root, SOURCE, artifacts, version)
            build["source_commit"] = SOURCE
            manifest.write_text(json.dumps(build))
            (artifacts / names[0]).write_text("changed")
            with self.assertRaises(ValueError): local.verify_build(self.root, SOURCE, artifacts, version)

    def test_cloud_full_tests_are_manual_only(self):
        for name in ("ci.yml", "full-regression.yml"):
            workflow = (local.ROOT / ".github/workflows" / name).read_text()
            self.assertIn("  workflow_dispatch:", workflow)
            for trigger in ("  push:", "  pull_request:", "  schedule:"):
                self.assertNotIn(trigger, workflow)

    def test_recollects_all_receipts_without_reusing_coverage(self):
        local.verify(self.root, SOURCE)
        local.verify(self.root, SOURCE)
        self.assertEqual(local.inventory(self.root), self.report["files"])

    def test_wrong_source_and_policy_rejected(self):
        with self.assertRaises(ValueError): local.verify(self.root, "b" * 40)
        self.report["policy"] = "skip-everything"
        self.save()
        with self.assertRaises(ValueError): local.verify(self.root, SOURCE)

    def test_skipped_failed_duplicate_checks_rejected(self):
        for mutate in [lambda r: r["checks"].pop(), lambda r: r["checks"].append(r["checks"][0]),
                       lambda r: r["checks"][0].update(exit_code=1), lambda r: r.update(result="SKIP")]:
            old = json.loads(json.dumps(self.report))
            mutate(self.report)
            self.save()
            with self.assertRaises(ValueError): local.verify(self.root, SOURCE)
            self.report = old

    def test_changed_log_rejected(self):
        (self.root / "rust.log").write_text("modified")
        with self.assertRaises(ValueError): local.verify(self.root, SOURCE)

    def test_missing_shard_rejected_even_if_outer_inventory_updated(self):
        (self.root / "native-results/receipt-15.json").unlink()
        self.report["files"] = local.inventory(self.root)
        self.save()
        with self.assertRaises(subprocess.CalledProcessError): local.verify(self.root, SOURCE)

    def test_false_coverage_rejected_even_if_outer_inventory_updated(self):
        (self.root / "native-results/coverage.json").write_text("{}")
        self.report["files"] = local.inventory(self.root)
        self.save()
        with self.assertRaises(ValueError): local.verify(self.root, SOURCE)


if __name__ == "__main__":
    unittest.main()
