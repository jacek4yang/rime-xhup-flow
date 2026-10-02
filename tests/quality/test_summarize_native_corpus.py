import json
from pathlib import Path
import tempfile
import unittest
from summarize_native_corpus import intervals, rates, read_trace


class SummaryTests(unittest.TestCase):
    def test_quality_misses_in_denominator(self):
        rows = [{"rank": rank, "commit_exact": rank >= 0, "pending_reachable": False,
                 "contract_failures": int(rank == 2)} for rank in (0, 2, -1)]
        self.assertEqual(rates(rows), {"cases": 3, "top1": 1/3, "top3": 2/3, "top256": 2/3,
                                      "commit_exact": 2/3, "pending_reachable": 0, "contract_failures": 1})

    def test_incomplete_duplicate_wrong_mode_rejected(self):
        meta = {"cases": [{"id": "a", "characters": 2}]}
        row = {"case": "a", "event": "corpus_result", "mode": "planner", "rank": 0,
               "keys": 4, "commit_exact": True}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace"
            for content in ("", json.dumps(row)+"\n"+json.dumps(row),
                            json.dumps({**row, "mode": "native-only"}),
                            json.dumps({**row, "case": "extra"}),
                            json.dumps({**row, "keys": 6})):
                path.write_text(content)
                with self.assertRaises(ValueError):
                    read_trace(path, meta, "planner")

    def test_cluster_resampling_is_paired_and_deterministic(self):
        cases = [{"id": str(i), "sentence": str(i//2)} for i in range(6)]
        rows = {c["id"]: {"rank": int(c["id"])%3-1, "commit_exact": False,
                          "pending_reachable": False} for c in cases}
        a = intervals(cases, rows, rows)
        self.assertEqual(a, intervals(cases, rows, rows))
        self.assertEqual(a["clusters_with_eligible_cases"], 3)
        self.assertTrue(all(interval == [0,0] for interval in a["paired_delta"].values()))


if __name__ == "__main__":
    unittest.main()
