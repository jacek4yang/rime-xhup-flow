#!/usr/bin/env python3
import shutil
import tempfile
import unittest
from pathlib import Path
import tomllib

from check_generated_runtime_sources import MODULES, verify

ROOT = Path(__file__).resolve().parents[2]


class RuntimeSourceBinding(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.package = Path(self.temp.name)
        for module in MODULES:
            relative = Path("lua/xhup_flow") / (module + ".lua")
            (self.package / relative).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / "rime" / relative, self.package / relative)
        version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
        for name in ("xhup_flow", "xhup_flow_static"):
            template = (ROOT / "rime/templates" / (name + ".schema.yaml.in")).read_bytes()
            (self.package / (name + ".schema.yaml")).write_bytes(template.replace(b"{{VERSION}}", version.encode()))

    def test_exact_source(self):
        self.assertEqual(verify(self.package, ROOT), 10)

    def test_each_module_and_schema_mutation_rejected(self):
        for path in self.package.rglob("*"):
            if not path.is_file():
                continue
            original = path.read_bytes()
            with self.subTest(path=path.name):
                path.write_bytes(original + b"\n")
                with self.assertRaisesRegex(ValueError, "runtime source mismatch"):
                    verify(self.package, ROOT)
                path.write_bytes(original)

    def test_missing_module_rejected(self):
        (self.package / "lua/xhup_flow/native_tail.lua").unlink()
        with self.assertRaises(ValueError):
            verify(self.package, ROOT)


if __name__ == "__main__":
    unittest.main()
