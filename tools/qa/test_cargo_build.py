"""Headless Cargo-protocol regressions; no executable or native window is launched."""
import contextlib
import copy
import io
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

from cargo_build import build_executable


class CargoArtifacts(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()
        self.manifest = self.root / "fixture/Cargo.toml"
        # Neither location is guessed by the selector. Suffix and target layout
        # deliberately differ from the machine on which these tests execute.
        self.executable = self.root / "configured output/foreign-triple/debug/example.exe"
        self.executable.parent.mkdir(parents=True)
        self.executable.write_bytes(b"reported artifact, never execute")
        self.stale = self.root / "target/debug/examples/docking_fixture"
        self.stale.parent.mkdir(parents=True)
        self.stale.write_bytes(b"stale sentinel, never execute")

    def artifact(self, **changes):
        message = {
            "reason": "compiler-artifact", "package_id": "path+file:///fixture#0.1.0",
            "manifest_path": str(self.manifest),
            "target": {"name": "docking_fixture", "kind": ["example"]},
            "profile": {"test": False}, "executable": str(self.executable),
            "fresh": False,
        }
        message.update(changes)
        return message

    def build(self, messages, code=0, *, kind="example", package="fixture", name="docking_fixture"):
        process = MagicMock()
        process.__enter__.return_value = process
        process.stdout = io.StringIO("".join(
            (json.dumps(message) if not isinstance(message, str) else message) + "\n"
            for message in messages))
        process.wait.return_value = code
        with patch("cargo_build.subprocess.Popen", return_value=process) as popen, \
                contextlib.redirect_stdout(io.StringIO()) as output, \
                contextlib.redirect_stderr(io.StringIO()) as diagnostics:
            result = build_executable(self.root, package=package,
                manifest=Path("fixture/Cargo.toml"), name=name, kind=kind,
                features=("docking-fixture",))
        return result, popen.call_args, output.getvalue(), diagnostics.getvalue()

    def finished(self, success=True):
        return {"reason": "build-finished", "success": success}

    def test_custom_directory_suffix_and_configured_target_use_only_reported_path(self):
        with patch.dict(os.environ, {"CARGO_TARGET_DIR": str(self.root / "custom env output")}):
            result, call, output, _ = self.build([self.artifact(), self.finished()])
        self.assertEqual(result, self.executable)
        self.assertNotEqual(result, self.stale)
        command = call.args[0]
        self.assertIn("--message-format=json", command)
        self.assertIn("--locked", command)
        self.assertIn("--example", command)
        self.assertEqual(command[command.index("--features") + 1], "docking-fixture")
        self.assertEqual(call.kwargs["cwd"], self.root)
        self.assertNotIn("env", call.kwargs, "Cargo must inherit target/config environment")
        self.assertNotIn("--target-dir", command)
        self.assertNotIn("--target", command)
        self.assertIn(str(self.executable), output)

    def test_bin_identity_and_fresh_artifacts_are_supported(self):
        artifact = self.artifact(target={"name": "xtask", "kind": ["bin"]}, fresh=True)
        result, call, _, _ = self.build([artifact, self.finished()],
                                      kind="bin", package="xtask", name="xtask")
        self.assertEqual(result, self.executable)
        self.assertIn("--bin", call.args[0])

    def test_foreign_package_host_tool_build_script_and_test_artifacts_are_not_selected(self):
        for change in [
            {"manifest_path": str(self.root / "dependency/Cargo.toml")},
            {"target": {"name": "another_target", "kind": ["example"]}},
            {"target": {"name": "docking_fixture", "kind": ["custom-build"]}},
            {"target": {"name": "docking_fixture", "kind": ["bin"]}},
            {"profile": {"test": True}},
            {"executable": None},
        ]:
            with self.subTest(change=change):
                wrong = self.artifact(**change)
                wrong["executable"] = str(self.stale) if wrong["executable"] else None
                result, _, _, _ = self.build([wrong, self.artifact(), self.finished()])
                self.assertEqual(result, self.executable)
                with self.assertRaisesRegex(RuntimeError, "0 executable artifacts"):
                    self.build([wrong, self.finished()])

    def test_missing_reported_artifact_never_falls_back_to_stale_default(self):
        with self.assertRaisesRegex(RuntimeError, "0 executable artifacts"):
            self.build([self.finished()])
        self.assertEqual(self.stale.read_bytes(), b"stale sentinel, never execute")

    def test_nonexistent_relative_or_directory_executable_is_rejected(self):
        for path in [self.root / "missing.exe", Path("relative.exe"), self.root]:
            with self.subTest(path=path):
                with self.assertRaisesRegex(RuntimeError, "existing absolute file"):
                    self.build([self.artifact(executable=str(path)), self.finished()])

    def test_ambiguous_cross_target_artifacts_fail_instead_of_last_wins(self):
        second = copy.deepcopy(self.artifact())
        second["executable"] = str(self.stale)
        with self.assertRaisesRegex(RuntimeError, "2 executable artifacts"):
            self.build([self.artifact(), second, self.finished()])

    def test_build_failure_cannot_publish_an_executable(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.build([self.artifact(), self.finished(False)], code=101)
        for endings in [[], [self.finished(False)], [self.finished(), self.finished()]]:
            with self.subTest(endings=endings):
                with self.assertRaisesRegex(RuntimeError, "successful build-finished"):
                    self.build([self.artifact()] + endings)

    def test_compiler_diagnostics_are_rendered_while_messages_are_processed(self):
        diagnostic = {"reason": "compiler-message", "message": {"rendered": "compiler detail 日本語\n"}}
        _, _, _, diagnostics = self.build([diagnostic, self.artifact(), self.finished()])
        self.assertEqual(diagnostics, "compiler detail 日本語\n")

    def test_invalid_json_or_protocol_never_selects_stale_files(self):
        for invalid in ["not JSON", {"reason": "compiler-artifact"}]:
            process = MagicMock()
            process.__enter__.return_value = process
            process.stdout = io.StringIO((invalid if isinstance(invalid, str)
                                         else json.dumps(invalid)) + "\n")
            with self.subTest(invalid=invalid), \
                    patch("cargo_build.subprocess.Popen", return_value=process), \
                    contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises((ValueError, KeyError)):
                    build_executable(self.root, package="fixture", manifest=self.manifest,
                                     name="docking_fixture", kind="example")
            process.kill.assert_called_once()
            process.wait.assert_called_once()


if __name__ == "__main__":
    unittest.main()
