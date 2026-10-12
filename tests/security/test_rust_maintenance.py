import json
import shutil
import tempfile
import unittest
from pathlib import Path

from check_rust_maintenance import PATCHES, ROOT, check

class SourceIntegrityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in PATCHES:
            shutil.copytree(ROOT / "vendor" / name, self.root / "vendor" / name)
        for name in ("Cargo.toml", "Cargo.lock", "trainer/src-tauri/tauri.conf.json"):
            (self.root / name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, self.root / name)

    def test_unreviewed_source_is_rejected(self):
        path = self.root / "vendor/urlpattern/src/lib.rs"
        path.write_bytes(path.read_bytes() + b"\n// extra source\n")
        with self.assertRaisesRegex(AssertionError, "differs from upstream"):
            check(self.root)

    def test_unlisted_source_is_rejected(self):
        (self.root / "vendor/gtk3-macros/src/unreviewed.rs").write_text("pub fn extra() {}")
        with self.assertRaisesRegex(AssertionError, "unexpected/missing"):
            check(self.root)

    def test_removed_cargo_binding_is_rejected(self):
        cargo = self.root / "Cargo.toml"
        cargo.write_text(cargo.read_text().replace('urlpattern = { path = "vendor/urlpattern" }', ''))
        with self.assertRaises((AssertionError, KeyError)):
            check(self.root)

    def test_retired_chain_cannot_return(self):
        lock = self.root / "Cargo.lock"
        original = lock.read_text()
        for name in ("unic-common", "paste"):
            with self.subTest(name=name):
                lock.write_text(original + f'\n[[package]]\nname = "{name}"\nversion = "1.0.15"\n')
                with self.assertRaisesRegex(AssertionError, "retired chain"):
                    check(self.root)

    def test_binary_notice_removal_is_rejected(self):
        path = self.root / "trainer/src-tauri/tauri.conf.json"
        config = json.loads(path.read_text())
        del config["bundle"]["resources"]["../../vendor/urlpattern/LICENSE"]
        path.write_text(json.dumps(config))
        with self.assertRaisesRegex(AssertionError, "missing bundled notice"):
            check(self.root)

    def test_binary_notice_collision_is_rejected(self):
        path = self.root / "trainer/src-tauri/tauri.conf.json"
        config = json.loads(path.read_text())
        config["bundle"]["resources"]["../../vendor/urlpattern/LICENSE"] = config["bundle"]["resources"]["../../vendor/glib/LICENSE"]
        path.write_text(json.dumps(config))
        with self.assertRaisesRegex(AssertionError, "colliding bundled notice"):
            check(self.root)

    def test_notice_removal_is_rejected(self):
        (self.root / "vendor/urlpattern/LICENSE").unlink()
        with self.assertRaisesRegex(AssertionError, "unexpected/missing"):
            check(self.root)

if __name__ == "__main__":
    unittest.main()
