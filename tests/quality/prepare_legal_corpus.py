#!/usr/bin/env python3
"""Prepare frozen full/mixed legal-code cases, without consulting candidate ranks."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

from prepare_ud_corpus import is_hanzi, partition

HERE = Path(__file__).resolve().parent
MODES = ("full", "alternating", "shortest")


def dictionary_indexes(package, identities):
    full, aliases = {}, {}
    for name, expected in identities.items():
        raw = (package / name).read_bytes()
        if hashlib.sha256(raw).hexdigest() != expected:
            raise ValueError(f"frozen dictionary mismatch: {name}")
        _, marker, body = raw.decode("utf-8").partition("\n...\n")
        if not marker:
            raise ValueError(f"missing dictionary body: {name}")
        for line in body.splitlines():
            if not line or line.startswith("#"):
                continue
            text, code, weight = line.split("\t")
            if not code or any(c < "a" or c > "z" for c in code):
                raise ValueError(f"invalid legal code in {name}")
            weight = int(weight)
            choice = (len(code), -weight, code)
            if text not in aliases or choice < aliases[text]:
                aliases[text] = choice
            if name == "xhup_flow_chars.dict.yaml" and len(text) == 1 and len(code) == 4:
                choice = (-weight, code)
                if text not in full or choice < full[text]:
                    full[text] = choice
    return ({text: choice[-1] for text, choice in full.items()},
            {text: choice[-1] for text, choice in aliases.items()})


def prepare(raw, full, aliases):
    counts, cases, excluded, output = Counter(), [], [], []
    sentence, serial, run = "", 0, []

    def flush():
        nonlocal serial
        if not run:
            return
        serial += 1
        text = "".join(word for word, _ in run)
        identity = f"{sentence}-run-{serial}"
        missing = sorted(set(text) - full.keys())
        reason = ("run_length" if not 2 <= len(text) <= 32 else
                  "missing_full_code" if missing else None)
        if reason:
            counts[f"excluded_{reason}"] += 1
            excluded.append({"id": identity, "sentence": sentence, "text": text,
                             "reason": reason, "missing_characters": missing})
        else:
            counts["eligible_runs"] += 1
            full_input = "".join(full[c] for c in text)
            for mode in MODES:
                segments = []
                for position, (word, _) in enumerate(run):
                    ordinary = "".join(full[c] for c in word)
                    alias = aliases.get(word, ordinary)
                    use_alias = (len(alias) < len(ordinary) and
                                 (mode == "shortest" or mode == "alternating" and position % 2 == 0))
                    if use_alias:
                        segments.append({"text": word, "keys": alias, "source": "static_alias"})
                    else:
                        segments.extend({"text": c, "keys": full[c], "source": "full"} for c in word)
                keys = "".join(segment["keys"] for segment in segments)
                case = f"{identity}-{mode}"
                output.append(f"{case}\t{keys}\t{text}")
                cases.append({"id": case, "sentence": sentence, "mode": mode,
                              "characters": len(text), "keys": len(keys), "proper_name": any(p for _, p in run),
                              "same_input_as_full": keys == full_input, "segments": segments})
                counts[f"cases_{mode}"] += 1
        run.clear()

    for line in raw.decode("utf-8").splitlines():
        if line.startswith("# sent_id = "):
            flush()
            sentence = line.removeprefix("# sent_id = ")
            counts["sentences"] += 1
        elif not line:
            flush()
        elif not line.startswith("#"):
            fields = line.split("\t")
            if len(fields) != 10 or not sentence:
                raise ValueError("malformed CoNLL-U token or missing sentence identity")
            if not fields[0].isdigit():
                counts["non_integer_nodes"] += 1
                continue
            counts["tokens"] += 1
            if not is_hanzi(fields[1]):
                flush()
                counts["mixed_or_non_hanzi_tokens"] += 1
            else:
                run.append((fields[1], fields[3] == "PROPN"))
    flush()
    return ("\n".join(output) + ("\n" if output else "")).encode(), {
        "counts": dict(sorted(counts.items())), "cases": cases, "excluded": excluded}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("package", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--manifest", type=Path, default=HERE / "pud-legal-source.json")
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    raw = args.source.read_bytes()
    blob = hashlib.sha1(f"blob {len(raw)}\0".encode() + raw).hexdigest()
    if blob != manifest["blob"] or hashlib.sha256(raw).hexdigest() != manifest["sha256"]:
        raise ValueError("frozen upstream source identity mismatch")
    notices = {}
    for name, expected in manifest["upstream_notice_sha256"].items():
        notice = (args.source.parent / name).read_bytes()
        if hashlib.sha256(notice).hexdigest() != expected:
            raise ValueError(f"frozen upstream notice mismatch: {name}")
        notices[name] = notice
    full, aliases = dictionary_indexes(args.package, manifest["dictionary_sha256"])
    fixture, metadata = prepare(raw, full, aliases)
    groups = partition(fixture, metadata)
    metadata.update({"source": manifest, "fixture_sha256": hashlib.sha256(fixture).hexdigest(),
                     "preparation_sources": {name: hashlib.sha256((HERE / name).read_bytes()).hexdigest()
                                             for name in ("prepare_legal_corpus.py", "prepare_ud_corpus.py")},
                     "partitions": {name: {"sha256": hashlib.sha256(data).hexdigest(),
                                           "cases": data.count(b"\n")} for name, data in groups.items()}})
    if manifest.get("fixture_sha256", metadata["fixture_sha256"]) != metadata["fixture_sha256"]:
        raise ValueError("frozen legal-code fixture mismatch")
    for name, identity in metadata["partitions"].items():
        if name in manifest and manifest[name] != identity:
            raise ValueError(f"frozen {name} partition mismatch")
    args.output.mkdir(parents=True, exist_ok=False)
    for name, notice in notices.items():
        (args.output / f"upstream-{name}").write_bytes(notice)
    (args.output / "all.tsv").write_bytes(fixture)
    for name, data in groups.items():
        (args.output / f"{name}.tsv").write_bytes(data)
    (args.output / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n",
                                               encoding="utf-8")
    print(json.dumps({"counts": metadata["counts"], "partitions": metadata["partitions"]}, sort_keys=True))


if __name__ == "__main__":
    main()
