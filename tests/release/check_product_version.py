#!/usr/bin/env python3
"""Reject mixed or overridden product versions before any platform build."""
import argparse
import json
from pathlib import Path
import re
import tomllib

PRODUCT_JSON = (
    "trainer/src-tauri/tauri.conf.json",
    "trainer/package.json",
    "miniapp/package.json",
    "packages/trainer-core/package.json",
)
VERSION = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-rc\.[1-9][0-9]*)?")


def check(root: Path, expected: str | None = None) -> str:
    with (root / "Cargo.toml").open("rb") as source:
        version = tomllib.load(source)["workspace"]["package"]["version"]
    if not isinstance(version, str) or not VERSION.fullmatch(version):
        raise ValueError("workspace version must be canonical stable or RC")
    if expected is not None and version != expected:
        raise ValueError(f"requested {expected!r} differs from source version {version!r}")
    for name in PRODUCT_JSON:
        actual = json.loads((root / name).read_text(encoding="utf-8"))["version"]
        if actual != version:
            raise ValueError(f"{name}: {actual!r} differs from workspace {version!r}")
    return version


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--expect-version")
    args = parser.parse_args()
    try:
        print(check(args.root, args.expect_version))
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f"product version rejected: {error}\n")
