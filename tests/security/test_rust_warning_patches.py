"""Warning cleanup is reversible, never a new blanket source-hash exemption."""
import hashlib
import json
from pathlib import Path
import unittest
from rust_warning_patches import PATCHES, original_before_warning_fixes

ROOT = Path(__file__).resolve().parents[2]


class WarningPatches(unittest.TestCase):
    def test_exact_original_identities_and_security_backport_remain(self):
        glib = json.loads((ROOT / "vendor/glib/UPSTREAM-FILES.json").read_text())
        gtk = json.loads((ROOT / "vendor/gtk3-macros/XHUP-UPSTREAM.json").read_text())["files"]
        for path in PATCHES:
            data = (ROOT / path).read_bytes()
            restored = original_before_warning_fixes(path, data)
            relative = path.split("/", 2)[2]
            expected = (glib if path.startswith("vendor/glib/") else gtk)[relative]
            self.assertEqual(hashlib.sha256(restored).hexdigest(), expected, path)
            changed = data.replace(PATCHES[path][0][1].encode(), b"changed", 1)
            with self.assertRaises(AssertionError):
                original_before_warning_fixes(path, changed)
        self.assertNotIn("vendor/glib/src/variant_iter.rs", PATCHES)
        self.assertEqual(hashlib.sha256((ROOT / "vendor/glib/src/variant_iter.rs").read_bytes()).hexdigest(),
                         "a0f5ee8acb8faa089bcdfbc9a57372609fce7654026ccef7d9a224d05a654ccc")

    def test_unknown_source_is_not_whitelisted(self):
        self.assertEqual(original_before_warning_fixes("other.rs", b"unchanged"), b"unchanged")


if __name__ == "__main__":
    unittest.main()
