"""Actual distribution ordering and DEB payload preservation, not GUI acceptance."""
import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from linux_package_versions import native_versions, normalize_deb, payload_identity


@unittest.skipUnless(sys.platform == "linux", "Linux distribution tooling")
class LinuxPackageVersions(unittest.TestCase):
    def test_native_upgrade_order_from_legacy_rc_through_final(self):
        versions = ["2.1.0-rc.1", "2.1.0-rc.2", "2.1.0-rc.10", "2.1.0", "2.1.1-rc.1"]
        native = [native_versions(value) for value in versions]
        subprocess.run(["dpkg", "--compare-versions", "2.0.0-rc.3", "lt", native[0]["deb"]], check=True)
        for before, after in zip(native, native[1:]):
            with self.subTest(before=before, after=after):
                subprocess.run(["dpkg", "--compare-versions", before["deb"], "lt", after["deb"]], check=True)
                expression = ('%{lua:local c=rpm.vercmp("' + before["rpm_version"] + '","' + after["rpm_version"]
                    + '"); if c==0 then c=rpm.vercmp("' + before["rpm_release"] + '","' + after["rpm_release"]
                    + '") end; print(c)}')
                comparison = subprocess.check_output(["rpm", "--eval", expression], text=True)
                self.assertLess(int(comparison), 0)

    def make_deb(self, root):
        package = root / "source"
        (package / "DEBIAN").mkdir(parents=True)
        (package / "DEBIAN/control").write_text(
            "Package: xhup-version-fixture\nVersion: 2.1.0-rc.1\nArchitecture: all\n"
            "Maintainer: Fixture <fixture@example.invalid>\nDescription: synthetic packaging fixture\n")
        data = package / "usr/share/xhup-version-fixture"
        data.mkdir(parents=True)
        (data / "payload").write_bytes(b"synthetic data\x00\xff\n")
        (data / "payload").chmod(0o755)
        (data / "alias").symlink_to("payload")
        source = root / "source.deb"
        subprocess.run(["dpkg-deb", "--build", "--root-owner-group", str(package), str(source)],
                       check=True, stdout=subprocess.DEVNULL)
        return source

    def test_normalization_preserves_payload_and_original_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = self.make_deb(root)
            original = hashlib.sha256(source.read_bytes()).hexdigest()
            normalized = root / "normalized.deb"
            normalize_deb(source, normalized, "2.1.0-rc.1")
            self.assertEqual(payload_identity(source), payload_identity(normalized))
            self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), original)
            installed_version = subprocess.check_output(
                ["dpkg-deb", "--field", str(normalized), "Version"], text=True).strip()
            subprocess.run(["dpkg", "--compare-versions", installed_version, "lt", "2.1.0"], check=True)

    def test_wrong_source_version_cannot_produce_an_artifact(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = self.make_deb(root)
            output = root / "wrong.deb"
            with self.assertRaises(ValueError):
                normalize_deb(source, output, "2.1.0-rc.2")
            self.assertFalse(output.exists())

    def test_existing_output_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = self.make_deb(root)
            output = root / "existing.deb"
            output.write_bytes(b"immutable earlier artifact")
            with self.assertRaises(FileExistsError):
                normalize_deb(source, output, "2.1.0-rc.1")
            self.assertEqual(output.read_bytes(), b"immutable earlier artifact")


if __name__ == "__main__":
    unittest.main()
