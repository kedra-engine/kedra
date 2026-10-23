"""Exercise the real required-check entry point against GitHub needs results."""

import itertools
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
GATE = ROOT / "tools/ci/required.py"
JOBS = ("changes", "policy", "native", "coverage", "supply-chain", "repository", "secrets", "python-test")
FILTERS = ("rust", "python", "markdown", "workflows", "distribution")


def successful_results():
    results = {job: {"result": "success", "outputs": {}} for job in JOBS}
    results["changes"]["outputs"] = {key: "true" for key in FILTERS}
    return results


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

    def test_gate_accepts_actual_workflow_jobs_and_rejects_obsolete_identity(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        # Check the configured boundary, not just a fixture shared with the gate.
        # This is the workflow's explicit single-line needs contract, not a YAML parser.
        needs = re.search(
            r"^  required:\n(?:(?!  \S)[^\n]*\n)*?    needs: \[([^\]\n]+)\]$",
            workflow, re.MULTILINE,
        )
        self.assertIsNotNone(needs, "CI required must declare its explicit job needs")
        jobs = [job.strip() for job in needs.group(1).split(",")]
        self.assertCountEqual(jobs, JOBS)
        for job in jobs:
            self.assertRegex(workflow, rf"(?m)^  {re.escape(job)}:$")
        results = {job: {"result": "success", "outputs": {}} for job in jobs}
        results["changes"]["outputs"] = {key: "true" for key in FILTERS}
        result = self.run_gate(json.dumps(results))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        results["python-qa"] = results.pop("python-test")
        self.assertNotEqual(self.run_gate(json.dumps(results)).returncode, 0)

    def test_only_jobs_not_selected_by_changed_paths_may_be_skipped(self):
        for rust, python, markdown in itertools.product((True, False), repeat=3):
            with self.subTest(rust=rust, python=python, markdown=markdown):
                results = successful_results()
                selected = {"rust": rust, "python": python, "markdown": markdown}
                results["changes"]["outputs"].update({
                    key: str(value).lower() for key, value in selected.items()
                })
                for job, wanted in (("native", rust), ("coverage", rust),
                                    ("supply-chain", rust), ("python-test", python),
                                    ("repository", markdown)):
                    results[job]["result"] = "success" if wanted else "skipped"
                result = self.run_gate(json.dumps(results))
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                for job in JOBS:
                    broken = json.loads(json.dumps(results))
                    broken[job]["result"] = "failure"
                    self.assertNotEqual(self.run_gate(json.dumps(broken)).returncode, 0)

    def test_missing_or_malformed_change_selection_blocks(self):
        for outputs in ({}, None, [], "true"):
            with self.subTest(outputs=outputs):
                results = successful_results()
                results["changes"]["outputs"] = outputs
                self.assertNotEqual(self.run_gate(json.dumps(results)).returncode, 0)
        for key in FILTERS:
            with self.subTest(missing=key):
                results = successful_results()
                del results["changes"]["outputs"][key]
                self.assertNotEqual(self.run_gate(json.dumps(results)).returncode, 0)
            for value in (None, "", "invalid", True, [], {}):
                with self.subTest(key=key, value=value):
                    results = successful_results()
                    results["changes"]["outputs"][key] = value
                    self.assertNotEqual(self.run_gate(json.dumps(results)).returncode, 0)

    def test_valid_step_selections_do_not_change_job_requirements(self):
        for workflows, distribution in itertools.product(("true", "false"), repeat=2):
            with self.subTest(workflows=workflows, distribution=distribution):
                results = successful_results()
                results["changes"]["outputs"].update({
                    "workflows": workflows, "distribution": distribution,
                })
                result = self.run_gate(json.dumps(results))
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
