import unittest
from prepare_ud_corpus import normalize, parse_syllables, prepare


class PreparationTests(unittest.TestCase):
    def test_normalization(self):
        self.assertEqual(normalize("Lǜ nüé Ê ’"), "lv nve ea '")

    def test_ambiguity_not_arbitrarily_resolved(self):
        inventory = dict.fromkeys(["xi", "xian", "an", "a"])
        self.assertEqual(set(parse_syllables("xiana", 2, inventory)),
                         {("xian", "a")})
        inventory = dict.fromkeys(["a", "aa"])
        self.assertEqual(len(parse_syllables("aaa", 2, inventory)), 2)

    def test_apostrophe_is_hard_boundary(self):
        inventory = dict.fromkeys(["xi", "an", "xian"])
        self.assertEqual(parse_syllables("xi'an", 2, inventory), [("xi", "an")])
        self.assertEqual(parse_syllables("xi'an", 1, inventory), [])
        for malformed in ["'xi", "xi'", "xi''an", "xi3", ""]:
            self.assertEqual(parse_syllables(malformed, 1, inventory), [])

    def test_resource_limits(self):
        inventory = {"a": "aa"}
        for raw, count in [("a" * 513, 64), ("a" * 65, 65), ("a", 0), ("a", -1)]:
            self.assertEqual(parse_syllables(raw, count, inventory), [])

    def test_exclusions_and_all_eligible_chunks(self):
        def token(i, text, translit, pos="NOUN"):
            return f"{i}\t{text}\t_\t{pos}\t_\t_\t0\troot\t_\tTranslit={translit}\n"
        source = "# sent_id = independent\n"
        source += token(1, "你好", "nǐhǎo")
        source += token(2, "。", "_")
        source += token(3, "你", "nǐ", "PROPN") + token(4, "好", "hǎo")
        source += token(5, "字", "unsupported")
        source += token(6, "你", "ni")
        output, meta = prepare(source.encode(), {"ni": "ni", "hao": "hc"})
        self.assertEqual(output.decode().splitlines(),
                         ["independent-chunk-1\tnihc\t你好", "independent-chunk-2\tnihc\t你好"])
        self.assertEqual(meta["counts"], {"eligible_chunks": 2, "excluded_run_length": 1,
                         "mixed_or_non_hanzi_tokens": 1, "sentences": 1,
                         "tokens": 6, "unsupported_transliteration_tokens": 1})
        self.assertEqual([c["proper_name"] for c in meta["cases"]], [False, True])

    def test_no_target_frequency_selection(self):
        source = ("# sent_id = ambiguous\n"
                  "1\t甲乙\t_\tNOUN\t_\t_\t0\troot\t_\tTranslit=aaa\n")
        output, meta = prepare(source.encode(), {"a": "aa", "aa": "bb"})
        self.assertEqual(output, b"\n")
        self.assertEqual(meta["counts"]["ambiguous_transliteration_tokens"], 1)


if __name__ == "__main__":
    unittest.main()
