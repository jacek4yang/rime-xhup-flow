import unittest
from check_corpus_overlap import normalize, scan


class OverlapTests(unittest.TestCase):
    def test_normalization(self):
        self.assertEqual(normalize("你，好！ＡＢ１２"), "你好AB12")

    def test_exact_and_shared_span_separate(self):
        report = scan({"short": "你好", "long": "甲乙丙丁戊己庚辛壬", "other": "未知输入"},
                      ["你，好", "前甲乙丙丁戊己庚辛后", "再甲乙丙丁戊己庚辛再甲乙丙丁戊己庚辛"])
        self.assertEqual(report["sentences_scanned"], 3)
        self.assertEqual(report["cases"]["short"], {"exact_sentence_matches": 1,
                         "shared_8char_span_sentences": 0, "span_check_applicable": False})
        self.assertEqual(report["cases"]["long"]["exact_sentence_matches"], 0)
        self.assertEqual(report["cases"]["long"]["shared_8char_span_sentences"], 2)
        self.assertEqual(report["cases"]["other"]["exact_sentence_matches"], 0)

    def test_repeated_cases_all_retained(self):
        report = scan({"one": "相同文本", "two": "相同文本"}, ["相同文本"])
        self.assertEqual(set(report["cases"]), {"one", "two"})
        self.assertEqual(report["cases"]["one"], report["cases"]["two"])


if __name__ == "__main__":
    unittest.main()
