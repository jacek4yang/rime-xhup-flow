"""Archive regressions against actual compiler output and canonical notices."""
import hashlib
import json
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
import zipfile
import rime_archive as archive
from check_generated_runtime_sources import compile_reference

ROOT = Path(__file__).resolve().parents[2]


class RimeArchive(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.reference = cls.enterClassContext(compile_reference(ROOT))

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name)
        self.package = self.work / "package"
        shutil.copytree(self.reference.directory, self.package)
        self.paths = archive.payloads(self.package, ROOT, self.reference)
        self.zip = self.work / "package.zip"
        archive.create(self.zip, self.paths)

    def test_pre_archive_cli_uses_exact_inventory(self):
        command = [sys.executable, str(ROOT / "tests/release/rime_archive.py"), "verify", str(self.package)]
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        (self.package / "default.custom.yaml").unlink()
        # Same count, wrong file: a numerical gate alone must not pass.
        (self.package / "foreign.yaml").write_text("foreign")
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)

    def test_all_generated_files_not_just_yaml_lua_and_determinism(self):
        other = self.work / "second.zip"
        archive.create(other, self.paths)
        self.assertEqual(self.zip.read_bytes(), other.read_bytes())
        with self.assertRaises(FileExistsError):
            archive.create(self.zip, self.paths)

    def test_missing_policy_or_notice_and_modified_member(self):
        for missing in ("default.custom.yaml", "xhup_flow.sources.tsv", "NOTICE.md",
                        "licenses/pinyin-data-MIT.txt"):
            out = self.work / "missing.zip"
            with zipfile.ZipFile(out, "w") as zipped:
                for name, source in self.paths.items():
                    if name != missing:
                        zipped.writestr(name, source.read_bytes())
            with self.assertRaises(ValueError):
                archive.check(out, self.paths)
        out = self.work / "changed.zip"
        with zipfile.ZipFile(out, "w") as zipped:
            for name, source in self.paths.items():
                zipped.writestr(name, source.read_bytes() + b"changed")
        with self.assertRaises(ValueError):
            archive.check(out, self.paths)

    def test_private_or_unknown_generated_file_fails(self):
        for name in ("user.yaml", "installation.yaml", "secrets.txt",
                     "xhup_flow_user.userdb/CURRENT"):
            path = self.package / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("private")
            with self.assertRaises(ValueError):
                archive.payloads(self.package, ROOT, self.reference)
            path.unlink()

    def test_missing_and_replaced_policy(self):
        policy = self.package / "xhup_flow.sources.tsv"
        policy.unlink()
        with self.assertRaises(ValueError):
            archive.payloads(self.package, ROOT, self.reference)
        policy.write_text("false-policy")
        with self.assertRaises(ValueError):
            archive.payloads(self.package, ROOT, self.reference)

    def test_unsafe_symlink_and_unexpected_zip_member(self):
        path = self.package / "xhup_flow.dict.yaml"
        path.unlink()
        path.symlink_to(ROOT / "LICENSE")
        with self.assertRaises(ValueError):
            archive.payloads(self.package, ROOT, self.reference)
        with zipfile.ZipFile(self.zip, "a") as zipped:
            zipped.writestr("../user.yaml", "private")
        with self.assertRaises(ValueError):
            archive.check(self.zip, self.paths)

    def test_matching_bytes_cannot_hide_symlink_metadata(self):
        out = self.work / "symlink.zip"
        with zipfile.ZipFile(self.zip) as original, zipfile.ZipFile(out, "w") as changed:
            for member in original.infolist():
                contents = original.read(member.filename)
                if member.filename == "default.custom.yaml":
                    member.external_attr = (stat.S_IFLNK | 0o777) << 16
                changed.writestr(member, contents)
        with self.assertRaises(ValueError):
            archive.check(out, self.paths)

    def test_duplicate_member_is_rejected_even_when_bytes_match(self):
        name = "xhup_flow.schema.yaml"
        with zipfile.ZipFile(self.zip, "a") as zipped, self.assertWarns(UserWarning):
            zipped.writestr(name, self.paths[name].read_bytes())
        with self.assertRaises(ValueError):
            archive.check(self.zip, self.paths)

    def test_notices_bound_to_binary_resources_and_pinned_upstream(self):
        config = ROOT / "trainer/src-tauri/tauri.conf.json"
        resources = json.loads(config.read_text())["bundle"]["resources"]
        for name, source in archive.NOTICES.items():
            if name in ("INSTALL.md", "NOTICE.md"):
                continue
            self.assertTrue(resources["../../" + source].startswith("licenses/"))
            self.assertEqual(Path(resources["../../" + source]).name, Path(source).name)
        self.assertEqual(resources["../../rime/package/NOTICE.md"],
                         "licenses/rime-data/NOTICE.md")
        notice = ROOT / "data/hanzi/LICENSE.pinyin-data"
        self.assertEqual(hashlib.sha256(notice.read_bytes()).hexdigest(),
                         "9c048697be2502a16e8bcb282d5d465a07295b2def0ffb05a269c5d39dbe1586")

    def test_extracted_installer_missing_or_changed_notice_fails(self):
        import check_bundle_notices
        config = ROOT / "trainer/src-tauri/tauri.conf.json"
        mapping = json.loads(config.read_text())["bundle"]["resources"]
        extracted = self.work / "extracted"
        for source, target in mapping.items():
            path = extracted / target
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile((config.parent / source).resolve(), path)
        self.assertEqual(check_bundle_notices.verify(extracted, ROOT), len(mapping))
        path = extracted / "licenses/rime-data/NOTICE.md"
        path.write_text("changed")
        with self.assertRaises(ValueError):
            check_bundle_notices.verify(extracted, ROOT)
        path.unlink()
        with self.assertRaises(ValueError):
            check_bundle_notices.verify(extracted, ROOT)

    def test_installer_config_utf8_is_independent_of_windows_locale(self):
        from unittest.mock import patch
        import check_bundle_notices
        root = self.work / "unicode-config"
        config = root / "trainer/src-tauri/tauri.conf.json"
        config.parent.mkdir(parents=True)
        (root / "LICENSE").write_bytes(b"original license\n")
        config.write_text(json.dumps({
            "productName": "小鹤输入法",
            "bundle": {"resources": {"../../LICENSE": "licenses/project/LICENSE"}},
        }, ensure_ascii=False), encoding="utf-8")
        extracted = self.work / "unicode-extracted"
        target = extracted / "licenses/project/LICENSE"
        target.parent.mkdir(parents=True)
        target.write_bytes((root / "LICENSE").read_bytes())
        original_read = Path.read_text

        def windows_read(path, encoding=None, errors=None):
            return original_read(path, encoding=encoding or "cp1252", errors=errors)

        # Reproduce the Windows cp1252 failure without changing the host locale.
        with self.assertRaises(UnicodeDecodeError):
            windows_read(config)
        with patch.object(Path, "read_text", windows_read):
            self.assertEqual(check_bundle_notices.verify(extracted, root), 1)

    def test_wix_basename_collision_is_rejected_without_ice_suppression(self):
        from check_bundle_notices import validate_mapping
        validate_mapping({"../../a/LICENSE": "licenses/a/LICENSE",
                          "../../b/LICENSE": "licenses/b/LICENSE"})
        with self.assertRaises(ValueError):
            validate_mapping({"../../a/LICENSE": "licenses/a-MIT.txt",
                              "../../b/LICENSE": "licenses/b-MIT.txt"})
        with self.assertRaises(ValueError):
            validate_mapping({"../../a/LICENSE": "licenses/a/LICENSE",
                              "../../b/license": "licenses/A/license"})



if __name__ == "__main__":
    unittest.main()
