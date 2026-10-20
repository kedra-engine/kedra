#!/usr/bin/env python3
"""Opt-in native QA client. Uses real Bevy input, geometry and renderer captures.

Not a CI prerequisite. Launch the editor with a new --remote-credentials file,
then pass that path here. No tokens are written to reports or command arguments.
Independent expectations live in docs/ui/startup-reference.json; never rewrite
them from observed UI. This does not automate native OS dialogs or certify IME.
"""
import argparse
import json
import sys
import tempfile
import time
import urllib.request
import uuid
from pathlib import Path


class Client:
    def __init__(self, credentials):
        self.endpoint = json.loads(credentials.read_text())
        self.session = self.rpc("app.describe")["session"]

    def rpc(self, method, params=None):
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
        request = urllib.request.Request(
            self.endpoint["url"], data=body.encode(),
            headers={"Authorization": "Bearer " + self.endpoint["token"],
                     "Content-Type": "application/json"})
        response = json.load(urllib.request.urlopen(request, timeout=8))
        if "error" in response:
            raise RuntimeError(f"{method}: {response['error']}")
        return response["result"]

    def wait(self, predicate, label, timeout=12):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            result = predicate()
            if result:
                return result
            time.sleep(.05)
        raise AssertionError(f"Timed out: {label}")

    def operation(self, method, **params):
        operation = str(uuid.uuid4())
        self.rpc(method, {"session": self.session, "id": operation, **params})
        return operation

    def completed(self, operation):
        def terminal():
            receipt = self.rpc("app.operation", {"id": operation})
            return receipt if receipt["state"] != "pending" else None
        receipt = self.wait(terminal, "operation completion")
        if receipt["state"] != "succeeded":
            raise AssertionError(receipt)
        return receipt["detail"]

    def window(self):
        return self.rpc("app.observe")["windows"][0]["id"]

    def input(self, **event):
        self.completed(self.operation("app.input", window=self.window(), input=event))

    def capture(self):
        result = self.completed(self.operation("app.capture", window=self.window()))
        assert not result["uniform"], f"Renderer returned a uniform frame: {result}"
        assert Path(result["path"]).is_file(), "Capture file was not published"
        return result


def geometry(client, reference, state):
    snapshot = client.rpc("app.observe")
    assert not snapshot["truncated"]
    elements = {node["id"]: node for node in snapshot["elements"] if node["visible"]}
    for name, expected in reference[state].items():
        assert name in elements, f"Missing visible element: {name}"
        actual = elements[name]["rect"]
        assert all(abs(a - b) <= reference["tolerance"] for a, b in zip(actual, expected)), (
            f"{state}/{name}: expected {expected}, observed {actual}")
    return snapshot


def lifecycle(client, isolated_state, capture_frames, reference):
    """Source-writing QA is permitted only with explicitly isolated preferences."""
    expected = isolated_state.resolve() / "recent-projects.toml"
    assert Path(client.rpc("editor.state")["preferences"]) == expected, (
        "Refusing lifecycle QA against personal preferences; launch with --state-directory")
    fixture_parent = expected.parent / "projects"
    fixture_parent.mkdir(parents=True, exist_ok=True)
    parent = Path(tempfile.mkdtemp(prefix="native-", dir=fixture_parent))
    project = parent / "Кедра QA"
    result = {"project": str(project), "steps": []}

    def projects():
        client.completed(client.operation("editor.project", command={"kind": "projects"}))
        client.wait(lambda: "projects" in client.rpc("app.observe")["screens"], "manager window")

    for command in [{"kind": "create", "parent": str(parent), "name": project.name},
                    {"kind": "open", "root": str(project)}]:
        old_window = client.window()
        started = time.monotonic()
        operation = client.operation("editor.project", command=command)
        step = {"command": command["kind"], "intermediate_loading_required": False}
        # Fast work intentionally has no loading window. Completion must not wait
        # for native creation/hydration of an unused intermediate surface.
        client.completed(operation)
        step["source_ready_seconds"] = time.monotonic() - started
        assert client.rpc("editor.state")["project"]["root"] == str(project)
        assert (project / "kedra.toml").is_file()
        assert (project / "assets").is_dir()
        client.wait(lambda: "editor" in client.rpc("app.observe")["screens"], "empty editor host")
        assert client.window() != old_window
        step["editor_ready_seconds"] = time.monotonic() - started
        if capture_frames:
            step["frame"] = client.capture()
        try:
            client.rpc("app.input", {"session": client.session, "id": str(uuid.uuid4()),
                                     "window": old_window, "input": {"kind": "move", "position": [10, 10]}})
        except RuntimeError as error:
            assert "Stale" in str(error)
        else:
            raise AssertionError("Retired native window accepted new input")
        result["steps"].append(step)
        projects()

    for command in [{"kind": "create", "parent": str(parent), "name": project.name},
                    {"kind": "open", "root": str(parent / "missing")}]:
        operation = client.operation("editor.project", command=command)
        receipt = client.wait(lambda: (receipt if receipt["state"] != "pending" else None)
                              if (receipt := client.rpc("app.operation", {"id": operation})) else None,
                              "expected rejected project request")
        assert receipt["state"] == "failed", receipt
        assert client.rpc("editor.state")["phase"] == "projects"
        result["steps"].append({"command": command["kind"], "expected_failure": receipt["detail"]})
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("credentials", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--geometry-only", action="store_true",
                        help="Run geometry/input only; report frames as unverified, never a visual pass")
    parser.add_argument("--lifecycle-state-directory", type=Path,
                        help="Also test source creation/opening; must match the editor's isolated --state-directory")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    reference = json.loads((root / "docs/ui/startup-reference.json").read_text())
    client = Client(args.credentials)
    report = {"reference": reference["source"], "runs": [], "status": "running"}
    try:
        assert client.rpc("editor.state")["phase"] == "projects", "Start from project manager"
        assert client.rpc("editor.state")["entry"] is None, "Close open forms before QA"
        for dpr in [1, 2]:
            print(f"Native startup QA: logical 1120×720, renderer DPR override {dpr}", flush=True)
            client.completed(client.operation("app.resize", window=client.window(),
                                             width=1120, height=720, dpr=dpr))
            run = {"dpr": dpr, "frames": {}}
            report["runs"].append(run)
            run["manager"] = geometry(client, reference, "manager")
            if not args.geometry_only:
                run["frames"]["manager"] = client.capture()
            client.input(kind="click", element="projects.new")
            client.wait(lambda: client.rpc("editor.state")["entry"], "create form")
            run["create"] = geometry(client, reference, "create")
            def focused():
                return next((node["id"] for node in client.rpc("app.observe")["elements"]
                             if node["focused"] and node["visible"]), None)
            client.wait(lambda: focused() == "entry.name", "modal autofocus")
            client.input(kind="key", key="tab")
            assert focused() == "entry.directory", "Tab must stay in the dialog"
            client.input(kind="key", key="shift_tab")
            assert focused() == "entry.name", "Shift+Tab must return to the name field"
            if not args.geometry_only:
                run["frames"]["create"] = client.capture()
            # Actual keyboard input, not semantic draft assignment.
            client.input(kind="click", element="entry.name")
            client.input(kind="key", key="select_all")
            client.input(kind="text", text="QA Кедра")
            assert client.rpc("editor.state")["entry"]["name"] == "QA Кедра"
            client.input(kind="key", key="select_all")
            client.input(kind="text", text="invalid/name")
            client.input(kind="key", key="enter")
            invalid = client.wait(lambda: client.rpc("editor.state")["entry"]["error"], "validation failure")
            assert client.rpc("editor.state")["entry"]["name"] == "invalid/name"
            run["validation"] = invalid
            if not args.geometry_only:
                run["frames"]["invalid"] = client.capture()
            client.input(kind="key", key="escape")
            client.wait(lambda: client.rpc("editor.state")["entry"] is None, "cancel form")
            client.input(kind="click", element="projects.open")
            client.wait(lambda: client.rpc("editor.state")["entry"], "open form")
            if not args.geometry_only:
                run["frames"]["open"] = client.capture()
            client.input(kind="click", element="entry.cancel")
        if args.lifecycle_state_directory:
            report["lifecycle"] = lifecycle(client, args.lifecycle_state_directory, not args.geometry_only, reference)
        report["status"] = "geometry_passed_frames_unverified" if args.geometry_only else "passed"
        print("PASS: geometry, pointer/keyboard and validation; frames UNVERIFIED" if args.geometry_only
              else "PASS: native geometry, pointer/keyboard, validation and renderer captures", flush=True)
    except Exception as error:
        report["status"] = "failed"
        report["error"] = str(error)
        raise
    finally:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(f"Report: {args.report}", flush=True)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"FAIL: {error}", file=sys.stderr)
        sys.exit(1)
