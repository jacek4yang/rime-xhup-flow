#!/usr/bin/env python3
"""Audit known training-sentence overlap without dropping any evaluation case."""
import argparse
from collections import defaultdict
import hashlib
import importlib.util
import json
from pathlib import Path
import unicodedata

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent


def normalize(text):
    return "".join(c for c in unicodedata.normalize("NFKC", text) if c.isalnum())


def scan(cases, sentences):
    exact_index, span_index = defaultdict(set), defaultdict(set)
    result = {}
    for case_id, text in cases.items():
        normalized = normalize(text)
        if not normalized:
            raise ValueError("empty normalized case")
        exact_index[normalized].add(case_id)
        for i in range(len(normalized)-7):
            span_index[normalized[i:i+8]].add(case_id)
        result[case_id] = {"exact_sentence_matches": 0, "shared_8char_span_sentences": 0,
                           "span_check_applicable": len(normalized) >= 8}
    count = 0
    for sentence in sentences:
        count += 1
        value = normalize(sentence)
        for case_id in exact_index.get(value, ()):
            result[case_id]["exact_sentence_matches"] += 1
        hits = set()
        for i in range(len(value)-7):
            hits.update(span_index.get(value[i:i+8], ()))
        for case_id in hits:
            result[case_id]["shared_8char_span_sentences"] += 1
    return {"sentences_scanned": count, "cases": result}


def script(name):
    path = ROOT / "data/corpus/scripts" / name
    spec = importlib.util.spec_from_file_location(path.stem, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_directory", type=Path)
    parser.add_argument("fixture", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    manifest = json.loads((HERE / "training-sources.json").read_text())
    for source in manifest["sources"]:
        raw = (args.source_directory / source["name"]).read_bytes()
        if hashlib.sha256(raw).hexdigest() != source["sha256"]:
            raise ValueError(f"source hash mismatch: {source['name']}")
    from importlib.metadata import version
    if version("opencc-python-reimplemented") != "0.1.7":
        raise ValueError("requires pinned opencc-python-reimplemented==0.1.7")
    fixture = args.fixture.read_bytes()
    cases = {}
    for line in fixture.decode().splitlines():
        case_id, _, text = line.split("\t")
        if case_id in cases:
            raise ValueError("duplicate fixture case")
        cases[case_id] = text
    kd = script("kdconv_to_sentences.py")
    ptt = script("ptt_to_sentences.py")

    def kd_sentences():
        for domain in kd.DOMAINS:
            for split in kd.SPLITS:
                for text in kd.utterances(args.source_directory / f"{domain}_{split}.json"):
                    yield from (s.strip() for s in kd.SENT_SPLIT.split(text) if s.strip())

    skipped = 0
    def ptt_sentences():
        nonlocal skipped
        converter = ptt.OpenCC("t2s")
        with (args.source_directory / "Gossiping-QA-Dataset.txt").open() as stream:
            for line in stream:
                fields = line.rstrip("\n").split("\t")
                if not line.strip():
                    continue
                if len(fields) != 2:
                    skipped += 1
                    continue
                for field in fields:
                    yield from (s.strip() for s in ptt.SENT_SPLIT.split(converter.convert(field)) if s.strip())

    report = {"schema": 1, "sources": manifest,
              "fixture_sha256": hashlib.sha256(fixture).hexdigest(),
              "normalization": "Unicode NFKC then alphanumeric characters only",
              "known_scope_only": True, "kdconv": scan(cases,kd_sentences()),
              "ptt": scan(cases,ptt_sentences())}
    report["ptt"]["skipped_malformed_lines"] = skipped
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2)+"\n")
    for label in ("kdconv", "ptt"):
        values = report[label]["cases"].values()
        print(label, report[label]["sentences_scanned"], "sentences;",
              sum(v["exact_sentence_matches"] > 0 for v in values), "exact case matches;",
              sum(v["shared_8char_span_sentences"] > 0 for v in values), "shared-span cases")


if __name__ == "__main__":
    main()
