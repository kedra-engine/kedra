#!/usr/bin/env python3
"""Opt-in unified-header native input/frame checks against isolated editor state.

Uses production docking input, observation and renderer readbacks. Does not claim
physical OS caption-button, native-drag or mixed-monitor acceptance.
"""
import argparse
import json
import math
from pathlib import Path
from docking import Docking


def validate_row(row, window):
    assert row and row["visible"] and row["window"] == window, "Missing native header"
    x0, y0, x1, y1 = row["rect"]
    assert all(math.isfinite(v) for v in (x0, y0, x1, y1)), "Nonfinite header"
    assert x1 > x0 and abs(y1-y0-32) <= 1, "Header must be one 32px logical row"


def center(node):
    x0, y0, x1, y1 = node["rect"]
    return [(x0+x1)/2, (y0+y1)/2]


def drag(c, view, point):
    window = c.start_drag(view, point)
    c.wait(lambda: c.node("dock.preview"), "measured docking preview")
    for _ in range(3):
        c.frame(window)
    c.finish_drag(window)


def check(c, state_directory):
    assert Path(c.rpc("editor.state")["preferences"]).resolve().parent == state_directory.resolve(), "Refusing personal preferences"
    state = c.wait(lambda: s if (s := c.state()).get("bound") else None, "editor workspace")
    report = {"input": "injected Bevy input; renderer frames", "steps": [],
              "limitations": ["Physical native buttons/drag, platform double-click, fullscreen and mixed monitors need separate acceptance."]}
    # Start the transition fixture from the documented four-tool preset. Earlier
    # runs may have persisted extra neighbours in a merged group; testing their
    # clipped label centers is not a valid two-tab drag origin.
    c.click("workspace/reset")
    state = c.wait(lambda: s if len((s := c.state())["layout"]["hosts"]) == 1
                   and len([n for n in s["layout"]["nodes"].values() if "Group" in n]) >= 4
                   and s["navigation_input"]["rendered"] == s["revision"] else None,
                   "isolated four-tool preset")
    report["steps"].append({"kind": "isolated-preset-reset"})
    main = c.window()
    for dpr in (1, 2):
        c.completed(c.operation("app.resize", window=main, width=1280, height=800, dpr=dpr))
        row = c.wait(lambda: c.node("window.chrome"), "main unified header")
        validate_row(row, main)
        canvas = c.node("editor.canvas")
        assert canvas and abs(canvas["rect"][1]-row["rect"][3]) <= 1, "Stacked title/toolbar rows"
        status, body = c.node("workspace/status"), c.node("workspace/host")
        assert status and body and status["rect"][1] >= body["rect"][3]-1, "Status bar must stay below the workspace"
        for key in ("editor.play", "editor.pause", "editor.stop", "editor.save", "editor.build", "editor.search"):
            control = c.node(key)
            assert control and not control["enabled"], "Unsupported command appears enabled: " + key
        before = c.state()["layout"]
        c.click("workspace/panels")
        c.wait(lambda: any(e["visible"] and e["id"].startswith("dock-menu/") for e in c.observe()["elements"]), "header menu input")
        c.event(kind="key", key="escape")
        assert c.state()["layout"] == before, "Header command started a docking gesture"
        report["steps"].append({"kind": "main-header", "dpr": dpr, "frame": c.frame(main)})

    ids = {record["kind"]: view for view, record in state["layout"]["views"].items()}
    a, b = ids["hierarchy"], ids["assets"]
    drag(c, a, center(c.node(c.group_id(c.group(b)))))
    c.wait(lambda: c.group(a) == c.group(b), "merged tool group")
    labels = {view: c.node(c.label(view))["entity"] for view in (a, b)}
    c.menu(a, "Float group")
    floating = c.wait(lambda: (node["window"] if node and node["window"] != main else None)
                      if (node := c.node(c.label(a))) else None, "floating group")
    row = c.wait(lambda: c.node("floating-caption/window.chrome"), "floating header")
    validate_row(row, floating)
    for view in (a, b):
        label = c.node(c.label(view))
        assert label["entity"] == labels[view]
        assert label["rect"][1] >= row["rect"][1] and label["rect"][3] <= row["rect"][3]+1, "Floating rail was not integrated"
    report["steps"].append({"kind": "integrated-float", "frame": c.frame(floating)})
    group = c.node(c.group_id(c.group(a)))
    point = [group["rect"][0]+12, center(group)[1]]
    before = c.state()["layout"]
    window = c.start_drag(a, point)
    c.wait(lambda: c.node("dock.preview"), "split candidate")
    c.event(window=window, kind="key", key="escape")
    c.finish_drag(window)
    assert c.state()["layout"] == before, "Cancelled split published header policy"
    drag(c, a, point)
    c.wait(lambda: c.group(a) != c.group(b), "floating split")
    for view in (a, b):
        label = c.node(c.label(view))
        assert label["entity"] == labels[view]
        assert label["rect"][1] >= row["rect"][3]-1, "Split host still embeds a group's rail"
        group = c.group(view)
        rail = c.node(f"dock/node/NodeId({group})/tabs")
        first = c.state()["layout"]["nodes"][group]["Group"]["tabs"][0]
        tab = c.node(f"dock/tab/ViewId({first})")
        assert abs(tab["rect"][0]-rail["rect"][0]) <= 1, "Returned rail retained integrated-slot padding"
    report["steps"].append({"kind": "standalone-split-float", "frame": c.frame(floating)})
    drag(c, a, center(c.node(c.group_id(c.group(b)))))
    c.wait(lambda: c.group(a) == c.group(b), "floating merge")
    for view in (a, b):
        label = c.node(c.label(view))
        assert label["entity"] == labels[view] and label["rect"][3] <= row["rect"][3]+1
    report["steps"].append({"kind": "reintegrated-float", "frame": c.frame(floating)})
    c.menu(a, "Redock to main window", retired=True)
    c.wait(lambda: len(c.observe()["windows"]) == 1, "floating retirement after redock")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("credentials", type=Path)
    parser.add_argument("--state-directory", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    print("Checking unified native headers and retained docking transitions…", flush=True)
    report = {"status": "incomplete"}
    try:
        report.update(check(Docking(args.credentials), args.state_directory))
        report["status"] = "passed"
    except Exception as error:
        report.update(status="failed", error=type(error).__name__ + ": " + str(error))
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2))
    print("Window chrome:", report["status"], "—", args.report, flush=True)
    if report.get("error"):
        print(report["error"], flush=True)
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
