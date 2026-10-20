"""Exercise the real required-check entry point against GitHub needs results."""

import json
import os
from pathlib import Path
import subprocess
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
GATE = ROOT / "tools/ci/required.py"
JOBS = ("policy", "native", "coverage", "supply-chain", "repository", "secrets", "python-qa")


def successful_results():
    return {job: {"result": "success", "outputs": {}} for job in JOBS}


class RequiredCheckTests(unittest.TestCase):
    def run_gate(self, raw):
        env = dict(os.environ)
        env.pop("NEEDS_JSON", None)
        if raw is not None:
            env["NEEDS_JSON"] = raw
        return subprocess.run(
            [sys.executable, "-I", str(GATE)],
            env=env, capture_output=True, text=True, timeout=10,
        )

    def test_every_successful_expected_job_is_required(self):
        self.assertTrue(GATE.is_file(), "The configured gate entry point must exist")
        result = self.run_gate(json.dumps(successful_results()))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_each_failure_cancellation_skip_or_unknown_result_blocks(self):
        for job in JOBS:
            for state in ("failure", "cancelled", "skipped", "pending", "", None, True):
                with self.subTest(job=job, state=state):
                    results = successful_results()
                    results[job]["result"] = state
                    self.assertNotEqual(self.run_gate(json.dumps(results)).returncode, 0)

    def test_missing_or_extra_job_blocks(self):
        for job in JOBS:
            with self.subTest(missing=job):
                results = successful_results()
                del results[job]
                self.assertNotEqual(self.run_gate(json.dumps(results)).returncode, 0)
        results = successful_results()
        results["unexpected"] = {"result": "success"}
        self.assertNotEqual(self.run_gate(json.dumps(results)).returncode, 0)

    def test_malformed_input_and_job_shapes_block(self):
        for raw in (None, "", "{", "null", "[]", "{}", "true", '"success"'):
            with self.subTest(raw=raw):
                self.assertNotEqual(self.run_gate(raw).returncode, 0)
        for value in (None, [], "success", {}, {"result": ["success"]}):
            with self.subTest(value=value):
                results = successful_results()
                results["native"] = value
                self.assertNotEqual(self.run_gate(json.dumps(results)).returncode, 0)

    def test_untrusted_results_are_not_executed_or_echoed(self):
        marker = "::error::UNTRUSTED_NEEDS_VALUE"
        results = successful_results()
        results["native"]["result"] = marker
        result = self.run_gate(json.dumps(results))
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn(marker, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
