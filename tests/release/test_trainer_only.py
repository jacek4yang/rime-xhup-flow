"""The shipped training app must have no Rime installer or management IPC."""
from pathlib import Path
import tomllib
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class TrainerOnlyTests(unittest.TestCase):
    def test_native_container_has_no_application_commands(self):
        source = (ROOT / "trainer/src-tauri/src/lib.rs").read_text()
        self.assertNotIn("invoke_handler", source)
        self.assertNotIn("generate_handler", source)
        for name in ("commands", "manager", "exclusive", "package_export"):
            self.assertFalse((ROOT / f"trainer/src-tauri/src/{name}.rs").exists())
        self.assertEqual((ROOT / "trainer/src-tauri/build.rs").read_text().strip(),
                         "fn main() {\n    tauri_build::build();\n}")

    def test_native_dependency_boundary_excludes_installer(self):
        manifest = tomllib.loads((ROOT / "trainer/src-tauri/Cargo.toml").read_text())
        self.assertEqual(set(manifest["dependencies"]), {"tauri"})
        self.assertEqual(set(manifest["build-dependencies"]), {"tauri-build"})
        self.assertFalse((ROOT / "trainer/src-tauri/examples/product_cli.rs").exists())
        # Canonical training data remains generated at build time, not forked.
        self.assertIn("generate trainer", (ROOT / "trainer/package.json").read_text())

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

    def test_retained_lessons_do_not_promise_deleted_management(self):
        lessons = (ROOT / "packages/trainer-core/src/lessons/content.ts").read_text()
        self.assertNotIn("控制中心", lessons)
        self.assertIn("Trainer 的进度备份只含练习记录", lessons)
        self.assertIn("手动教程", lessons)

    def test_release_copy_has_no_retired_android_training_exemption(self):
        release = (ROOT / ".github/workflows/xhup-flow-rc-release.yml").read_text()
        self.assertNotIn("仅 Android 桌面生命周期", release)
        self.assertNotIn("输入法控制中心", release)
        self.assertIn("训练器生命周期覆盖所有平台", release)

    def test_no_product_route_or_installer_onboarding(self):
        shell = (ROOT / "trainer/src/components/AppShell.tsx").read_text()
        for removed in ("ControlCenterView", "FirstRunWizard", '"product"', "onboarding"):
            self.assertNotIn(removed, shell)
        for name in ("product.ts", "native.ts"):
            self.assertFalse((ROOT / f"trainer/src/lib/{name}").exists())
        store = (ROOT / "trainer/src/stores/trainer-store.ts").read_text()
        self.assertIn("xhup-flow.trainer.v2", store)


if __name__ == "__main__":
    unittest.main()
