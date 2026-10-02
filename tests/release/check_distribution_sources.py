#!/usr/bin/env python3
"""Check normal compiled CLI dependency evidence, not unit-test/research binaries."""
import argparse
import csv
import hashlib
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--depfile", type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
with (root / "data/xhup/sources.tsv").open() as stream:
    header = "id kind url revision path blob sha256 license extractor usage priority notes".split()
    rows = list(csv.DictReader((line for line in stream if not line.startswith("#")),
                              fieldnames=header, delimiter="\t"))
restricted = [row for row in rows if row["usage"] == "research-only"]
assert restricted, "source classification must explicitly preserve excluded research sources"
for row in restricted:
    assert row["license"] == "redistribution-not-authorized", row["id"]
    path = root / row["path"]
    assert hashlib.sha256(path.read_bytes()).hexdigest() == row["sha256"], row["id"]
assert any(row["id"] == "sogou-cell-research" for row in restricted)
deps = args.depfile.read_text()
assert "xhup-cli" in deps and "word_codes.rs" in deps, "not a normal CLI build dependency manifest"
assert "data/words/sogou/" not in deps, "restricted payload compiled into distributed CLI"
print("PASS clean-v1: restricted registry identity/hash verified; no Sogou payload in CLI dependencies")
