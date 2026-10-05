"""Reverse exact reviewed warning fixes before existing upstream/security hashes.

No upstream inventory, security-backport hash, lockfile, or lint level is changed.
"""
import json
from pathlib import Path

PATCHES = json.loads(Path(__file__).with_name("rust-warning-patches.json").read_text())


def original_before_warning_fixes(relative, data):
    for before, after in reversed(PATCHES.get(relative, [])):
        assert before and after and before != after, f"invalid warning patch: {relative}"
        assert data.count(after.encode()) == 1, f"missing/changed warning patch: {relative}"
        data = data.replace(after.encode(), before.encode(), 1)
    return data
