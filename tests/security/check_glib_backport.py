#!/usr/bin/env python3
"""A source-bound backport check, NOT a blanket cargo-audit ignore."""
import hashlib
import json
import tomllib
from pathlib import Path
from rust_warning_patches import original_before_warning_fixes

ROOT = Path(__file__).resolve().parents[2]
vendor = ROOT / "vendor/glib"
manifest = json.loads((vendor / "UPSTREAM-FILES.json").read_text())
allowed = set(manifest) | {"UPSTREAM-FILES.json", "XHUP-BACKPORT.md"}
actual = {str(p.relative_to(vendor)) for p in vendor.rglob("*") if p.is_file()}
assert actual == allowed, f"unexpected/missing vendored files: {actual ^ allowed}"
patched_file = "src/variant_iter.rs"
patched_hash = "a0f5ee8acb8faa089bcdfbc9a57372609fce7654026ccef7d9a224d05a654ccc"
for name, upstream_hash in manifest.items():
    expected = patched_hash if name == patched_file else upstream_hash
    data = original_before_warning_fixes(f"vendor/glib/{name}", (vendor / name).read_bytes())
    assert hashlib.sha256(data).hexdigest() == expected, name
lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
glib = [p for p in lock["package"] if p["name"] == "glib"]
assert len(glib) == 1 and glib[0]["version"] == "0.18.5" and "source" not in glib[0], glib
cargo = tomllib.loads((ROOT / "Cargo.toml").read_text())
assert cargo["patch"]["crates-io"]["glib"]["path"] == "vendor/glib"
bundle = json.loads((ROOT / "trainer/src-tauri/tauri.conf.json").read_text())["bundle"]
assert "../../vendor/glib/LICENSE" in bundle["resources"]
assert "../../vendor/glib/COPYRIGHT" in bundle["resources"]
print(f"PASS RUSTSEC-2024-0429 backport: {len(manifest)} upstream file identities, exact patched source, Cargo binding and notices")
