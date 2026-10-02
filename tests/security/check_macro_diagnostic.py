#!/usr/bin/env python3
"""Compile an invalid GLib derive: migration must preserve its actual diagnostic."""
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="xhup-macro-diagnostic-") as temporary:
    work = Path(temporary)
    paths = {name: str(ROOT / "vendor" / name).replace("\\", "/")
             for name in ("glib", "glib-macros", "proc-macro-error2")}
    manifest = '[package]\nname = "xhup-diagnostic-fixture"\nversion = "0.0.0"\nedition = "2021"\n[workspace]\n'
    manifest += '[dependencies]\nglib = { path = "' + paths["glib"] + '" }\n[patch.crates-io]\n'
    for name in ("glib-macros", "proc-macro-error2"):
        manifest += name + ' = { path = "' + paths[name] + '" }\n'
    (work / "Cargo.toml").write_text(manifest)
    (work / "src").mkdir()
    (work / "src/lib.rs").write_text('#[derive(glib::Enum)]\nstruct NotAnEnum;\n')
    env = os.environ.copy()
    env.setdefault("CARGO_TARGET_DIR", str(ROOT / "target"))
    result = subprocess.run(["cargo", "check", "--offline", "--manifest-path", str(work / "Cargo.toml")],
                            env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    assert result.returncode != 0, "invalid derive unexpectedly compiled"
    assert "src/lib.rs:1:10" in result.stdout, result.stdout
    assert "cannot find" not in result.stdout and "could not find" not in result.stdout, result.stdout
    assert "proc-macro derive panicked" not in result.stdout, result.stdout
    # Exact upstream diagnostic, not just any compilation error.
    assert "only supports enums" in result.stdout, result.stdout
print("PASS maintained macro backend rejects invalid input with upstream diagnostic")
