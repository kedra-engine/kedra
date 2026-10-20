#!/usr/bin/env python3
"""Native Hierarchy visual contracts from Figma; requires an isolated open QA project.

Uses current real document nodes, not a fabricated runtime component taxonomy.
Checks layout/colors and saves frames at 320/232 logical pixels. It is not a
full pixel-equivalence claim for platform text rasterization or shared dock chrome.
"""
import argparse
import json
from pathlib import Path

from docking import Docking


def run(c, state_directory):
    assert Path(c.rpc("editor.state")["preferences"]).parent == state_directory.resolve()
    state = c.state()
    view = next(identity for identity, record in state["layout"]["views"].items() if record["kind"] == "hierarchy")
    prefix = f"hierarchy/ViewId({view})/"

    def node(key, visible=True):
        return c.node(prefix + key, visible=visible)

    def menu(caption):
        c.menu(view, caption)

    def close_search():
        if node("close-search"):
            c.click(prefix + "close-search")
        c.wait(lambda: not node("toolbar"), "default has no search toolbar")

    def resize(width):
        window = c.node(c.label(view))["window"]
        c.completed(c.operation("app.resize", window=window, width=width + 2, height=674, dpr=2))
        c.wait(lambda: abs(c.node(c.group_id(c.group(view)))["rect"][2] - c.node(c.group_id(c.group(view)))["rect"][0] - width) < 1, "reference panel width")
        return window

    def near(actual, expected):
        assert abs(actual - expected) <= 1, (actual, expected)

    def select_root(window):
        # Reveal can scroll a deeply selected node; Home reaches the root through
        # the same native key path used by an end user.
        menu("Reveal selected node")
        c.event(window=window, kind="key", key="home")
        return c.wait(lambda: node("row/0"), "root visible")

    def narrow_panel(width):
        # app.resize intentionally admits whole windows >=320px. Verify a narrower
        # docked panel through the actual divider, without weakening that API guard.
        c.menu(view, "Redock to main window", retired=True)
        c.click("workspace/reset")
        group = c.wait(lambda: c.node(c.group_id(c.group(view))), "Hierarchy group")
        right = group["rect"][2]
        dividers = [n for n in c.observe()["elements"] if n["visible"] and n["id"].endswith("/divider") and n["rect"][2] - n["rect"][0] <= 8]
        divider = min(dividers, key=lambda n: abs(n["rect"][0] - right))
        rect = divider["rect"]
        center = [(rect[0] + rect[2]) / 2, (rect[1] + rect[3]) / 2]
        window = c.window()
        c.event(window=window, kind="move", position=center)
        c.event(window=window, kind="button", pressed=True)
        target = [group["rect"][0] + width + (rect[2] - rect[0]) / 2, center[1]]
        c.event(window=window, kind="move", position=target)
        c.event(window=window, kind="button", pressed=False)
        c.wait(lambda: abs(c.node(c.group_id(c.group(view)))["rect"][2] - c.node(c.group_id(c.group(view)))["rect"][0] - width) <= 1, "narrow docked Hierarchy")
        return window

    close_search()
    assert not node("status"), "Persistent UUID footer is not in the default design"
    c.click(c.label(view))
    c.menu(view, "Float active tab")
    c.wait(lambda: c.node(c.label(view))["window"] != c.window(), "floating Hierarchy")
    window = resize(320)
    root = select_root(window)
    close_search()
    root = node("row/0")
    viewport = node("entries")["rect"]
    near(root["rect"][0], viewport[0] + 4)
    near(root["rect"][1], viewport[1] + 4)
    near(root["rect"][3] - root["rect"][1], 26)
    near(root["rect"][2], viewport[2] - 4)
    c.click(prefix + "node/0")
    c.event(window=window, kind="key", key="left")
    c.event(window=window, kind="key", key="right")
    c.wait(lambda: node("row/1"), "expanded branch")
    near(node("row/1")["rect"][0] - root["rect"][0], 12)
    near(node("row/1")["rect"][1] - root["rect"][1], 26)
    c.event(window=window, kind="key", key="down")
    c.wait(lambda: node("row/1")["background"] == [69, 102, 143, 255], "persistent selection color")
    selected = c.state()["documents"]
    c.event(window=window, kind="move", position=[310, 620])
    default_frame = c.frame(window)

    # Search is a 38px content toolbar, not a dock-header replacement.
    menu("Search nodes")
    c.wait(lambda: node("search"), "search field shown")
    toolbar = node("toolbar")["rect"]
    near(toolbar[3] - toolbar[1], 38)
    for key in ("search-icon", "filter", "close-search"):
        assert node(key), key
    assert not node("filter")["enabled"], "Unsupported filters must not appear functional"
    for key in ("filter", "close-search"):
        rect = node(key)["rect"]
        near(rect[2] - rect[0], 26)
        near(rect[3] - rect[1], 26)
    c.click(prefix + "search")
    c.event(window=window, kind="text", text="STRASSE")
    c.wait(lambda: node("search")["value"] == "STRASSE", "current-folder-style retained search")
    window = narrow_panel(232)
    c.click(prefix + "search")
    search_frame = c.frame(window)
    document_view = next(d["view"] for d in c.state()["documents"]["documents"] if d["document"] == c.state()["documents"]["active"])
    c.click(c.label(document_view))
    c.wait(lambda: node("row/1")["background"] == [58, 58, 58, 255], "inactive selection color")
    inactive = c.frame(window)
    # Moving focus to another tool does not change node identity.
    assert c.state()["documents"] == selected
    c.event(window=window, kind="key", key="escape")
    close_search()
    select_root(window)
    c.event(window=window, kind="key", key="left")
    c.wait(lambda: not node("row/1"), "collapsed root")
    collapsed_frame = c.frame(window)
    menu("Toggle source details")
    c.wait(lambda: node("status"), "optional source details remain available")
    menu("Toggle source details")
    c.wait(lambda: not node("status"), "source details can be dismissed")
    c.event(window=window, kind="key", key="right")
    c.click("workspace/reset")
    return {"frames": [default_frame, search_frame, inactive, collapsed_frame],
            "verified": ["default has no toolbar/footer", "26px rows", "12px indentation", "4px viewport inset", "selection color", "38px search", "26px flat actions", "232/320px hosts", "optional source details", "collapsed state"],
            "limitations": ["injected Bevy input", "dynamic source names/icons differ from the design specimen", "not complete Figma pixel equivalence", "no structural editing"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("credentials", type=Path)
    parser.add_argument("--state-directory", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    result = run(Docking(args.credentials), args.state_directory)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(f"PASS: native Hierarchy appearance contracts; report: {args.report}")


if __name__ == "__main__":
    main()
