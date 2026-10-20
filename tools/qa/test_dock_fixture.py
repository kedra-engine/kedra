"""Headless tests of capture readiness and independent-reference safety."""
import json
import contextlib
import io
import http.client
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from urllib.error import HTTPError, URLError
from unittest.mock import Mock, patch
import dock_fixture as dock
import docking_states


class CapturePolicy(unittest.TestCase):
    def client(self):
        client = object.__new__(dock.Fixture)
        client.frame = Mock(return_value={"path": "unused.png", "frame": 42})
        client.fixture=Mock(return_value={"feedback_ready":True})
        return client

    def test_requires_three_consecutive_identical_frames(self):
        client = self.client()
        with patch.object(dock, "digest", side_effect=["a", "b", "a", "a", "a"]):
            frame, sha = client.stable(7)
        self.assertEqual(client.frame.call_count, 5)
        self.assertEqual((frame["frame"], sha), (42, "a"))

    def test_never_accepts_an_unsettled_renderer(self):
        client = self.client()
        with patch.object(dock, "digest", side_effect=["a", "b"]*8):
            with self.assertRaisesRegex(AssertionError, "never settled"):
                client.stable(7)
        self.assertEqual(client.frame.call_count, 16)

    def test_figma_exports_cover_every_named_state_and_are_unmodified(self):
        references = dock.verify_references()
        policy = json.loads(dock.POLICY.read_text())
        self.assertEqual(len(references), 20)
        self.assertEqual(set(references), set(policy["critical_regions"]))
        self.assertEqual(policy["exclusions"], [])
        self.assertEqual(policy["comparison"]["max_excluded_ppm"], 0)

    def test_comparator_receives_figma_bytes_not_the_application_as_reference(self):
        entry = dock.verify_references()["i00-baseline"]
        client = Mock()
        client.fixture.return_value = {"renderer": {"backend": "Test", "name": "NoGPU"}}
        before = dock.digest(dock.REFERENCES/"i00-baseline.png")
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            executable = directory / "configured target" / "xtask.exe"
            with patch.object(dock.platform,"platform",return_value="headless-unit-test"), patch.object(dock.subprocess, "run", return_value=Mock(returncode=1, stdout="mismatch", stderr="")) as run:
                matched = dock.compare("i00-baseline", entry, {"path": "not-a-reference.png", "frame": 9}, "a"*64, client, directory, executable=executable)
            self.assertEqual(run.call_args.args[0][0], str(executable))
            self.assertFalse(matched)
            config = json.loads((directory/"i00-baseline.config.json").read_text())
            evidence = json.loads((directory/"i00-baseline.evidence.json").read_text())
            self.assertEqual(config["reference"]["kind"], "design_reference")
            self.assertEqual(config["reference"]["sha256"], before)
            self.assertNotEqual(config["reference"]["image"], evidence["image"])
            self.assertEqual(dock.digest(directory/config["reference"]["image"]), before)
            self.assertFalse(Path(evidence["image"]).is_absolute())
        self.assertEqual(dock.digest(dock.REFERENCES/"i00-baseline.png"), before)


class PreviewReadiness(unittest.TestCase):
    def client(self, observations):
        client = object.__new__(dock.Fixture)
        client.node = Mock(side_effect=observations)
        client.window = Mock(return_value=1)
        client.frame = Mock(return_value={"path": "unused.png", "frame": 42})
        client.fixture = Mock(return_value={"dragging": True, "feedback_ready": True})
        client.wait = Mock(side_effect=lambda predicate, label: predicate())
        return client

    def preview(self, x=0):
        return {"entity": 7, "window": 1, "rect": [x, 0, x+100, 100]}

    def test_transient_preview_is_rechecked_after_readback_and_returns_checked_node(self):
        old, settled = self.preview(), self.preview(10)
        # A glimpse disappears; an initially visible rectangle then changes.
        client = self.client([old, None, old, old] + [settled, settled]*3)
        with patch.object(dock, "digest", return_value="a"):
            preview = client.settle_preview()
        self.assertIs(preview, settled)
        self.assertEqual(client.frame.call_count, 5)
        self.assertEqual(client.node.call_count, 10)

    def test_preview_never_seen_remains_a_bounded_diagnostic_failure(self):
        client = self.client([None]*32)
        with patch.object(dock, "digest", return_value="a"):
            with self.assertRaisesRegex(AssertionError, "Preview never settled.*dragging"):
                client.settle_preview()
        self.assertEqual(client.frame.call_count, 16)

    def test_visible_preview_with_unsettled_renderer_is_not_ready(self):
        client = self.client([self.preview()]*32)
        with patch.object(dock, "digest", side_effect=["a", "b"]*8):
            with self.assertRaisesRegex(AssertionError, "Preview never settled"):
                client.settle_preview()
        self.assertEqual(client.frame.call_count, 16)

    def test_rectangle_change_during_readback_resets_readiness(self):
        old, settled = self.preview(), self.preview(10)
        client = self.client([old, old]*2 + [old, settled] + [settled, settled]*3)
        with patch.object(dock, "digest", return_value="a"):
            self.assertIs(client.settle_preview(), settled)
        self.assertEqual(client.frame.call_count, 6)


class Orchestrator(unittest.TestCase):
    def run_suite(self, visuals=(True, True), names=None, scenario_error=None,
                  cleanup_error=None, waits=None, binaries=None, build_error=None):
        """Run the real CLI orchestrator without Cargo, RPC, or native windows."""
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)/"suite"
            process, log, client = Mock(), Mock(), Mock()
            process.wait.return_value = 0
            client.fixture.return_value = {"ready": True, "project_tools": False,
                                           "preferences": False}
            client.observe.return_value = {}
            client.state.return_value = {}
            client.stable.return_value = ({"path": "unused.png", "frame": 9,
                                           "width": 1200, "height": 800}, "a"*64)
            client.event.side_effect = cleanup_error
            if waits is not None:
                process.wait.side_effect = waits
            runs = iter(names or [("baseline",), ("baseline",)])

            def launch(binary, delivery, path):
                if binaries:
                    self.assertEqual(binary, binaries[0])
                path.mkdir()
                return process, log, client

            def compare(*args, **kwargs):
                if binaries:
                    self.assertEqual(kwargs["executable"], binaries[1])
                return next(visual_results)

            visual_results = iter(visuals)

            def scenarios(c, save):
                if scenario_error:
                    raise scenario_error
                for name in next(runs):
                    save(name, 1)

            with contextlib.ExitStack() as stack:
                stack.enter_context(patch.object(sys, "argv", ["dock_fixture.py", "--output",
                    str(output), "--delivery", directory]))
                stack.enter_context(patch.object(dock, "verify_references", return_value={
                    "baseline": {"size": [1200, 800]}}))
                stack.enter_context(patch.object(dock.subprocess, "run"))
                stack.enter_context(patch.object(dock, "build_executable",
                    side_effect=build_error or binaries or [Path("fixture-built"), Path("xtask-built")]))
                stack.enter_context(patch.object(dock, "launch", side_effect=launch))
                stack.enter_context(patch.object(dock, "scenarios", side_effect=scenarios))
                stack.enter_context(patch.object(dock.shutil, "copyfile"))
                stack.enter_context(patch.object(dock, "compare", side_effect=compare))
                stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
                stack.enter_context(contextlib.redirect_stderr(io.StringIO()))
                code = dock.main()
            report = json.loads((output/"report.json").read_text())
        return code, report, process, log

    def test_complete_matching_runs_pass(self):
        code, report, process, log = self.run_suite()
        self.assertEqual(code, 0)
        self.assertEqual(report["status"], "passed")
        self.assertEqual(report["visual_policy"], "accepted")
        self.assertEqual(process.wait.call_count, 2)
        self.assertEqual(log.close.call_count, 2)

    def test_launches_only_cargo_reported_artifacts(self):
        code, report, _, _ = self.run_suite(binaries=[
            Path("/custom target/cross-target/debug/examples/fixture.exe"),
            Path("/configured output/host/debug/xtask.exe")])
        self.assertEqual(code, 0, report)
        self.assertEqual(report["status"], "passed")

    def test_successful_f12_receipt_cannot_hide_forced_zero_exit(self):
        timeout = subprocess.TimeoutExpired("fixture", 10)
        for waits, terminations, kills in [([timeout, 0] * 2, 2, 0),
                                           ([timeout, timeout, 0] * 2, 2, 2),
                                           ([timeout, 0, 0], 1, 0),
                                           ([0, timeout, 0], 1, 0)]:
            with self.subTest(waits=waits):
                code, report, process, log = self.run_suite(waits=waits)
                self.assertEqual(code, 1)
                self.assertEqual(report["status"], "failed")
                self.assertTrue(any("forced" in e for e in report["cleanup_errors"]))
                self.assertEqual(process.terminate.call_count, terminations)
                self.assertEqual(process.kill.call_count, kills)
                self.assertTrue(all(c.kwargs == {"timeout": 10}
                                    for c in process.wait.call_args_list))
                self.assertEqual(log.close.call_count, 2)

    def test_missing_cargo_artifact_fails_report_before_any_native_launch(self):
        code, report, process, log = self.run_suite(
            build_error=RuntimeError("Cargo reported 0 executable artifacts"))
        self.assertEqual(code, 1)
        self.assertEqual(report["status"], "failed")
        self.assertIn("0 executable artifacts", report["error"])
        self.assertEqual(report["runs"], [])
        process.wait.assert_not_called()
        log.close.assert_not_called()

    def test_earlier_visual_failure_cannot_be_hidden_by_last_run(self):
        code, report, _, _ = self.run_suite(visuals=(False, True))
        self.assertEqual(code, 1)
        self.assertEqual(report["status"], "failed")
        self.assertEqual([r["baseline"]["visual_passed"] for r in report["runs"]],
                         [False, True])

    def test_last_visual_failure_and_all_strict_mismatches_still_fail(self):
        for visuals in [(True, False), (False, False)]:
            with self.subTest(visuals=visuals):
                code, report, _, _ = self.run_suite(visuals=visuals)
                self.assertEqual(code, 1)
                self.assertEqual(report["status"], "failed")

    def test_missing_scenario_in_any_run_fails(self):
        for names in [((), ("baseline",)), (("baseline",), ())]:
            with self.subTest(names=names):
                code, report, _, _ = self.run_suite(names=names)
                self.assertEqual(code, 1)
                self.assertEqual(report["not_captured"], ["baseline"])

    def test_cleanup_rpc_error_reaps_process_and_preserves_scenario_error(self):
        for failure in [RuntimeError("stale window"), [None, RuntimeError("exit RPC failed")]]:
            with self.subTest(failure=failure):
                code, report, process, log = self.run_suite(
                    scenario_error=AssertionError("original mechanics failure"),
                    cleanup_error=failure)
                self.assertEqual(code, 1)
                self.assertIn("original mechanics failure", report["error"])
                process.wait.assert_called_once_with(timeout=10)
                log.close.assert_called_once()

    def test_cleanup_escalates_to_kill_without_replacing_scenario_error(self):
        timeout = subprocess.TimeoutExpired("fixture", 10)
        code, report, process, log = self.run_suite(
            scenario_error=AssertionError("original mechanics failure"),
            waits=[timeout, timeout, 0])
        self.assertEqual(code, 1)
        self.assertIn("original mechanics failure", report["error"])
        process.terminate.assert_called_once()
        process.kill.assert_called_once()
        self.assertEqual(process.wait.call_count, 3)
        self.assertTrue(all(c.kwargs == {"timeout": 10} for c in process.wait.call_args_list))
        log.close.assert_called_once()

    def test_unreapable_process_closes_log_and_keeps_primary_failure(self):
        timeout = subprocess.TimeoutExpired("fixture", 10)
        code, report, process, log = self.run_suite(
            scenario_error=AssertionError("original mechanics failure"),
            waits=[timeout, timeout, timeout])
        self.assertEqual(code, 1)
        self.assertIn("original mechanics failure", report["error"])
        self.assertTrue(report["cleanup_errors"])
        process.kill.assert_called_once()
        log.close.assert_called_once()

    def test_cleanup_failure_alone_cannot_report_a_successful_suite(self):
        code, report, process, log = self.run_suite(cleanup_error=RuntimeError("stale window"))
        self.assertEqual(code, 1)
        self.assertTrue(report["cleanup_errors"])
        self.assertEqual(process.wait.call_count, 2)
        self.assertEqual(log.close.call_count, 2)

    def test_f12_disconnect_with_unforced_zero_exit_is_success(self):
        for disconnect in [URLError(ConnectionRefusedError(61, "fixture closed")),
                           http.client.RemoteDisconnected("fixture closed")]:
            with self.subTest(disconnect=disconnect):
                code, report, process, log = self.run_suite(
                    cleanup_error=[None, disconnect, None, disconnect])
                self.assertEqual(code, 0)
                self.assertNotIn("cleanup_errors", report)
                process.terminate.assert_not_called()
                process.kill.assert_not_called()
                self.assertEqual(log.close.call_count, 2)

    def test_expected_f12_disconnect_preserves_original_scenario_failure(self):
        code, report, _, log = self.run_suite(
            scenario_error=AssertionError("original mechanics failure"),
            cleanup_error=[None, ConnectionResetError("fixture closed")])
        self.assertEqual(code, 1)
        self.assertIn("original mechanics failure", report["error"])
        self.assertNotIn("cleanup_errors", report)
        log.close.assert_called_once()

    def test_first_button_disconnect_is_not_a_successful_exit_receipt(self):
        code, report, _, _ = self.run_suite(cleanup_error=ConnectionRefusedError("pre-exit"))
        self.assertEqual(code, 1)
        self.assertTrue(report["cleanup_errors"])

    def test_f12_disconnect_with_abnormal_exit_or_escalation_still_fails(self):
        disconnect = ConnectionResetError("fixture closed")
        timeout = subprocess.TimeoutExpired("fixture", 10)
        for waits in [[7, 7], [timeout, 0, timeout, 0]]:
            with self.subTest(waits=waits):
                code, report, _, _ = self.run_suite(
                    cleanup_error=[None, disconnect, None, disconnect], waits=waits)
                self.assertEqual(code, 1)
                self.assertTrue(report["cleanup_errors"])

    def test_f12_rpc_application_error_is_not_treated_as_disconnect(self):
        for error in [RuntimeError("app.input rejected"), URLError(TimeoutError("timeout")),
                       HTTPError("http://localhost", 403, "rejected", {}, None)]:
            if isinstance(error, HTTPError):
                self.addCleanup(error.close)
            with self.subTest(error=error):
                code, report, _, _ = self.run_suite(
                    cleanup_error=[None, error, None, error])
                self.assertEqual(code, 1)
                self.assertTrue(report["cleanup_errors"])

    def test_abnormal_exit_without_transport_error_is_reported(self):
        code, report, _, _ = self.run_suite(waits=[7, 7])
        self.assertEqual(code, 1)
        self.assertTrue(report["cleanup_errors"])

    def test_launch_timeout_kills_stubborn_process_and_closes_log(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)/"run"
            process = Mock()
            process.poll.return_value = None
            timeout = subprocess.TimeoutExpired("fixture", 10)
            process.wait.side_effect = [timeout, timeout, 0]
            with patch.object(dock.subprocess, "Popen", return_value=process) as popen, \
                    patch.object(dock.time, "monotonic", side_effect=[0, 41]):
                with self.assertRaisesRegex(AssertionError, "failed to start"):
                    dock.launch(Path("unused"), Path(directory), output)
            self.assertTrue(popen.call_args.kwargs["stdout"].closed)
            process.kill.assert_called_once()

    def test_failed_process_spawn_closes_opened_log(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(dock.subprocess, "Popen", side_effect=OSError("spawn failed")) as popen:
                with self.assertRaisesRegex(OSError, "spawn failed"):
                    dock.launch(Path("unused"), Path(directory), Path(directory)/"run")
            self.assertTrue(popen.call_args.kwargs["stdout"].closed)

    def test_initial_rpc_failure_reaps_process_and_preserves_original_error(self):
        with tempfile.TemporaryDirectory() as directory:
            process = Mock()
            timeout = subprocess.TimeoutExpired("fixture", 10)
            process.wait.side_effect = [timeout, timeout, 0]

            def spawn(*args, **kwargs):
                (Path(directory)/"run/session.json").write_text("{}")
                return process

            with patch.object(dock.subprocess, "Popen", side_effect=spawn) as popen, \
                    patch.object(dock, "Fixture", side_effect=RuntimeError("initial RPC failure")):
                with self.assertRaisesRegex(RuntimeError, "initial RPC failure"):
                    dock.launch(Path("unused"), Path(directory), Path(directory)/"run")
            self.assertTrue(popen.call_args.kwargs["stdout"].closed)
            process.kill.assert_called_once()


class NativeDraftScenarios(unittest.TestCase):
    def client(self, before, after, entity_after=7):
        client = Mock()
        fields = iter([{"id": "draft", "entity": 7, "value": before},
                       {"id": "draft", "entity": entity_after, "value": after}])
        def node(identity):
            if identity == "draft" or identity.endswith("/draft"):
                return next(fields)
            return {"rect": [0, 0, 28, 28]}
        client.node.side_effect = node
        client.state.return_value = {"layout": "before"}
        client.finish_drag.side_effect = lambda window: setattr(client.state, "return_value", {"layout": "after"})
        client.wait.side_effect = lambda predicate, label: predicate()
        client.group.side_effect = lambda view: "target" if view == "assets" else "result"
        client.group_id.side_effect = lambda group: group
        return client

    def test_native_scenarios_reject_ignored_text_before_drag(self):
        for scenario, expected in [("fixture", "Retained native draft"),
                                   ("states", "Retained docking state")]:
            with self.subTest(scenario=scenario):
                client = self.client("", "")
                with self.assertRaisesRegex(AssertionError, "text input was not delivered"):
                    self.scenario(scenario, client)
                client.start_drag.assert_not_called()

    def test_native_scenarios_reject_lost_text_or_recreated_entity_after_drop(self):
        for scenario, expected in [("fixture", "Retained native draft"),
                                   ("states", "Retained docking state")]:
            for after, entity in [("", 7), (expected, 8)]:
                with self.subTest(scenario=scenario, after=after, entity=entity):
                    client = self.client(expected, after, entity)
                    with self.assertRaises(AssertionError):
                        self.scenario(scenario, client)
                    client.finish_drag.assert_called_once()

    def test_native_scenarios_accept_exact_nonempty_retained_text(self):
        for scenario, expected in [("fixture", "Retained native draft"),
                                   ("states", "Retained docking state")]:
            with self.subTest(scenario=scenario):
                client = self.client(expected, expected)
                self.scenario(scenario, client)
                client.event.assert_any_call(kind="text", text=expected)
                client.finish_drag.assert_called_once()
                if scenario == "states":
                    self.assertEqual(client.event.call_args_list[0].kwargs,
                                     {"kind": "key", "key": "select_all"})

    def scenario(self, name, client):
        if name == "fixture":
            dock.draft_retention(client, "notes", "assets")
        else:
            docking_states.compass_drop(client, {"notes": "notes", "assets": "assets"}, "left")


if __name__ == "__main__":
    unittest.main()
