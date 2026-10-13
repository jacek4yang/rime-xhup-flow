#!/usr/bin/env python3
"""Version identity boundaries; isolated synthetic source trees only."""
import json
from pathlib import Path
import tempfile
import unittest

from check_product_version import PRODUCT_JSON, check


class ProductVersionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def source(self, version):
        (self.root / "Cargo.toml").write_text(
            f'[workspace.package]\nversion = "{version}"\n', encoding="utf-8")
        for name in PRODUCT_JSON:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps({"version": version}), encoding="utf-8")

    def test_rc_cannot_be_renamed_to_stable_or_another_rc(self):
        self.source("2.1.0-rc.1")
        for requested in ("2.1.0", "2.1.0-rc.2"):
            with self.subTest(requested=requested), self.assertRaises(ValueError):
                check(self.root, requested)
        self.assertEqual(check(self.root, "2.1.0-rc.1"), "2.1.0-rc.1")

    def test_stable_requires_every_product_to_be_bumped(self):
        for name in PRODUCT_JSON:
            self.source("2.1.0")
            (self.root / name).write_text(json.dumps({"version": "2.1.0-rc.1"}))
            with self.subTest(product=name), self.assertRaises(ValueError):
                check(self.root, "2.1.0")
        self.source("2.1.0")
        self.assertEqual(check(self.root, "2.1.0"), "2.1.0")

    def test_noncanonical_versions_are_not_publishable(self):
        for version in ("02.1.0", "2.1.0-rc.0", "2.1.0-rc.01", "2.1.0+renamed", "2.1.0-alpha.1"):
            self.source(version)
            with self.subTest(version=version), self.assertRaises(ValueError):
                check(self.root)

    def test_missing_product_version_does_not_fall_back(self):
        self.source("2.1.0")
        (self.root / PRODUCT_JSON[0]).write_text("{}")
        with self.assertRaises(KeyError):
            check(self.root)


if __name__ == "__main__":
    unittest.main()
