#!/usr/bin/env python3
"""Compare extracted installer notice bytes with the current Tauri resource map."""
import argparse
import json
from pathlib import Path


def validate_mapping(mapping):
    destinations = set()
    for source, target in mapping.items():
        target_path = Path(target)
        if (not target.startswith("licenses/") or ".." in target_path.parts
                or target_path.name != Path(source).name):
            raise ValueError("notice resources must preserve source basenames in namespaced directories")
        # WiX omits File/@Name and installs Source's basename. Do not rely on
        # filename remapping that succeeds for deb/NSIS but collides in MSI.
        folded = target.casefold()
        if folded in destinations:
            raise ValueError("case-insensitive notice destination collision")
        destinations.add(folded)


def verify(resources, root):
    config = root / "trainer/src-tauri/tauri.conf.json"
    mapping = json.loads(config.read_text(encoding="utf-8"))["bundle"]["resources"]
    validate_mapping(mapping)
    wanted = {target: (config.parent / source).resolve()
              for source, target in mapping.items() if target.startswith("licenses/")}
    if not wanted or len(wanted) != len(mapping):
        raise ValueError("unexpected or duplicate notice resource destinations")
    actual = set()
    for path in (resources / "licenses").rglob("*"):
        if path.is_symlink():
            raise ValueError("symlinked notice")
        if path.is_file():
            actual.add(path.relative_to(resources).as_posix())
    if actual != set(wanted):
        raise ValueError(f"notice inventory mismatch: missing={set(wanted)-actual}, extra={actual-set(wanted)}")
    for target, source in wanted.items():
        expected = source.read_bytes()
        if not expected or (resources / target).read_bytes() != expected:
            raise ValueError(f"notice bytes differ: {target}")
    return len(wanted)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("resources", type=Path)
    args = parser.parse_args()
    count = verify(args.resources, Path(__file__).resolve().parents[2])
    print(f"PASS extracted installer notices: {count} exact source identities")
