#!/usr/bin/env python3
"""Real compiler outputs, stale-source rejection, and untrusted package mutations."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

from check_generated_runtime_sources import MANIFEST, _dependency_inputs, compile_reference, identity, verify

ROOT = Path(__file__).resolve().parents[2]


class DependencyPaths(unittest.TestCase):
    def test_make_paths_preserve_quotes_dollars_and_escaped_spaces(self):
        with tempfile.TemporaryDirectory(prefix="xhup compiler's ") as directory:
            root = Path(directory)
            names = ["word's.rs", 'word$"two.rs']
            paths = {root / name for name in names}
            for path in paths:
                path.touch()
            escaped = [str(path).replace("$", "$$").replace(" ", "\\ ") for path in sorted(paths)]
            depfile = root / "compiler.d"
            for rules in ("\n".join(name + ":" for name in escaped),
                          "compiler: " + " ".join(escaped)):
                with self.subTest(rules=rules):
                    depfile.write_text(rules + "\n", encoding="utf-8")
                    self.assertEqual(_dependency_inputs(depfile, root), paths)


class RuntimeSourceBinding(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.reference = cls.enterClassContext(compile_reference(ROOT))

    def setUp(self):
        self.work = Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.package = self.work / "package"
        shutil.copytree(self.reference.directory, self.package)

    def source_snapshot(self):
        source = self.work / "source"
        for name in self.reference.inputs:
            path = source / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, path)
        return source

    def test_new_source_file_invalidates_the_compiler_input_inventory(self):
        source = self.source_snapshot()
        self.reference.check_sources(source)
        (source / "crates/xhup-cli/src/new_module.rs").write_text("pub fn added() {}\n", encoding="utf-8")
        with self.assertRaises(ValueError):
            self.reference.check_sources(source)

    def test_independent_generation_is_byte_identical(self):
        with compile_reference(ROOT) as independent:
            self.assertEqual(self.reference.artifacts, independent.artifacts)

    def test_changed_artifacts_are_rejected_even_with_forged_manifest(self):
        path = self.package / "lua/xhup_flow/native_tail.lua"
        path.write_bytes(path.read_bytes() + b'\nerror("modified runtime")\n')
        manifest_path = self.package / MANIFEST
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        for item in manifest["artifacts"]:
            if item["path"] == path.relative_to(self.package).as_posix():
                item.update(identity(path))
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        with self.assertRaises(ValueError):
            verify(self.package, ROOT, self.reference)

    def test_old_verbatim_source_is_not_a_compiled_artifact(self):
        relative = Path("lua/xhup_flow/annotation.lua")
        shutil.copyfile(ROOT / "rime" / relative, self.package / relative)
        with self.assertRaises(ValueError):
            verify(self.package, ROOT, self.reference)

    def test_dictionary_schema_and_generated_data_mutations_are_rejected(self):
        for name in ("xhup_flow_chars.dict.yaml", "xhup_flow.schema.yaml",
                     "lua/xhup_flow/data/quick_hints.lua", "xhup_flow.sources.tsv"):
            path = self.package / name
            original = path.read_bytes()
            with self.subTest(path=name):
                path.write_bytes(original + b"\n")
                with self.assertRaises(ValueError):
                    verify(self.package, ROOT, self.reference)
            path.write_bytes(original)

    def test_missing_module_and_same_count_foreign_replacement_are_rejected(self):
        (self.package / "lua/xhup_flow/full_span.lua").unlink()
        with self.assertRaises(ValueError):
            verify(self.package, ROOT, self.reference)
        (self.package / "foreign.lua").write_text("return {}", encoding="utf-8")
        with self.assertRaises(ValueError):
            verify(self.package, ROOT, self.reference)

    def test_symlink_is_not_an_artifact_even_if_bytes_match(self):
        relative = Path("xhup_flow_static.schema.yaml")
        (self.package / relative).unlink()
        (self.package / relative).symlink_to(self.reference.directory / relative)
        with self.assertRaises(ValueError):
            verify(self.package, ROOT, self.reference)

    def test_every_lua_source_change_invalidates_cached_compiler_evidence(self):
        source = self.source_snapshot()
        for name in self.reference.inputs:
            if not name.endswith(".lua"):
                continue
            path = source / name
            original = path.read_bytes()
            with self.subTest(source=name):
                path.write_bytes(original + b"\n-- changed source\n")
                with self.assertRaises(ValueError):
                    verify(self.package, source, self.reference)
            path.write_bytes(original)
        (source / "rime/lua/xhup_flow/unregistered.lua").write_text("return {}", encoding="utf-8")
        with self.assertRaises(ValueError):
            verify(self.package, source, self.reference)

    def test_registry_builder_rejects_unregistered_missing_and_colliding_modules(self):
        source = self.source_snapshot()
        output = self.work / "compiler-output"
        output.mkdir()
        environment = dict(os.environ, CARGO_MANIFEST_DIR=str(source / "crates/xhup-generator"),
                           OUT_DIR=str(output))

        def build_registry():
            return subprocess.run([str(self.reference.registry_builder)], cwd=source,
                                  env=environment, capture_output=True, text=True)

        valid = build_registry()
        self.assertEqual(valid.returncode, 0, valid.stderr)
        orphan = source / "rime/lua/xhup_flow/unregistered.lua"
        orphan.write_text("return {}", encoding="utf-8")
        self.assertNotEqual(build_registry().returncode, 0)
        orphan.unlink()

        missing = source / "rime/lua/xhup_flow/native_tail.lua"
        original = missing.read_bytes()
        missing.unlink()
        self.assertNotEqual(build_registry().returncode, 0)
        missing.write_bytes(original)

        registry_path = source / "rime/lua/modules.json"
        registry = json.loads(registry_path.read_text(encoding="utf-8"))
        registry["production"].append("xhup_flow.lua")
        (source / "rime/lua/xhup_flow.lua").write_text("return {}", encoding="utf-8")
        registry_path.write_text(json.dumps(registry), encoding="utf-8")
        self.assertNotEqual(build_registry().returncode, 0)

    def test_notice_builder_rejects_empty_license(self):
        source = self.source_snapshot()
        (source / "LICENSE").write_bytes(b"")
        output = self.work / "compiler-output"
        output.mkdir()
        result = subprocess.run(
            [str(self.reference.registry_builder)], cwd=source,
            env=dict(os.environ, CARGO_MANIFEST_DIR=str(source / "crates/xhup-generator"),
                     OUT_DIR=str(output)), capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
