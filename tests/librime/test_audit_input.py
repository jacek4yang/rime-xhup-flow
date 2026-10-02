"""Negative framing/status tests: real C helper, no native acceptance claim."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent


class AuditInputTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temp.cleanup)
        root = Path(cls.temp.name)
        source = root / "driver.c"
        source.write_text("""#include "audit_input.h"
#include <stdlib.h>
int main(int argc,char **argv) {
  if(argc==3) return audit_inputs(argv[1],argv[2]) ? 0 : 2;
  if(argc==4) return audit_status(atoi(argv[2]),atoi(argv[3]));
  return 3;
}
""")
        cls.binary = root / "driver"
        subprocess.run([os.environ.get("CC", "cc"), "-std=c11", "-Wall", "-Wextra", "-Werror",
                        "-I", str(HERE), str(source), "-o", str(cls.binary)], check=True)

    def run_pair(self, manifest, capture):
        with tempfile.TemporaryDirectory() as directory:
            source, saved = Path(directory)/"manifest", Path(directory)/"capture"
            source.write_bytes(manifest)
            saved.write_bytes(capture)
            return subprocess.run([self.binary, source, saved], check=False).returncode

    def test_complete_pair(self):
        self.assertEqual(self.run_pair(b"# source\n\nni\tA|B\r\nhc\tC\n",
                                       b"ni\tA\x1fB\nhc\tC\n"), 0)

    def test_missing_extra_or_misaligned_capture_rejected(self):
        for capture in (b"", b"hc\tA\n", b"ni\tA\nhc\tB\n", b"# comment\nni\tA\n"):
            with self.subTest(capture=capture):
                self.assertNotEqual(self.run_pair(b"ni\tA\n", capture), 0)

    def test_malformed_or_incomplete_manifest_rejected(self):
        for manifest in (b"", b"# empty\n", b"ni A\n", b"ni\t\n", b"\tA\n",
                         b"ni\tA\tB\n", b"ni\tA", b"ni\tA\x00B\n", b"ni1\tA\n",
                         b"ni\t" + b"A"*65536 + b"\n"):
            with self.subTest(manifest=manifest[:25]):
                self.assertNotEqual(self.run_pair(manifest, b"ni\tA\n"), 0)

    def test_reported_sentinel_failures_cannot_exit_zero(self):
        for result, failures, expected in ((0,0,0),(0,1,1),(1,0,1),(2,0,2),(2,3,2)):
            outcome = subprocess.run([self.binary,"status",str(result),str(failures)], check=False)
            self.assertEqual(outcome.returncode, expected)


if __name__ == "__main__":
    unittest.main()
