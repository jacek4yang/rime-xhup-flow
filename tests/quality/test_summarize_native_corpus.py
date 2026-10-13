import json
from pathlib import Path
import tempfile
import unittest
from summarize_native_corpus import intervals, rates, read_trace, prefix_contract, require_contracts


class SummaryTests(unittest.TestCase):
    def test_quality_misses_in_denominator(self):
        rows = [{"rank": rank, "commit_exact": rank >= 0, "pending_reachable": False,
                 "contract_failures": int(rank == 2)} for rank in (0, 2, -1)]
        self.assertEqual(rates(rows), {"cases": 3, "top1": 1/3, "top3": 2/3, "top256": 2/3,
                                      "commit_exact": 2/3, "pending_reachable": 0, "contract_failures": 1})

    def test_incomplete_duplicate_wrong_mode_rejected(self):
        meta = {"cases": [{"id": "a", "characters": 2}]}
        row = {"case": "a", "event": "corpus_result", "mode": "planner", "rank": 0,
               "keys": 4, "commit_exact": True, "rank_contract": "complete_raw_span_v1",
               "complete_start": 0, "complete_end": 4}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace"
            for content in ("", json.dumps(row)+"\n"+json.dumps(row),
                            json.dumps({**row, "mode": "native-only"}),
                            json.dumps({**row, "case": "extra"}),
                            json.dumps({**row, "keys": 6})):
                path.write_text(content)
                with self.assertRaises(ValueError):
                    read_trace(path, meta, "planner")

    def test_complete_trace_and_missing_step_or_commit(self):
        meta = {"cases": [{"id": "a", "characters": 2}]}
        key = {"case": "a", "key": 97, "keypress_ns": 1, "key_to_menu_ns": 2, "bounded_probe_ns": 3}
        commit = {"case": "a", "event": "selection_commit"}
        result = {"case": "a", "event": "corpus_result", "mode": "planner", "rank": 0,
                  "keys": 4, "commit_exact": True, "rank_contract": "complete_raw_span_v1",
                  "complete_start": 0, "complete_end": 4}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace"
            records = [key]*8 + [commit, result]
            path.write_text("\n".join(map(json.dumps, records)))
            rows, timing = read_trace(path, meta, "planner")
            self.assertEqual(timing["key_to_menu_ns"]["samples"], 8)
            for incomplete in (records[1:], [key]*8 + [result]):
                path.write_text("\n".join(map(json.dumps, incomplete)))
                with self.assertRaises(ValueError):
                    read_trace(path, meta, "planner")

    def test_partial_text_match_is_not_complete_recall(self):
        meta = {"cases": [{"id": "a", "characters": 2}]}
        key = {"case": "a", "key": 97, "keypress_ns": 1, "key_to_menu_ns": 2, "bounded_probe_ns": 3}
        result = {"case": "a", "event": "corpus_result", "mode": "planner", "rank": 0,
                  "keys": 4, "commit_exact": False, "rank_contract": "complete_raw_span_v1",
                  "complete_start": 0, "complete_end": 3, "text_rank": 0, "text_end": 3,
                  "pending_reachable": True, "contract_failures": 0}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace"
            for invalid in (result, {**result, "complete_end": 4, "rank_contract": "text_only"},
                            {**result, "complete_end": 4, "complete_start": 1}):
                path.write_text("\n".join(map(json.dumps, [key]*8 + [invalid])))
                with self.assertRaises(ValueError):
                    read_trace(path, meta, "planner")
            miss = {**result, "rank": -1, "complete_end": 0}
            path.write_text("\n".join(map(json.dumps, [key]*8 + [miss])))
            rows, _ = read_trace(path, meta, "planner")
            self.assertEqual(rates(list(rows.values()))["top256"], 0)

    def test_offsetting_gains_cannot_hide_prefix_harm(self):
        baseline = {"a": {"rank": 0}, "b": {"rank": 1}, "c": {"rank": 9}}
        planner = {"a": {"rank": 1}, "b": {"rank": 0}, "c": {"rank": 3}}
        contract = prefix_contract(planner, baseline)
        self.assertEqual(contract, {"baseline_top5_cases": 2,
                                   "unchanged_rank_cases": 0, "changed_rank_cases": 2})
        report = {"planner": {"all": {"contract_failures": 0}},
                  "native-only": {"all": {"contract_failures": 0}},
                  "native_top5_contract": contract}
        require_contracts(report)  # Negative historical study may be recorded.
        with self.assertRaises(ValueError):
            require_contracts(report, True)
        report["native_top5_contract"] = prefix_contract(baseline, baseline)
        require_contracts(report, True)
        report["native-only"]["all"]["contract_failures"] = 1
        with self.assertRaises(ValueError):
            require_contracts(report)
        with self.assertRaises(ValueError):
            prefix_contract({}, baseline)

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
