#!/usr/bin/env python3
"""Offline, hash-bound source-external corpus preparation (no dictionary ranking)."""
import argparse
from collections import Counter
from functools import lru_cache
import hashlib
import json
from pathlib import Path
import re
import unicodedata

HERE = Path(__file__).resolve().parent


def normalize(value):
    value = unicodedata.normalize("NFD", value.lower())
    value = value.replace("u\u0308", "v").replace("e\u0302", "ea")
    value = "".join(c for c in value if c not in "\u0300\u0301\u0304\u030c")
    return value.replace("’", "'")


def parse_syllables(raw, count, inventory):
    value = normalize(raw)
    if (not re.fullmatch("[a-z]+(?:'[a-z]+)*", value)
            or len(value) > 512 or not 1 <= count <= 64):
        return []
    @lru_cache(None)
    def walk(offset, remaining):
        if offset == len(value):
            return [()] if remaining == 0 else []
        if remaining < 1:
            return []
        found = []
        for end in range(offset + 1, min(len(value), offset + 6) + 1):
            syllable = value[offset:end]
            if syllable not in inventory:
                continue
            next_offset = end + (end < len(value) and value[end] == "'")
            for tail in walk(next_offset, remaining - 1):
                found.append((syllable,) + tail)
                if len(found) == 2:
                    return found  # Only uniqueness matters, never rank by target.
        return found
    return walk(0, count)


def is_hanzi(text):
    return bool(text) and all(
        0x3400 <= ord(c) <= 0x9FFF or 0x20000 <= ord(c) <= 0x323AF for c in text)


def prepare(raw, inventory):
    rows, cases, counts = [], [], Counter()
    sentence_id = ""
    run = []
    serial = 0

    def flush():
        nonlocal serial
        if not run:
            return
        text = "".join(word for word, _, _ in run)
        syllables = tuple(s for _, sequence, _ in run for s in sequence)
        if not 2 <= len(text) <= 64:
            counts["excluded_run_length"] += 1
        else:
            serial += 1
            case = f"{sentence_id}-chunk-{serial}"
            code = "".join(inventory[s] for s in syllables)
            rows.append(f"{case}\t{code}\t{text}")
            cases.append({"id": case, "sentence": sentence_id, "characters": len(text),
                          "proper_name": any(proper for _, _, proper in run)})
            counts["eligible_chunks"] += 1
        run.clear()

    for line in raw.decode("utf-8").splitlines():
        if line.startswith("# sent_id = "):
            flush()
            sentence_id = line.removeprefix("# sent_id = ")
            counts["sentences"] += 1
        elif not line:
            flush()
        elif not line.startswith("#"):
            fields = line.split("\t")
            if len(fields) != 10:
                raise ValueError("malformed CoNLL-U token")
            if not fields[0].isdigit():
                counts["non_integer_nodes"] += 1
                continue
            counts["tokens"] += 1
            word = fields[1]
            if not is_hanzi(word):
                flush()
                counts["mixed_or_non_hanzi_tokens"] += 1
                continue
            misc = dict(field.split("=", 1) for field in fields[9].split("|") if "=" in field)
            options = parse_syllables(misc.get("Translit", ""), len(word), inventory)
            if len(options) != 1:
                flush()
                counts["ambiguous_transliteration_tokens" if options else "unsupported_transliteration_tokens"] += 1
                continue
            run.append((word, options[0], fields[3] == "PROPN"))
    flush()
    return ("\n".join(rows) + "\n").encode(), {"counts": dict(sorted(counts.items())), "cases": cases}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("syllable_codes", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("metadata", type=Path)
    args = parser.parse_args()
    manifest = json.loads((HERE / "ud-source.json").read_text())
    raw = args.source.read_bytes()
    if hashlib.sha256(raw).hexdigest() != manifest["sha256"]:
        raise ValueError("upstream source SHA256 mismatch")
    inventory = dict(line.split("\t") for line in args.syllable_codes.read_text().splitlines())
    if len(inventory) != 406 or any(not re.fullmatch("[a-z]{2}", v) for v in inventory.values()):
        raise ValueError("expected 406 canonical encoded syllables")
    output, metadata = prepare(raw, inventory)
    metadata.update({"source": manifest, "fixture_sha256": hashlib.sha256(output).hexdigest(),
                     "encoder_export_sha256": hashlib.sha256(args.syllable_codes.read_bytes()).hexdigest()})
    args.output.write_bytes(output)
    args.metadata.write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(metadata["counts"], sort_keys=True))


if __name__ == "__main__":
    main()
