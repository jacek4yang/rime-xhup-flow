"""Source eligibility gate tests. Synthetic depfiles test validation, not a build."""
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class DistributionGate(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in ["tests/release/check_distribution_sources.py", "data/xhup/sources.tsv",
                     "data/words/sogou/MANIFEST.tsv"]:
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        self.dep = self.root / "xhup-cli.d"
        self.dep.write_text("xhup-cli: crates/xhup-generator/src/word_codes.rs\n")

    def run_gate(self):
        return subprocess.run([sys.executable, str(self.root / "tests/release/check_distribution_sources.py"),
                               "--depfile", str(self.dep)], capture_output=True).returncode

    def test_valid_evidence(self):
        self.assertEqual(self.run_gate(), 0)

    def test_restricted_dependency(self):
        self.dep.write_text("xhup-cli: word_codes.rs data/words/sogou/protect_list.tsv")
        self.assertNotEqual(self.run_gate(), 0)

    def test_wrong_manifest(self):
        (self.root / "data/words/sogou/MANIFEST.tsv").write_text("modified")
        self.assertNotEqual(self.run_gate(), 0)

    def test_not_a_cli_build_manifest(self):
        self.dep.write_text("unrelated: file.rs")
        self.assertNotEqual(self.run_gate(), 0)

    def test_registry_cannot_silently_omit_restriction(self):
        path = self.root / "data/xhup/sources.tsv"
        path.write_text("".join(line for line in path.read_text().splitlines(True)
                               if not line.startswith("sogou-cell-research")))
        self.assertNotEqual(self.run_gate(), 0)


if __name__ == "__main__":
    unittest.main()
