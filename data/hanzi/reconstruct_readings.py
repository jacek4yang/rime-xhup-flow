#!/usr/bin/env python3
"""Reconstruct core readings offline from two exact upstream inputs (MIT)."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
import unicodedata

HERE = Path(__file__).resolve().parent
TONE_MARKS = "\u0300\u0301\u0304\u030c"


def normalize(value):
    decomposed = unicodedata.normalize("NFD", value.lower())
    decomposed = decomposed.replace("u\u0308", "v").replace("e\u0302", "ea")
    result = "".join(c for c in decomposed if c not in TONE_MARKS)
    if not re.fullmatch("[a-z]+", result):
        raise ValueError(f"unsupported upstream reading: {value!r}")
    return result


def parse(text):
    result = {}
    for number, line in enumerate(text.splitlines(), 1):
        content = line.split("#", 1)[0].strip()
        if not content:
            continue
        match = re.fullmatch(r"U\+([0-9A-F]{4,6}):\s*(\S+)", content)
        if not match:
            raise ValueError(f"malformed source line {number}: {line!r}")
        scalar = int(match[1], 16)
        if scalar > 0x10FFFF or 0xD800 <= scalar <= 0xDFFF:
            raise ValueError(f"not a Unicode scalar: {scalar}")
        char = chr(scalar)
        if char in result:
            raise ValueError(f"duplicate character: {char}")
        result[char] = {normalize(pinyin) for pinyin in match[2].split(",")}
    return result


def render(primary, dictionary):
    lines = []
    for char, readings in sorted(primary.items()):
        if len(readings) != 1:
            raise ValueError(f"primary must have exactly one reading: {char}")
        if char not in dictionary:
            raise ValueError(f"missing dictionary support: {char}")
        first = next(iter(readings))
        lines.append(f"{char}\t{first}\tprimary")
        for reading in sorted(dictionary[char] - readings):
            lines.append(f"{char}\t{reading}\talt")
    return ("\n".join(lines) + "\n").encode("utf-8")


def reconstruct(source_dir):
    manifest = json.loads((HERE / "upstream-inputs.json").read_text())
    sources = {}
    for filename, expected in manifest["files"].items():
        raw = (source_dir / filename).read_bytes()
        if hashlib.sha256(raw).hexdigest() != expected:
            raise ValueError(f"upstream SHA256 mismatch: {filename}")
        sources[filename] = parse(raw.decode("utf-8"))
    output = render(sources["kMandarin_8105.txt"], sources["kTGHZ2013.txt"])
    if len(sources["kMandarin_8105.txt"]) != 8105 or len(output.splitlines()) != 8580:
        raise ValueError("pinned reconstruction changed core cardinalities")
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", required=True, type=Path)
    parser.add_argument("--check", type=Path)
    args = parser.parse_args()
    output = reconstruct(args.source_dir)
    if args.check:
        if output != args.check.read_bytes():
            raise ValueError(f"reconstructed bytes differ: {args.check}")
        print("PASS independently reconstructed core readings: 8105 characters / 8580 rows")
    else:
        sys.stdout.buffer.write(output)


if __name__ == "__main__":
    main()
