#!/usr/bin/env python3
"""Reject stale/cross-worktree runtime code before claiming native qualification."""
import argparse
from pathlib import Path
import tomllib

MODULES = (
    "annotation", "context_ranker", "full_span", "init", "joint_decoder",
    "native_tail", "quick_hint", "user_memory",
)


def verify(package, root):
    version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    expected = {}
    for name in MODULES:
        relative = Path("lua/xhup_flow") / (name + ".lua")
        expected[relative] = (root / "rime" / relative).read_bytes()
    for name in ("xhup_flow", "xhup_flow_static"):
        relative = Path(name + ".schema.yaml")
        template = (root / "rime/templates" / (str(relative) + ".in")).read_bytes()
        expected[relative] = template.replace(b"{{VERSION}}", version.encode())
    for relative, source in expected.items():
        actual = package / relative
        if not actual.is_file() or actual.read_bytes() != source:
            raise ValueError(f"runtime source mismatch: {actual}; rebuild in an isolated Cargo target directory")
    return len(expected)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    args = parser.parse_args()
    count = verify(args.package, Path(__file__).resolve().parents[2])
    print(f"PASS generated runtime source binding: {count} exact module/schema identities")
