#!/usr/bin/env python3
"""Isolated production-dock native tests. References are ONLY pinned Figma exports.

One command builds a NEW delivery, starts no editor startup/tools, injects input,
captures stable frames twice, then runs the existing xtask visual comparator.
No bless/update/record-baseline option exists. A missing scenario or visual
mismatch is a failure, never silently accepted as a new reference.
"""
import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import time
import traceback
from pathlib import Path
from urllib.error import HTTPError, URLError
from docking import Docking
from cargo_build import build_executable

ROOT = Path(__file__).resolve().parents[2]
REFERENCES = ROOT / "docs/ui/docking/figma"
POLICY = ROOT / "docs/ui/docking/policy.json"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_references():
    manifest = json.loads((REFERENCES / "manifest.json").read_text())
    assert manifest["schema"] == 1
    frames = manifest["frames"]
    assert len({f["name"] for f in frames}) == len(frames)
    for frame in frames:
        path = REFERENCES / (frame["name"] + ".png")
        assert digest(path) == frame["sha256"], "Figma reference changed: " + frame["name"]
    return {f["name"]: f for f in frames}


class Fixture(Docking):
    def state(self):
        return self.rpc("dock.state")

    def fixture(self):
        return self.rpc("dock.fixture")

    def reset(self, size=(1200, 800)):
        self.event(kind="modifier",modifier="shift",pressed=False)
        self.event(kind="button", pressed=False)
        self.event(kind="key", key="escape")
        self.completed(self.operation("app.resize", window=self.window(), width=size[0], height=size[1], dpr=1))
        generation = self.fixture()["generation"]
        self.event(kind="key", key="f6")
        self.wait(lambda: self.fixture()["generation"] > generation, "fixture setup reset")
        self.wait(lambda: len(self.observe()["windows"]) == 1, "retired fixture floats")
        self.event(kind="move", position=[1, 1])
        self.wait(lambda:len([e for e in self.observe()["elements"] if e["visible"] and e["id"].endswith("/divider") and e["rect"][2]>e["rect"][0] and e["rect"][3]>e["rect"][1]])==2,"fixture baseline layout and divider restoration")
        self.stable(self.window())

    def stable(self, window, attempts=16):
        """Three identical native PNGs after receipts, bounded, no arbitrary sleep."""
        self.wait(lambda:self.fixture()["feedback_ready"],"delivered native glyph variants")
        previous = None
        count = 0
        for _ in range(attempts):
            frame = self.frame(window)
            sha = digest(ROOT / frame["path"])
            count = count + 1 if sha == previous else 1
            previous = sha
            if count >= 3:
                return frame, sha
        raise AssertionError("Renderer never settled to three identical captures")

    def point(self, view):
        r = self.node(self.group_id(self.group(view)))["rect"]
        return [(r[0]+r[2])/2, (r[1]+r[3])/2]

    def settle_preview(self, attempts=16):
        """Fence revocable measurement readiness with renderer readbacks, not a glimpse."""
        self.wait(lambda: self.fixture()["feedback_ready"], "delivered native glyph variants")
        previous, count = None, 0
        observations = []

        def identity(node):
            if node is None:
                return None
            return node["entity"], node["window"], tuple(node["rect"])

        for _ in range(attempts):
            before = self.node("dock.preview")
            frame = self.frame(before["window"] if before else self.window())
            sha = digest(ROOT / frame["path"])
            after = self.node("dock.preview")
            start, end = identity(before), identity(after)
            observations.append({"frame": frame["frame"], "before": start, "after": end, "sha256": sha})
            if start is not None and start == end:
                current = (end, sha)
                count = count + 1 if current == previous else 1
                previous = current
                if count >= 3:
                    return after
            else:
                previous, count = None, 0
        raise AssertionError(f"Preview never settled across {attempts} native readbacks; "
                             f"last observations: {observations[-4:]!r}; fixture: {self.fixture()!r}")

    def cancel(self, window):
        self.event(window=window, kind="key", key="escape")
        self.finish_drag(window)
        assert not self.node("dock.drag/ghost")
        assert not self.node("dock.preview")
        assert not self.node("dock.insertion")

    def resize_split(self, split, point):
        r = split["rect"]
        self.event(kind="move", position=[(r[0]+r[2])/2, (r[1]+r[3])/2])
        self.event(kind="button", pressed=True)
        self.event(kind="move", position=point)
        self.event(kind="button", pressed=False)
        self.event(kind="move", position=[1, 1])


def scenarios(c, save):
    a, b, child, d = c.fixture()["views"]

    def capture(name, window=None):
        save(name, window or c.window())

    def compact():
        c.reset((960, 640))
        dividers = [e for e in c.observe()["elements"] if e["visible"] and e["id"].endswith("/divider")]
        vertical = next(e for e in dividers if e["rect"][2]-e["rect"][0] < 8)
        c.resize_split(vertical, [303, 400])
        horizontal = next(e for e in c.observe()["elements"] if e["visible"] and e["id"].endswith("/divider") and e["rect"][3]-e["rect"][1] < 8)
        c.resize_split(horizontal, [600, 341])

    c.reset()
    capture("i00-baseline")
    compact()
    capture("i01-compact")
    c.reset()
    before = c.state()["layout"]
    window = c.start_drag(child, [784, 270])
    c.settle_preview()
    assert c.state()["layout"] == before
    capture("i02-compass")
    c.event(kind="move", position=[1031, 270])
    c.settle_preview()
    capture("i03-direct")
    c.finish_drag(window)
    c.wait(lambda: c.group(child) != c.group(b), "split result")
    c.event(kind="move", position=[1, 1])
    capture("i04-split-result")

    c.reset()
    window = c.start_drag(child, [151, 420])
    c.settle_preview()
    capture("i05-merge-preview")
    c.finish_drag(window)
    c.wait(lambda: c.group(child) == c.group(a), "merge result")
    c.event(kind="move", position=[1, 1])
    capture("i06-merged")

    c.reset()
    before = c.state()["layout"]
    window = c.start_drag(child, [519, 525])
    c.wait(lambda: c.node("dock.insertion"), "tab insertion")
    assert c.state()["layout"] == before
    capture("i07-insertion")
    c.finish_drag(window)
    c.event(kind="move", position=[1, 1])
    tabs = c.state()["layout"]["nodes"][c.group(child)]["Group"]["tabs"]
    assert tabs == [d, child], {"expected": [d, child], "actual": tabs,
                               "state": c.state(), "fixture": c.fixture(), "ui": c.observe()}
    capture("i08-reordered")

    c.reset()
    before = c.state()["layout"]
    window = c.start_drag(child, [183, 420])
    c.wait(lambda: c.node("dock.drag/reason"), "blocked split")
    assert not c.node("dock.preview")
    capture("i09-blocked")
    c.finish_drag(window)
    assert c.state()["layout"] == before
    c.reset()
    c.event(kind="move", position=[303, 420])
    capture("i10-splitter-hover")
    divider = next(e for e in c.observe()["elements"] if e["visible"] and e["id"].endswith("/divider") and e["rect"][2]-e["rect"][0] < 8)
    c.resize_split(divider, [343, 420])
    capture("i11-resized")

    c.reset()
    baseline=c.state()["layout"]
    c.menu(b,"Maximize group")
    c.event(kind="move", position=[1, 1])
    capture("i12-maximized")
    assert any(host["maximized"]==c.group(b) for host in c.state()["layout"]["hosts"].values())
    c.menu(b,"Restore group")
    assert c.state()["layout"]["nodes"]==baseline["nodes"]
    assert all(host["maximized"] is None for host in c.state()["layout"]["hosts"].values())

    c.reset()
    c.event(kind="key",key="f8")
    overflow=c.wait(lambda:c.node(f"dock/node/NodeId({c.group(b)})/menu"),"common overflow menu action")
    capture("i13-overflow")
    assert not any(e["visible"] and e["id"].startswith("dock/node/") and e["id"].endswith(("/maximize","/overflow")) for e in c.observe()["elements"]),"Only the common flat menu belongs in a dock header"
    titles=[e for e in c.observe()["elements"] if e["visible"] and e["id"].endswith("/title") and e.get("text","").endswith("…")]
    assert titles,"Long native tab titles must show a shaped ellipsis, not silent clipping"
    c.click(overflow["id"])
    c.wait(lambda:c.node("dock-menu"),"overflow full-title popup")
    assert any(e.get("text")=="Maximize group" and e["visible"] for e in c.observe()["elements"])
    assert any(e.get("text")=="Panel with a very long title that does not fit its tab" and e["visible"] for e in c.observe()["elements"])
    c.event(kind="key",key="escape")

    # Float only C through the same pointer gesture used in the product. No
    # semantic docking RPC or test-only float implementation is involved.
    c.reset()
    c.menu(child,"Float active tab")
    c.wait(lambda: len(c.observe()["windows"]) == 2, "native float")
    c.wait(lambda:not c.fixture()["pending"] and c.node(c.label(child))["window"]!=c.window(),"acknowledged content transfer")
    floating = c.node(c.label(child))["window"]
    c.completed(c.operation("app.resize", window=floating, width=640, height=480, dpr=1))
    c.event(window=floating, kind="move", position=[1, 1])
    capture("i14-float-main")
    capture("i14-float-window", floating)
    window = c.start_drag(child, [100, 100])
    # Destination events supply coordinates in the main client (portable path).
    c.event(window=c.window(), kind="move", position=[415, 525])
    c.wait(lambda: c.node("dock.insertion"), "cross-host redock insertion")
    capture("i15-redock")
    c.event(window=c.window(),kind="button",pressed=False)
    c.wait(lambda: len(c.observe()["windows"]) == 1, "redock retires float")
    c.reset()
    capture("i16-restored")
    compact()
    before = c.state()["layout"]
    window = c.start_drag(child, [183, 340])
    c.wait(lambda: c.node("dock.drag/reason"), "compact blocked split")
    capture("i17-compact-blocked")
    c.cancel(window)
    assert c.state()["layout"] == before
    c.reset()
    capture("i18-macos")

    # Extra mechanics assertions without masking/changing any Figma capture.
    c.reset((1440, 1100))
    c.menu(child, "Maximize group")
    c.click(c.label(d))
    before = c.state()["layout"]
    group = c.node(c.group_id(c.group(d)))["rect"]
    window = c.start_drag(d, [group[0]+8, (group[1]+group[3])/2])
    # A hidden candidate must not become admissible merely by waiting frames.
    # Renderer readbacks fence well beyond the old three-update admission gate.
    c.stable(window)
    preview = c.node("dock.preview")
    c.finish_drag(window)
    if preview:
        actual = c.node(c.group_id(c.group(d)))["rect"]
        assert all(abs(a-b) <= 1 for a,b in zip(preview["rect"], actual)), "Maximized drop committed outside its measured visible preview"
    else:
        assert c.state()["layout"] == before, "Unmeasured maximized drop published an invisible sibling"

    for direction in ("left", "right", "top", "bottom", "tab"):
        c.reset((1440, 1100))
        window = c.start_drag(child, c.point(b))
        guide = c.wait(lambda: c.node(f"dock.guide/{direction}/guide"), direction)
        r = guide["rect"]
        c.event(kind="move", position=[(r[0]+r[2])/2, (r[1]+r[3])/2])
        preview = c.settle_preview()["rect"]
        before = c.state()["layout"]
        assert c.fixture()["dragging"]
        c.finish_drag(window)
        c.wait(lambda: c.state()["layout"] != before, "guide commit")
        assert (c.group(child) == c.group(b)) == (direction == "tab")
        actual = c.node(c.group_id(c.group(child)))["rect"]
        assert all(abs(a-b) <= 1 for a,b in zip(preview, actual)), (direction, preview, actual)
    draft_retention(c, child, a)
    c.reset()
    group=c.group(child)
    before=c.state()["layout"]["nodes"][group]["Group"]
    c.event(kind="modifier",modifier="shift",pressed=True)
    window=c.start_drag(child,c.point(a))
    c.settle_preview()
    c.finish_drag(window)
    c.event(kind="modifier",modifier="shift",pressed=False)
    assert c.group(child)==c.group(d)==c.group(a)
    order=c.state()["layout"]["nodes"][c.group(a)]["Group"]["tabs"]
    assert order[-2:]==before["tabs"],"Whole-group drag lost source tab order"
    c.reset()

    # Menus are checked against invoking buttons, not host origin or screenshot
    # tolerances. The fixture uses the actual product Popover/close adapters.
    for dpr in (1,2):
        c.completed(c.operation("app.resize",window=c.window(),width=1200,height=800,dpr=dpr))
        for view in (a,child):
            button=c.node(f"dock/node/NodeId({c.group(view)})/menu")
            c.event(kind="move",position=[1,1])
            c.wait(lambda:c.node(button["id"])["background"][3]==0,"transparent flat idle state")
            r=button["rect"]
            c.event(kind="move",position=[(r[0]+r[2])/2,(r[1]+r[3])/2])
            c.wait(lambda:c.node(button["id"])["background"][:3]==[61,61,61],"flat hover feedback")
            c.event(kind="button",pressed=True)
            c.wait(lambda:c.node(button["id"])["background"][:3]==[41,41,41],"flat press feedback")
            assert not c.fixture()["dragging"],"Menu press must not initiate tab drag"
            c.event(kind="button",pressed=False)
            popup=c.wait(lambda:c.node("dock-menu"),"anchored menu")
            x0,y0,x1,y1=popup["rect"]
            assert popup["window"]==button["window"]
            assert abs(x1-button["rect"][2])<=1.1
            assert abs(y0-button["rect"][3]-4)<=1.1
            assert 3<=x0<x1<=1197 and 3<=y0<y1<=797
            c.event(kind="key",key="escape")
    c.reset()
    c.menu(child,"Float active tab")
    c.wait(lambda:not c.fixture()["pending"] and c.node(c.label(child))["window"]!=c.window(),"floating close setup")
    c.click("floating-caption/window.close",retired=True)
    c.wait(lambda:len(c.observe()["windows"])==1,"floating close retires native host")
    assert child in c.state()["layout"]["hidden"]
    c.reset()

    # A floating native field must traverse only its own visible window controls,
    # not the primary window or the inactive tabs' hidden fields.
    c.event(kind="key", key="f7")
    c.menu(child, "Float active tab")
    def floating_field():
        field = c.node("fixture.C/draft")
        return field if not c.fixture()["pending"] and field and field["window"] != c.window() else None
    c.wait(floating_field, "floating keyboard setup")
    c.click("fixture.C/draft")
    field = c.node("fixture.C/draft")
    floating = field["window"]
    assert field["focused"]
    c.event(window=floating, kind="key", key="tab")
    focused = c.wait(lambda: next((e for e in c.observe()["elements"] if e["focused"] and e["entity"] != field["entity"]), None), "floating Tab traversal")
    assert focused["window"] == floating and focused["visible"] and focused["enabled"], "Tab escaped the floating native window"
    c.event(window=floating, kind="key", key="shift_tab")
    c.wait(lambda: (field := c.node("fixture.C/draft")) and field["focused"], "floating reverse Tab traversal")
    c.frame(floating)
    c.reset()


def draft_retention(c, child, destination):
    """Native-input scenario: require delivered text before testing its retention."""
    c.reset()
    c.event(kind="key", key="f7")
    c.click("fixture.C/draft")
    c.event(kind="text", text="Retained native draft")
    field = c.node("fixture.C/draft")
    assert field["value"] == "Retained native draft", "Native draft text input was not delivered"
    window = c.start_drag(child, c.point(destination))
    c.settle_preview()
    c.finish_drag(window)
    current = c.node("fixture.C/draft")
    assert current["entity"] == field["entity"] and current["value"] == "Retained native draft"


def cleanup(process, log, client=None):
    """Best-effort RPC shutdown, then bounded wait/terminate/kill; never mask a failure."""
    errors = []
    exit_input_error = None
    unforced_success = False
    try:
        if client is not None:
            try:
                client.event(kind="button", pressed=False)
            except Exception as error:
                errors.append("Shutdown button release: " + str(error))
            else:
                try:
                    client.event(kind="key", key="f12")
                except Exception as error:
                    # F12 can close the server before the input receipt is polled.
                    # Classify only after the OS confirms a normal, unforced exit.
                    exit_input_error = error
        if process is not None:
            reaped = False
            for signal in (None, process.terminate, process.kill):
                if signal is not None:
                    errors.append("Fixture shutdown required forced termination")
                    try:
                        signal()
                    except Exception as error:
                        errors.append("Shutdown signal: " + str(error))
                try:
                    code = process.wait(timeout=10)
                    reaped = True
                    unforced_success = signal is None and code == 0
                    if code != 0:
                        errors.append(f"Fixture exited with status {code}")
                    break
                except subprocess.TimeoutExpired:
                    continue
                except Exception as error:
                    errors.append("Shutdown wait: " + str(error))
            if not reaped:
                errors.append("Fixture could not be reaped after bounded wait/terminate/kill")
        if exit_input_error is not None:
            reason = (exit_input_error.reason if isinstance(exit_input_error, URLError)
                      and not isinstance(exit_input_error, HTTPError) else exit_input_error)
            if not (unforced_success and isinstance(reason, ConnectionError)):
                errors.append("Shutdown F12 input: " + str(exit_input_error))
    finally:
        try:
            log.close()
        except Exception as error:
            errors.append("Closing native log: " + str(error))
    return errors


def launch(binary, delivery, directory):
    directory.mkdir()
    credentials = directory / "session.json"
    env = dict(os.environ, KEDRA_FIXTURE_ASSETS=str(delivery), KEDRA_QA_CREDENTIALS=str(credentials),
               KEDRA_QA_ARTIFACTS=str(directory / "frames"))
    log = (directory / "native.log").open("w")
    process = None
    try:
        process = subprocess.Popen([str(binary)], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
        deadline = time.monotonic()+40
        while not credentials.exists():
            if process.poll() is not None or time.monotonic() > deadline:
                raise AssertionError("Fixture failed to start; see " + str(directory / "native.log"))
            time.sleep(0.05)
        client=Fixture(credentials)
    except BaseException:
        for error in cleanup(process, log):
            print("Fixture cleanup: " + error, file=sys.stderr)
        raise
    print("Native diagnostics:",directory/"native.log",flush=True)
    return process, log, client


def compare(name, entry, frame, sha, client, directory, *, executable, visual_policy="strict",
            capture_profile=None):
    policy = json.loads(POLICY.read_text())
    description = client.fixture()
    renderer = description["renderer"]
    profile = {"id": "dock-native-dpr1-reduced-v1", "platform": sys.platform,
               "renderer": renderer["backend"] + "/" + renderer["name"],
               "environment": platform.platform(), "logical_size": entry["size"],
                "dpr": 1, "color_space": "srgb_assumed", "motion": "reduced"}
    if capture_profile is not None:
        profile = capture_profile
    config = {"schema": 1, "scenario": name, "fixture": policy["fixture"], "profile": profile,
              "reference": {"image": name+".reference.png", "sha256": entry["sha256"],
                            "source": "https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/UI-KIT?node-id="+entry["node"].replace(":", "-"),
                            "kind": "design_reference", "approval": policy["reference_approval"],
                            "export_size": entry["size"], "crop": [0, 0] + entry["size"],
                            "crop_reason": "Complete independent Figma frame, no crop, resampling or mask."},
              "animation": policy["animation"], "comparison": policy["comparison"], "exclusions": [],
              "critical_regions": [{"name": name+"/"+str(i), "rect": rect,
                                    "budget": {"max_changed_pixels": int(rect[2]*rect[3]*0.02), "max_changed_ppm": 20000}}
                                   for i, rect in enumerate(policy["critical_regions"][name])]}
    if visual_policy == "accepted":
        from dock_visual_policy import accepted, validate_text
        revision = json.loads((REFERENCES/"manifest.json").read_text())["revision"]
        config = accepted(config, revision)
        validate_text(name, client.observe()["elements"])
    elif visual_policy != "strict":
        raise ValueError("Unknown visual policy")
    evidence = {"schema": 1, "scenario": name, "fixture": policy["fixture"], "profile": profile,
                "image": name+".actual.png", "sha256": sha,
                "producer": "isolated production dock fixture / Bevy renderer readback",
                "readiness": "Three consecutive identical PNGs after input receipts; no live text cursor in design captures",
                "receipt_frame": frame["frame"]}
    config_path = directory / (name+".config.json")
    shutil.copyfile(REFERENCES/(name+".png"),directory/(name+".reference.png"))
    evidence_path = directory / (name+".evidence.json")
    config_path.write_text(json.dumps(config, indent=2))
    evidence_path.write_text(json.dumps(evidence, indent=2))
    result = subprocess.run([str(executable), "visual", "--config", str(config_path),
                             "--actual", str(evidence_path), "--output", str(directory/(name+".diff")), "--non-interactive"],
                            cwd=ROOT, capture_output=True, text=True)
    (directory/(name+".visual.log")).write_text(result.stdout+result.stderr)
    return result.returncode == 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path, help="NEW artifact directory")
    parser.add_argument("--delivery", type=Path, help="Reuse an explicitly verified delivery")
    parser.add_argument("--repeats", type=int, default=2, help="Independent process repetitions, at least two")
    parser.add_argument("--visual-policy", choices=("strict", "accepted"), default="accepted",
                        help="Owner-reviewed acceptance (default), or unchanged strict audit")
    args = parser.parse_args()
    if args.repeats < 2:
        parser.error("At least two independent processes are required for stability verification")
    references = verify_references()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {"schema": 1, "fixture": "docking.fixture.v1", "reference": "pinned Figma exports ONLY",
              "reference_revision":json.loads((REFERENCES/"manifest.json").read_text())["revision"],
               "runs": [], "status": "incomplete", "visual_policy": args.visual_policy}
    try:
        print("Building the opt-in dock-only host…", flush=True)
        fixture_binary = build_executable(ROOT, package="kedra_editor",
            manifest=Path("crates/kedra_editor/Cargo.toml"), name="docking_fixture",
            kind="example", features=("docking-fixture",))
        comparator_binary = build_executable(ROOT, package="xtask",
            manifest=Path("xtask/Cargo.toml"), name="xtask", kind="bin")
        delivery = args.delivery.resolve() if args.delivery else output / "delivery"
        if not args.delivery:
            subprocess.run(["cargo", "run", "-p", "kedra", "--locked", "--", "assets", "build", "assets/editor", str(delivery), "--namespace", "editor"], cwd=ROOT, check=True)
        previous = None
        for repeat in range(args.repeats):
            directory = output / ("run-"+str(repeat+1))
            process, log, client = launch(fixture_binary, delivery, directory)
            captures = {}
            report["runs"].append(captures)
            try:
                client.wait(lambda: client.fixture().get("ready"), "dock fixture ready")
                assert not client.fixture()["project_tools"] and not client.fixture()["preferences"]

                def save(name, window):
                    frame, sha = client.stable(window)
                    entry = references[name]
                    assert [frame["width"], frame["height"]] == entry["size"], (name, frame, entry)
                    shutil.copyfile(ROOT/frame["path"], directory/(name+".actual.png"))
                    (directory/(name+".observe.json")).write_text(json.dumps({"ui":client.observe(),"layout":client.state(),"fixture":client.fixture()},indent=2))
                    visual = compare(name, entry, frame, sha, client, directory,
                                     executable=comparator_binary, visual_policy=args.visual_policy)
                    captures[name] = {"sha256": sha, "visual_passed": visual}
                    print(f"run {repeat+1}: {name}: stable capture; Figma {args.visual_policy} {'PASS' if visual else 'FAIL/review required'}", flush=True)

                scenarios(client, save)
                if previous is not None:
                    unstable = [name for name in captures.keys() & previous.keys()
                                if captures[name]["sha256"] != previous[name]["sha256"]]
                    assert not unstable, "Independent process captures changed: " + ", ".join(unstable)
                previous = captures
            finally:
                errors = cleanup(process, log, client)
                if errors:
                    report.setdefault("cleanup_errors", []).extend(
                        f"run {repeat+1}: {error}" for error in errors)
        report["not_captured_by_run"] = [sorted(set(references)-set(run)) for run in report["runs"]]
        report["not_captured"] = sorted({name for missing in report["not_captured_by_run"] for name in missing})
        report["stability"] = "incomplete" if report["not_captured"] else "passed"
        report["mechanics"]="passed (injected Bevy input, not physical OS input)"
        passed = (not report["not_captured"] and not report.get("cleanup_errors")
                  and all(c["visual_passed"] for run in report["runs"] for c in run.values()))
        report["status"] = "passed" if passed else "failed"
    except Exception as error:
        report["error"] = type(error).__name__+": "+str(error)
        report["traceback"]=traceback.format_exc()
        report["status"] = "failed"
    finally:
        (output/"report.json").write_text(json.dumps(report, indent=2))
    print("Dock fixture:", report["status"], "—", output/"report.json", flush=True)
    if report.get("error"):
        print(report["error"], file=sys.stderr)
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
