#!/usr/bin/env python3
"""Synthetic collector contract tests. No GitHub requests or real acceptance."""
import copy
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import collect_qualification as collector

SOURCE = "a" * 40


class CollectorTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.out = Path(self.temp.name) / "proofs"
        self.decision = dict(repository="owner/repo", approved_by="owner", source_commit=SOURCE,
                             ci_run=11, runtime_run=12, package_run=13)
        self.runs = {i: dict(id=i, run_attempt=2, status="completed", conclusion="success",
                            head_sha=SOURCE, head_branch="main") for i in (11, 12, 13)}
        self.coverage = dict(version=1, revision=SOURCE, run="12.2", plan_sha256="b" * 64,
                             partitions_completed=16, rows=dict(static=1, extended=1, open=1000))
        self.published = copy.deepcopy(self.coverage)
        self.partitions = 16
        self.downloaded = False
        self.collected = False
        self.env = patch.dict(os.environ, GH_REPO="owner/repo", GITHUB_ACTOR="owner")
        self.env.start()
        self.addCleanup(self.env.stop)

    def gh(self, *args):
        if args == ("api", "repos/owner/repo"):
            return json.dumps({"owner": {"login": "owner"}})
        if args[0] == "api" and args[1].endswith("/jobs?per_page=100"):
            self.assertIn("/attempts/2/", args[1])
            return json.dumps(dict(total_count=0, jobs=[]))
        if args[0] == "api":
            return json.dumps(self.runs[int(args[1].rsplit("/", 1)[1])])
        self.assertEqual(args[:3], ("run", "download", "12"))
        self.downloaded = True
        root = Path(args[-1])
        (root / "native-audit-input").mkdir()
        for i in range(self.partitions):
            folder = root / f"native-audit-result-{i}"
            folder.mkdir()
            (folder / f"receipt-{i}.json").write_text("{}")
        complete = root / "native-audit-complete-coverage"
        complete.mkdir()
        (complete / "coverage.json").write_text(json.dumps(self.published))
        return ""

    def run_collector(self, args, *, check):
        self.assertTrue(check)
        self.assertEqual(args[2], "collect")
        self.assertIn("audit_shards.py", args[1])
        self.assertEqual(args[-4:], ["--revision", SOURCE, "--run", "12.2"])
        out = Path(args[args.index("--out") + 1])
        self.assertEqual(len(list(out.glob("receipt-*"))), 16)
        self.collected = True
        (out / "coverage.json").write_text(json.dumps(self.coverage))

    def invoke(self):
        with patch.object(collector, "gh", self.gh), patch.object(collector.subprocess, "run", self.run_collector):
            collector.collect(self.decision, self.out, SOURCE)

    def test_collects_every_partition_and_compares_receipt(self):
        self.invoke()
        self.assertTrue(self.collected)
        self.assertEqual(json.loads((self.out / "coverage.json").read_text()), self.coverage)

    def test_missing_partition_fails_before_collect(self):
        self.partitions = 15
        with self.assertRaises(ValueError): self.invoke()
        self.assertFalse(self.collected)

    def test_extra_partition_fails_before_collect(self):
        self.partitions = 17
        with self.assertRaises(ValueError): self.invoke()
        self.assertFalse(self.collected)

    def test_wrong_coverage_not_written(self):
        self.published["run"] = "12.1"
        with self.assertRaises(ValueError): self.invoke()
        self.assertFalse((self.out / "coverage.json").exists())

    def test_owner_cannot_be_supplied_by_decision_alone(self):
        os.environ["GITHUB_ACTOR"] = "collaborator"
        self.decision["approved_by"] = "collaborator"
        with self.assertRaises(ValueError): self.invoke()
        self.assertFalse(self.downloaded)

    def test_source_cannot_be_supplied_by_decision_alone(self):
        self.decision["source_commit"] = "b" * 40
        with self.assertRaises(ValueError): self.invoke()
        self.assertFalse(self.downloaded)

    def test_unfinished_or_wrong_source_run_stops_before_download(self):
        for key, value in [("status", "in_progress"), ("conclusion", "failure"),
                           ("head_sha", "b" * 40), ("head_branch", "feature")]:
            with self.subTest(key=key):
                self.runs[12][key] = value
                with self.assertRaises(ValueError): self.invoke()
                self.assertFalse(self.downloaded)
                # The failed proof destination is deliberately not reused.
                import shutil
                shutil.rmtree(self.out)
                self.runs[12] = dict(id=12, run_attempt=2, status="completed", conclusion="success",
                                    head_sha=SOURCE, head_branch="main")


if __name__ == "__main__":
    unittest.main()
