"""Packaging must run the same source-bound native contracts as normal CI."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


def verify(workflow):
    section = workflow.split("  rime-package:", 1)[1].split("      - name: 组装发布 zip", 1)[0]
    for gate in (
        "runs-on: ubuntu-24.04",
        "check_generated_runtime_sources.py build/rime-package",
        "rime_archive.py verify build/rime-package",
        "build-supported-lua.sh /tmp/xhup-package-lua",
        "run-deploy-audit.sh build/rime-package",
        "XHUP_AUDIT_ONLY_REPLAY=1 XHUP_REPLAY_VERIFY_LEARNING=1 tests/librime/run-flow-audit.sh build/rime-package",
    ):
        assert gate in section, f"missing pre-archive native gate: {gate}"


class PackagingRuntimeTests(unittest.TestCase):
    def setUp(self):
        self.workflow = (ROOT / ".github/workflows/product-packaging.yml").read_text()

    def test_all_gates_before_archive(self):
        verify(self.workflow)

    def test_matrix_uses_automatic_names_without_unexpanded_expression(self):
        workflow = (ROOT / ".github/workflows/full-regression.yml").read_text()
        job = workflow.split("  librime-full:", 1)[1].split("    steps:", 1)[0]
        self.assertNotIn("    name:", job)
        self.assertIn("shard: [" + ", ".join(map(str, range(16))) + "]", job)
        self.assertIn("needs: [prepare, librime-full]", workflow)

    def test_missing_gate_fails(self):
        for gate in ("check_generated_runtime_sources.py", "build-supported-lua.sh",
                     "run-deploy-audit.sh", "XHUP_REPLAY_VERIFY_LEARNING=1", "rime_archive.py verify"):
            with self.subTest(gate=gate), self.assertRaises(AssertionError):
                verify(self.workflow.replace(gate, "REMOVED"))

    def test_gate_after_archive_does_not_count(self):
        gate = "tests/librime/run-deploy-audit.sh build/rime-package"
        with self.assertRaises(AssertionError):
            verify(self.workflow.replace(gate, "true") + "\n" + gate)


if __name__ == "__main__":
    unittest.main()
