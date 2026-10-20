"""Accepted policy is explicit and scoped; replay never manufactures native success."""
import contextlib
import copy
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import dock_fixture
import dock_visual
import dock_visual_policy


class ApprovedPolicy(unittest.TestCase):
    def test_all_scenarios_are_explicit_capped_and_strict_budgets_are_unchanged(self):
        strict = json.loads(dock_fixture.POLICY.read_text())
        policy = json.loads(dock_visual_policy.PATH.read_text())
        references = dock_fixture.verify_references()
        self.assertEqual(set(policy["text_interiors"]), set(references))
        self.assertEqual({name for name, rects in policy["text_interiors"].items() if rects},
                         set(policy["required_text"]))
        self.assertLessEqual(policy["max_excluded_ppm"], 15000)
        for name, entry in references.items():
            config = {"fixture": strict["fixture"], "scenario": name,
                      "reference": {"approval": "pending"},
                      "comparison": strict["comparison"],
                      "critical_regions": strict["critical_regions"][name], "exclusions": []}
            before = copy.deepcopy(config)
            accepted = dock_visual_policy.accepted(config, policy["reference_revision"])
            self.assertEqual(config, before)
            self.assertEqual(accepted["critical_regions"], config["critical_regions"])
            for key in ("budget", "normalized_yiq_tolerance", "allow_uniform_reason"):
                self.assertEqual(accepted["comparison"][key], config["comparison"][key])
            area = 0
            for exclusion in accepted["exclusions"]:
                x, y, w, h = exclusion["rect"]
                self.assertGreater(w*h, 0)
                self.assertTrue(0 <= x < x+w <= entry["size"][0])
                self.assertTrue(0 <= y < y+h <= entry["size"][1])
                self.assertEqual(exclusion["kind"], "approved_visual_variance")
                self.assertTrue(exclusion["reason"])
                area += w*h
            self.assertLessEqual(area*1_000_000,
                                 entry["size"][0]*entry["size"][1]*15000)
        self.assertEqual(strict["exclusions"], [])
        self.assertEqual(strict["comparison"]["max_excluded_ppm"], 0)

    def test_unknown_scenario_and_changed_reference_revision_fail_closed(self):
        config = {"fixture": "docking.fixture.v1", "scenario": "unknown"}
        with self.assertRaises(ValueError):
            dock_visual_policy.accepted(config, "changed")
        revision = json.loads(dock_visual_policy.PATH.read_text())["reference_revision"]
        with self.assertRaises(KeyError):
            dock_visual_policy.accepted(config, revision)

    def test_font_variance_never_accepts_missing_changed_hidden_or_ambiguous_caption(self):
        element = {"id": "dock.drag/title", "text": "Panel C", "visible": True}
        dock_visual_policy.validate_text("i02-compass", [element])
        for elements in ([], [dict(element, text="")], [dict(element, visible=False)],
                         [element, element], [dict(element, text="Wrong panel")]):
            with self.assertRaises(ValueError):
                dock_visual_policy.validate_text("i02-compass", elements)


class Replay(unittest.TestCase):
    def run_replay(self, *, corrupted=False, cleanup=False, missing=False, matched=True,
                   mode="accepted"):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, output = root/"source", root/"output"
            source.mkdir()
            sha = dock_fixture.digest(dock_fixture.REFERENCES/"i00-baseline.png")
            report = {"stability": "passed", "mechanics": "passed (recorded native input)",
                      "runs": [{"i00-baseline": {"sha256": sha}} for _ in range(2)]}
            profile = {"logical_size": [1200, 800], "dpr": 1, "environment": "original OS"}
            for index in (1, 2):
                directory = source/f"run-{index}"
                directory.mkdir()
                image = directory/"i00-baseline.actual.png"
                image.write_bytes((dock_fixture.REFERENCES/"i00-baseline.png").read_bytes())
                (directory/"i00-baseline.observe.json").write_text(json.dumps({"fixture": {}, "ui": {"elements": []}}))
                (directory/"i00-baseline.evidence.json").write_text(json.dumps({
                    "sha256": sha, "scenario": "i00-baseline", "fixture": "docking.fixture.v1",
                    "profile": profile, "receipt_frame": 42}))
                if corrupted:
                    image.write_bytes(b"replaced frame")
            if missing:
                report["runs"][1] = {}
            if cleanup:
                report["cleanup_errors"] = ["forced termination"]
            report_file = source/"report.json"
            report_file.write_text(json.dumps(report))
            before = report_file.read_bytes()
            refs = {"i00-baseline": {"size": [1200, 800]}}
            with contextlib.ExitStack() as stack:
                stack.enter_context(patch.object(sys, "argv", ["dock_visual.py", "--captures",
                    str(source), "--output", str(output), "--visual-policy", mode]))
                stack.enter_context(patch.object(dock_visual, "verify_references", return_value=refs))
                stack.enter_context(patch.object(dock_visual, "build_executable", return_value=Path("built")))
                compare = stack.enter_context(patch.object(dock_visual, "compare", return_value=matched))
                stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
                if cleanup:
                    with self.assertRaisesRegex(ValueError, "normal cleanup"):
                        dock_visual.main()
                    self.assertFalse(output.exists())
                    return
                code = dock_visual.main()
            self.assertEqual(before, report_file.read_bytes())
            result = json.loads((output/"report.json").read_text())
            if corrupted or missing:
                self.assertEqual(code, 1)
                self.assertIn("error", result)
            else:
                self.assertEqual(code, 0 if matched else 1)
                self.assertEqual(compare.call_count, 2)
                self.assertEqual(compare.call_args.kwargs["capture_profile"], profile)
                self.assertEqual(compare.call_args.kwargs["visual_policy"], mode)
                self.assertEqual(result["kind"], "offline_visual_recomparison")
                self.assertEqual(result["source_report_sha256"], dock_fixture.digest(report_file))

    def test_accepted_replay_keeps_original_provenance_and_does_not_claim_new_native_input(self):
        self.run_replay()

    def test_strict_replay_and_accepted_mismatch_remain_red(self):
        for mode in ("strict", "accepted"):
            self.run_replay(mode=mode, matched=False)

    def test_changed_frames_missing_scenarios_and_bad_cleanup_are_never_green(self):
        for argument in ("corrupted", "cleanup", "missing"):
            self.run_replay(**{argument: True})
