#!/usr/bin/env python3
"""Figma docking interaction-state regression, using real Bevy input and frames.

Requires an isolated editor run, just like docking.py. Does not mutate docking
through RPC or claim physical OS pointer/monitor certification.
"""
import argparse
import json
from pathlib import Path
from docking import Docking


def center(element):
    x0, y0, x1, y1 = element["rect"]
    return [(x0 + x1) / 2, (y0 + y1) / 2]


def compass_drop(c, ids, direction):
    """Native guide-drop scenario with retained panel content (drafts in fixtures)."""
    identity = f'panel/ViewId({ids["notes"]})/' + ("empty" if "preview" in ids else "draft")
    if "preview" not in ids:
        c.click(identity)
        # Fixture reset intentionally retains native drafts between compass cases.
        c.event(kind="key", key="select_all")
        c.event(kind="text", text="Retained docking state")
    field = c.node(identity)
    if "preview" not in ids:
        assert field["value"] == "Retained docking state", "Native draft text input was not delivered"
    target_group = c.group(ids["assets"])
    target = c.node(c.group_id(target_group))
    before = c.state()["layout"]
    window = c.start_drag(ids["notes"], center(target))
    guides = [c.wait(lambda d=d: c.node(f"dock.guide/{d}/guide"), d)
              for d in ("tab", "left", "right", "top", "bottom")]
    assert len(guides) == 5
    for guide in guides:
        r = guide["rect"]
        assert r[2]-r[0] == 28 and r[3]-r[1] == 28
    guide = c.node(f"dock.guide/{direction}/guide")
    c.event(window=window, kind="move", position=center(guide))
    preview = c.wait(lambda: c.node("dock.preview"), "guide result preview")
    assert c.state()["layout"] == before
    frame = c.frame(window)
    c.finish_drag(window)
    c.wait(lambda: c.state()["layout"] != before, "guide drop")
    group = c.group(ids["notes"])
    assert (group == target_group) == (direction == "tab")
    committed = c.wait(lambda: c.node(c.group_id(group)), "result group")
    assert all(abs(a-b) <= 1 for a,b in zip(preview["rect"], committed["rect"])), (direction, preview["rect"], committed["rect"])
    current = c.node(field["id"])
    assert current["entity"] == field["entity"]
    if "preview" not in ids:
        assert current["value"] == "Retained docking state"
    return {"kind": "compass-"+direction, "frame": frame,
            "preview": preview["rect"], "result": committed["rect"]}


def run(c, state_directory):
    preferences = Path(c.rpc("editor.state")["preferences"])
    assert preferences.resolve().parent == state_directory.resolve(), "Refusing personal preferences"
    c.wait(lambda: c.state().get("bound"), "workspace")
    report = {"input": "Bevy injection and native frames; not physical OS input", "steps": []}

    def reset(width=1440, height=1100, dpr=2):
        c.event(kind="key", key="escape")
        c.click("workspace/reset")
        c.wait(lambda: len(c.observe()["windows"]) == 1, "reset floating hosts")
        c.completed(c.operation("app.resize", window=c.window(), width=width, height=height, dpr=dpr))
        state = c.state()
        ids = {v["kind"]: k for k, v in state["layout"]["views"].items()}
        if "preview" in ids:
            ids.update(notes=ids["preview"], project=ids["hierarchy"], activity=ids["inspector"])
        return ids

    def cancel(window):
        c.event(window=window, kind="key", key="escape")
        c.finish_drag(window)
        assert not c.node("dock.drag/ghost")
        assert not c.node("dock.insertion")
        assert not c.node("dock.guide/tab/guide")

    for dpr in (1, 2):
        ids = reset(dpr=dpr)
        for identity in (f'dock/node/NodeId({c.group(ids["project"])})/menu',
                         f'dock/node/NodeId({c.group(ids["notes"])})/menu', "workspace/panels"):
            button = c.node(identity)
            c.click(identity)
            popup = c.wait(lambda: c.node("dock-menu"), "anchored popup")
            b, m = button["rect"], popup["rect"]
            assert abs(m[2] - b[2]) <= 1.1, (b, m)
            assert abs(m[1] - b[3] - 4) <= 1.1, (b, m)
            assert m[0] >= 3 and m[1] >= 3
            assert m[2] <= 1437 and m[3] <= 1097
            c.event(kind="key", key="escape")
        report["steps"].append({"kind": "anchor-multiple-groups-toolbar", "dpr": dpr})

        # Both directions, including release immediately after insertion appears.
        for source, target, side in (("notes", "activity", "right"), ("notes", "activity", "left")):
            tab = c.node(f'dock/tab/ViewId({ids[target]})')
            r = tab["rect"]
            x = r[2] - 1 if side == "right" else r[0] + 1
            before = c.state()["layout"]
            window = c.start_drag(ids[source], [x, (r[1]+r[3])/2])
            marker = c.wait(lambda: c.node("dock.insertion"), "tab insertion marker")
            assert marker["rect"][2]-marker["rect"][0] == 2
            assert marker["rect"][3]-marker["rect"][1] == 28
            assert c.state()["layout"] == before, "Reorder published during drag"
            assert not c.node("dock.preview"), "Tab insertion must not paint a destination panel"
            assert not c.node("dock.guide/tab/guide"), "Compass covers rail insertion"
            if side == "right":
                report["steps"].append({"kind": "insertion", "frame": c.frame(window)})
            c.finish_drag(window)
            order = c.state()["layout"]["nodes"][c.group(ids[source])]["Group"]["tabs"]
            expected = [ids[target], ids[source]] if side == "right" else [ids[source], ids[target]]
            assert order == expected, (order, expected)
        report["steps"].append({"kind": "reorder-both-directions", "dpr": dpr})

    # All four split targets, with candidate geometry equal to committed frames.
    for direction in ("left", "right", "top", "bottom", "tab"):
        ids = reset()
        report["steps"].append(compass_drop(c, ids, direction))

    # Left group is too narrow to split: no red/misleading destination preview.
    ids = reset(width=960, height=800)
    before = c.state()["layout"]
    window = c.start_drag(ids["notes"], center(c.node(c.group_id(c.group(ids["project"])))))
    guide = c.wait(lambda: c.node("dock.guide/left/guide"), "unavailable left guide")
    c.event(window=window, kind="move", position=center(guide))
    c.wait(lambda: c.node("dock.drag/reason"), "blocked reason")
    assert not c.node("dock.preview")
    report["steps"].append({"kind": "unavailable", "frame": c.frame(window)})
    c.finish_drag(window)
    assert c.state()["layout"] == before, "Unavailable guide committed a split"

    window = c.start_drag(ids["notes"], center(c.node(c.group_id(c.group(ids["assets"])))))
    c.wait(lambda: c.node("dock.preview"), "merge preview for cancel")
    cancel(window)
    assert c.state()["layout"] == before
    report["steps"].append({"kind": "escape-removes-all-transients"})

    # Small floating host: native Popover stays within the client bounds.
    ids = reset()
    c.menu(ids["notes"], "Float group")
    c.wait(lambda: len(c.observe()["windows"]) == 2, "float")
    group = c.group(ids["notes"])
    button_id = f"dock/node/NodeId({group})/menu"
    floating = c.node(button_id)["window"]
    c.completed(c.operation("app.resize", window=floating, width=320, height=320, dpr=2))
    c.click(button_id)
    menu = c.wait(lambda: c.node("dock-menu"), "small floating popup")
    assert menu["window"] == floating
    assert menu["rect"][0] >= 3 and menu["rect"][2] <= 317
    assert menu["rect"][1] >= 3 and menu["rect"][3] <= 317
    report["steps"].append({"kind": "small-floating-popup", "frame": c.frame(floating)})
    c.event(window=floating, kind="key", key="escape")
    reset()
    report["status"] = "passed"
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("credentials", type=Path)
    parser.add_argument("--state-directory", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    result = run(Docking(args.credentials), args.state_directory)
    args.report.write_text(json.dumps(result, ensure_ascii=False, indent=2))
    print("PASS: anchored popups, reorder, all five guide targets, unavailable drop and cancellation")


if __name__ == "__main__":
    main()
