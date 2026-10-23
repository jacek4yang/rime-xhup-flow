import hashlib
from pathlib import Path
import tempfile
import unittest

from prepare_legal_corpus import dictionary_indexes, prepare
from prepare_ud_corpus import partition


def sentence(identity, words):
    return (f"# sent_id = {identity}\n" + "".join(
        f"{i}\t{word}\t_\t{tag}\t_\t_\t0\troot\t_\t_\n"
        for i, (word, tag) in enumerate(words, 1)) + "\n").encode()


class LegalCorpus(unittest.TestCase):
    def test_code_precedence_and_frozen_dictionary_rejection(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sources = {
                "xhup_flow_chars.dict.yaml": "你\tnirc\t100\n你\tnixx\t90\n好\thczz\t100\n好\thcaa\t100\n",
                "xhup_flow_words.dict.yaml": "你好\tnihc\t999\n你好\tnh\t1\n你好\tni\t100\n你好\tnf\t100\n",
            }
            identities = {}
            for name, body in sources.items():
                raw = ("---\n...\n" + body).encode()
                (root / name).write_bytes(raw)
                identities[name] = hashlib.sha256(raw).hexdigest()
            full, aliases = dictionary_indexes(root, identities)
            self.assertEqual(full, {"你": "nirc", "好": "hcaa"})
            self.assertEqual(aliases["你好"], "nf")
            (root / "xhup_flow_chars.dict.yaml").write_text("---\n...\n你\tnixx\t999\n")
            with self.assertRaises(ValueError):
                dictionary_indexes(root, identities)

    def test_modes_follow_token_boundaries_without_rank_filtering(self):
        raw = sentence("a", [("你好", "PROPN"), ("去", "VERB"), ("就是", "VERB"),
                             ("。", "PUNCT"), ("你", "PRON"), ("好", "ADJ")])
        full = dict(zip("你好去就是", ("aaaa", "bbbb", "cccc", "dddd", "eeee")))
        fixture, metadata = prepare(raw, full, {"你好": "nh", "去": "q", "就是": "jqu", "你": "n", "好": "h"})
        self.assertEqual(fixture.decode().splitlines(), [
            "a-run-1-full\taaaabbbbccccddddeeee\t你好去就是",
            "a-run-1-alternating\tnhccccjqu\t你好去就是",
            "a-run-1-shortest\tnhqjqu\t你好去就是",
            "a-run-2-full\taaaabbbb\t你好",
            "a-run-2-alternating\tnbbbb\t你好",
            "a-run-2-shortest\tnh\t你好",
        ])
        self.assertTrue(metadata["cases"][0]["proper_name"])
        self.assertFalse(metadata["cases"][3]["proper_name"])
        self.assertEqual(metadata["cases"][1]["segments"], [
            {"text": "你好", "keys": "nh", "source": "static_alias"},
            {"text": "去", "keys": "cccc", "source": "full"},
            {"text": "就是", "keys": "jqu", "source": "static_alias"},
        ])

    def test_unsupported_and_overlength_runs_are_retained_not_truncated(self):
        raw = (sentence("a", [("你龘", "NOUN")]) +
               sentence("e", [("你" * 33, "NOUN"), ("。", "PUNCT"), ("你", "PRON")]))
        fixture, metadata = prepare(raw, {"你": "aaaa"}, {"你龘": "ng"})
        self.assertEqual(fixture, b"")
        self.assertEqual([(item["text"], item["reason"]) for item in metadata["excluded"]],
                         [("你龘", "missing_full_code"), ("你" * 33, "run_length"), ("你", "run_length")])
        self.assertEqual(metadata["excluded"][0]["missing_characters"], ["龘"])

    def test_all_modes_stay_with_parent_and_identical_inputs_are_retained(self):
        raw = sentence("a", [("你好", "NOUN")]) + sentence("e", [("你好", "NOUN")])
        fixture, metadata = prepare(raw, {"你": "aaaa", "好": "bbbb"}, {})
        groups = partition(fixture, metadata)
        self.assertEqual([line.split(b"\t")[0] for line in groups["development"].splitlines()],
                         [b"a-run-1-full", b"a-run-1-alternating", b"a-run-1-shortest"])
        self.assertEqual([line.split(b"\t")[0] for line in groups["confirmation"].splitlines()],
                         [b"e-run-2-full", b"e-run-2-alternating", b"e-run-2-shortest"])
        self.assertTrue(all(case["same_input_as_full"] for case in metadata["cases"]))


if __name__ == "__main__":
    unittest.main()
