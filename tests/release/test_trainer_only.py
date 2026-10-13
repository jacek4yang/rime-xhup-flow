"""Installation instructions must stop before installing unverified downloads."""
from pathlib import Path
import re
import subprocess
import tempfile
import hashlib
import shutil
import unittest

ROOT = Path(__file__).resolve().parents[2]


class TrainerOnlyTests(unittest.TestCase):
    def test_download_failure_cannot_validate_a_stale_file(self):
        guide = (ROOT / "docs/install-guide.zh-CN.md").read_text()
        function = re.search(r"download_xhup\(\) \(.*?\n\)", guide, re.S).group()
        for shell in ("bash", "dash"):
            if not shutil.which(shell):
                continue
            with self.subTest(shell=shell), tempfile.TemporaryDirectory() as directory:
                path = Path(directory)
                (path / "old.apk").write_bytes(b"stale but hash-valid")
                digest = hashlib.sha256(b"stale but hash-valid").hexdigest()
                (path / "SHA256SUMS.txt").write_text(f"{digest}  old.apk\n")
                script = ("BASE=https://example.invalid\ncurl() { return 22; }\n" + function
                          + "\ndownload_xhup old.apk && touch installation-attempted\n")
                result = subprocess.run([shell, "-c", script], cwd=path,
                                        capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("SHA-256 OK", result.stdout)
                self.assertFalse((path / "installation-attempted").exists())

    def test_linux_install_commands_stop_on_hash_failure(self):
        guide = (ROOT / "docs/install-guide.zh-CN.md").read_text()
        blocks = [block for block in re.findall(r"```sh\n(.*?)```", guide, re.S)
                  if "sudo apt install" in block or "sudo dnf install" in block]
        self.assertEqual(len(blocks), 2)
        for block in blocks:
            with self.subTest(block=block), tempfile.TemporaryDirectory() as directory:
                marker = Path(directory) / "installation-attempted"
                prefix = f"download_xhup() {{ return 1; }}\nsudo() {{ touch '{marker}'; }}\n"
                result = subprocess.run(["bash", "-c", prefix + block], capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(marker.exists(), "failed checksum must prevent installation")

if __name__ == "__main__":
    unittest.main()
