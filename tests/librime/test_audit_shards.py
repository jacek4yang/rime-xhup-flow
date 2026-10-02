"""Synthetic receipt/partition regression tests; not librime qualification."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import audit_shards as audit

REV = "a" * 40
RUN = "123.1"


class Shards(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "input"
        self.root.mkdir()
        (self.root / "package").mkdir()
        (self.root / "package/xhup_flow.schema.yaml").write_text("schema")
        (self.root / "xhup-cli").write_text("synthetic executable identity")
        (self.root / "librime-lua.so").write_text("synthetic plugin identity")
        for kind in audit.KINDS:
            (self.root / f"{kind}.manifest").write_text(
                "# comment\na\t甲\nb\t乙\nc\t丙\na\t甲\ne\t戊\n")
        audit.partitions(self.root, 2, write=True)
        self.plan = audit.expected_plan(self.root, 2, REV, RUN)
        audit.write_json(self.root / "plan.json", self.plan)
        self.out = Path(self.temp.name) / "results"
        self.out.mkdir()
        for index in range(2):
            log = self.out / f"audit-{index}.log"
            log.write_text("\n".join(audit.BANNERS) + "\n")
            audit.write_json(self.out / f"receipt-{index}.json",
                             audit.expected_receipt(self.root, self.plan, index, log))

    def test_all_rows_including_duplicates_and_remainder(self):
        result = audit.collect(self.root, self.out, REV, RUN)
        self.assertEqual(result["rows"], dict.fromkeys(audit.KINDS, 5))
        for kind in audit.KINDS:
            self.assertEqual([p["rows"] for p in self.plan["inputs"][kind]["parts"]],
                             [3, 2])
            self.assertEqual((self.root / f"parts/0/{kind}.manifest").read_text(),
                             "a\t甲\nc\t丙\ne\t戊\n")

    def test_missing_receipt(self):
        (self.out / "receipt-1.json").unlink()
        with self.assertRaises(ValueError):
            audit.collect(self.root, self.out, REV, RUN)

    def test_mutated_receipt_fields(self):
        path = self.out / "receipt-1.json"
        original = json.loads(path.read_text())
        for key, value in (("index", 0), ("exit_code", 1), ("run", "old"),
                           ("revision", "b" * 40), ("coverage", {}),
                           ("plan_sha256", "bad"), ("count", 1)):
            with self.subTest(key=key):
                altered = copy.deepcopy(original)
                altered[key] = value
                path.write_text(json.dumps(altered))
                with self.assertRaises(ValueError):
                    audit.collect(self.root, self.out, REV, RUN)

    def test_changed_package_cli_manifest_partition(self):
        for name in ("package/xhup_flow.schema.yaml", "xhup-cli",
                     "librime-lua.so", "static.manifest", "parts/1/extended.manifest"):
            path = self.root / name
            original = path.read_bytes()
            with self.subTest(name=name):
                path.write_bytes(original + b"\n")
                with self.assertRaises(ValueError):
                    audit.collect(self.root, self.out, REV, RUN)
                path.write_bytes(original)

    def test_old_source_or_run(self):
        for revision, run in (("b" * 40, RUN), (REV, "123.2")):
            with self.assertRaises(ValueError):
                audit.collect(self.root, self.out, revision, run)

    def test_incomplete_or_failed_log(self):
        path = self.out / "audit-0.log"
        original = path.read_text()
        for value in ("", original + "FAIL negative\n",
                      original + "CLI NOT RUN\n",
                      original + audit.BANNERS[0] + "\n"):
            path.write_text(value)
            with self.assertRaises(ValueError):
                audit.expected_receipt(self.root, self.plan, 0, path)

    def test_log_tampering_extra_receipt_and_stale_write(self):
        path = self.out / "audit-0.log"
        path.write_text(path.read_text() + "changed\n")
        with self.assertRaises(ValueError):
            audit.collect(self.root, self.out, REV, RUN)
        with self.assertRaises(FileExistsError):
            audit.write_json(self.out / "receipt-0.json", {})
        (self.out / "receipt-2.json").write_text("{}")
        with self.assertRaises(ValueError):
            audit.collect(self.root, self.out, REV, RUN)

    def test_invalid_or_empty_partition(self):
        with self.assertRaises(ValueError):
            audit.partitions(self.root, 6)
        for row in (b"a\ttext", b"\n", b"A\ttext\n", b"a\t\n",
                    b"a\tbad\0\n", b"a\tbad\r\n"):
            (self.root / "static.manifest").write_bytes(row)
            with self.assertRaises(ValueError):
                audit.partitions(self.root, 1)

    def test_focused_environment_is_removed(self):
        out = self.out / "executed"

        def success(command, **kwargs):
            self.assertFalse(any(key.startswith(("XHUP_REPLAY_",))
                                 for key in kwargs["env"]))
            self.assertNotIn("XHUP_AUDIT_ONLY_LEARNING", kwargs["env"])
            self.assertEqual(kwargs["env"]["XHUP_AUDIT_SHARDS"], "2")
            self.assertEqual(len(command), 7)
            kwargs["stdout"].write("\n".join(audit.BANNERS) + "\n")

        with patch.dict(audit.os.environ, {"XHUP_AUDIT_ONLY_LEARNING": "1",
                                          "XHUP_REPLAY_CORPUS": "bad"}), \
             patch.object(audit.subprocess, "check_output", return_value=REV), \
             patch.object(audit.subprocess, "run", side_effect=success):
            audit.execute(self.root, out, REV, RUN, 0)
        self.assertTrue((out / "receipt-0.json").exists())

    def test_failed_process_cannot_write_success_receipt(self):
        out = self.out / "failed"
        import subprocess
        with patch.object(audit.subprocess, "check_output", return_value=REV), \
             patch.object(audit.subprocess, "run",
                          side_effect=subprocess.CalledProcessError(1, "audit")):
            with self.assertRaises(subprocess.CalledProcessError):
                audit.execute(self.root, out, REV, RUN, 0)
        self.assertFalse((out / "receipt-0.json").exists())


if __name__ == "__main__":
    unittest.main()
