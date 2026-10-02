import tempfile
import unittest
from pathlib import Path

from reconstruct_readings import normalize, parse, reconstruct, render


class ReconstructionTests(unittest.TestCase):
    def test_tones_umlaut_and_e_circumflex(self):
        for source, expected in {
            "lǜ": "lv", "nǚ": "nv", "lüè": "lve", "jǔ": "ju",
            "ê̄": "ea", "ế": "ea", "ê̌": "ea", "ề": "ea",
            "ńg": "ng", "ňg": "ng", "ǹg": "ng", "ḿ": "m",
        }.items():
            self.assertEqual(normalize(source), expected)

    def test_comments_do_not_override_primary(self):
        source = parse("U+55EF: ń  # 嗯 -> ǹg\n")
        self.assertEqual(source, {"嗯": {"n"}})
        self.assertEqual(render(source, {"嗯": {"ng"}}), "嗯\tn\tprimary\n嗯\tng\talt\n".encode())

    def test_union_deduplicates_tone_and_sorts(self):
        primary = parse("U+4E50: lè\nU+4E00: yī\n")
        dictionary = parse("U+4E00: yí,yì\nU+4E50: yuè,lè\n")
        self.assertEqual(render(primary, dictionary), "一\tyi\tprimary\n乐\tle\tprimary\n乐\tyue\talt\n".encode())

    def test_invalid_input_refused(self):
        for source in ("U+D800: a", "U+110000: a", "U+4E00: y1", "invalid", "U+4E00: a\nU+4E00: b"):
            with self.subTest(source=source), self.assertRaises(ValueError):
                parse(source)

    def test_missing_dictionary_or_multiple_primary_refused(self):
        with self.assertRaises(ValueError):
            render({"一": {"yi"}}, {})
        with self.assertRaises(ValueError):
            render({"一": {"yi", "ya"}}, {"一": {"yi"}})

    def test_changed_source_refused_before_parsing(self):
        with tempfile.TemporaryDirectory() as directory:
            (Path(directory) / "kMandarin_8105.txt").write_text("U+4E00: yī\n")
            with self.assertRaisesRegex(ValueError, "SHA256 mismatch"):
                reconstruct(Path(directory))


if __name__ == "__main__":
    unittest.main()
