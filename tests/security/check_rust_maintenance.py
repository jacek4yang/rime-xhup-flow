#!/usr/bin/env python3
"""Verify exact source-bound dependency migrations; never suppress an advisory."""
import hashlib
import json
from pathlib import Path
import tomllib
from rust_warning_patches import original_before_warning_fixes

ROOT = Path(__file__).resolve().parents[2]
PREFIX = "// Take a look at the license at the top of the repository in the LICENSE file.\n"
MACRO_PATCHES = {
    "Cargo.toml": [('[dependencies.proc-macro-error]\nversion = "1.0"',
                    '[dependencies.proc-macro-error]\nversion = "2.0.1"\npackage = "proc-macro-error2"')],
    "Cargo.toml.orig": [('proc-macro-error = "1.0"',
                         'proc-macro-error = { version = "2.0.1", package = "proc-macro-error2" }')],
    "src/lib.rs": [(PREFIX, PREFIX + "\n// Attribute expansions use the maintained crate's canonical name.\nextern crate proc_macro_error as proc_macro_error2;\n")],
}
PATCHES = {
    "glib-macros": MACRO_PATCHES,
    "gtk3-macros": MACRO_PATCHES,
    "urlpattern": {
        "Cargo.toml": [('[dependencies.unic-ucd-ident]\nversion = "0.9.0"\nfeatures = ["id"]',
                       '[dependencies.unicode-id-start]\nversion = "1.4.0"')],
        "Cargo.toml.orig": [('unic-ucd-ident = { version = "0.9.0", features = ["id"] }',
                            'unicode-id-start = "1.4.0"')],
        "src/tokenizer.rs": [
            ("unic_ucd_ident::is_id_start(code_point)", "unicode_id_start::is_id_start(code_point)"),
            ("unic_ucd_ident::is_id_continue(code_point)", "unicode_id_start::is_id_continue(code_point)"),
        ],
    },
    "proc-macro-error2": {"src/lib.rs": [("extern crate proc_macro;", "pub extern crate proc_macro;")]},
}
# Digests bind the source inventories and separately bundled upstream notices.
PINNED = json.loads((Path(__file__).with_name("rust-maintenance-pins.json")).read_text())

def digest(data):
    return hashlib.sha256(data).hexdigest()

def check(root=ROOT):
    cargo = tomllib.loads((root / "Cargo.toml").read_text())
    count = 0
    for name, changes in PATCHES.items():
        directory = root / "vendor" / name
        pins = PINNED[name]
        manifest = directory / "XHUP-UPSTREAM.json"
        assert digest(manifest.read_bytes()) == pins["inventory"], f"{name}: changed inventory"
        upstream = json.loads(manifest.read_text())
        assert upstream["name"] == name
        expected = set(upstream["files"]) | {"XHUP-UPSTREAM.json"} | set(pins["notices"])
        actual = {p.relative_to(directory).as_posix() for p in directory.rglob("*") if p.is_file()}
        assert actual == expected, f"{name}: unexpected/missing source files"
        assert not any(p.is_symlink() for p in directory.rglob("*")), f"{name}: symlink"
        for relative, original_hash in upstream["files"].items():
            data = original_before_warning_fixes(f"vendor/{name}/{relative}", (directory / relative).read_bytes())
            for old, new in reversed(changes.get(relative, [])):
                assert data.count(new.encode()) == 1, f"{name}/{relative}: missing exact patch"
                data = data.replace(new.encode(), old.encode(), 1)
            assert digest(data) == original_hash, f"{name}/{relative}: differs from upstream+patch"
            count += 1
        for relative, expected_hash in pins["notices"].items():
            assert digest((directory / relative).read_bytes()) == expected_hash, f"{name}: notice"
        assert cargo["patch"]["crates-io"][name]["path"] == f"vendor/{name}", f"{name}: Cargo binding"
    resources = json.loads((root / "trainer/src-tauri/tauri.conf.json").read_text())["bundle"]["resources"]
    notices = {
        "glib-macros": ("LICENSE", "COPYRIGHT"),
        "gtk3-macros": ("LICENSE", "COPYRIGHT"),
        "urlpattern": ("LICENSE",),
        "proc-macro-error2": ("LICENSE-MIT", "LICENSE-APACHE"),
    }
    destinations = []
    for name, files in notices.items():
        for filename in files:
            source = f"../../vendor/{name}/{filename}"
            destination = resources.get(source)
            assert destination and destination.startswith("licenses/"), f"{name}: missing bundled notice"
            assert ".." not in Path(destination).parts, f"{name}: unsafe notice destination"
            destinations.append(destination)
    for destination in destinations:
        assert list(resources.values()).count(destination) == 1, "colliding bundled notice destinations"
    lock = tomllib.loads((root / "Cargo.lock").read_text())
    for name in PATCHES:
        packages = [p for p in lock["package"] if p["name"] == name]
        assert len(packages) == 1 and "source" not in packages[0], f"{name}: lockfile not local"
    retired = {"proc-macro-error", "proc-macro-error-attr", "unic-char-property",
               "unic-char-range", "unic-common", "unic-ucd-ident", "unic-ucd-version"}
    assert not retired.intersection(p["name"] for p in lock["package"]), "retired chain reintroduced"
    return count

if __name__ == "__main__":
    print(f"PASS {check()} original source identities, exact maintenance patches, licenses and bindings")
