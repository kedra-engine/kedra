#!/usr/bin/env python3
"""Opt-in native docking QA using Bevy input and renderer frames, not OS capture.

Run only against --state-directory isolation. No semantic docking mutation or
project source write is used. This does not certify physical mixed-DPI dragging.
"""
import argparse
import json
from pathlib import Path
from startup import Client


class Docking(Client):
    def window(self):
        return next(w["id"] for w in self.observe()["windows"] if w["primary"])

    def observe(self):
        return self.rpc("app.observe")

    def state(self):
        return self.rpc("editor.workspace")

    def node(self, identity, visible=True):
        return next((e for e in self.observe()["elements"]
                     if e["id"] == identity and (not visible or e["visible"])), None)

    def event(self, window=None, **event):
        return self.completed(self.operation("app.input", window=window or self.window(), input=event))

    def click(self, identity, retired=False):
        element = self.wait(lambda: self.node(identity), "visible " + identity)
        try:
            self.event(window=element["window"], kind="click", element=identity)
        except AssertionError:
            if not retired or any(w["id"] == element["window"] for w in self.observe()["windows"]):
                raise

    def label(self, view):
        return f"dock/view/ViewId({view})/label"

    def group(self, view):
        return next(group for group, node in self.state()["layout"]["nodes"].items()
                    if view in node.get("Group", {}).get("tabs", []))

    def group_id(self, group):
        return f"dock/group/NodeId({group})"

    def menu(self, view, caption, retired=False):
        group = self.group(view)
        self.click(f"dock/node/NodeId({group})/menu")
        item = self.wait(lambda: next((e for e in self.observe()["elements"]
                                     if e["id"].startswith("dock-menu/")
                                     and e.get("text") == caption and e["visible"]), None), caption)
        self.click(item["id"].removesuffix("/caption"), retired=retired)

    def frame(self, window):
        result = self.completed(self.operation("app.capture", window=window))
        assert not result["uniform"] and Path(result["path"]).is_file()
        return result

    def start_drag(self, view, position):
        label = self.node(self.label(view))
        rect = label["rect"]
        self.event(window=label["window"], kind="move", position=[(rect[0]+rect[2])/2, (rect[1]+rect[3])/2])
        self.event(window=label["window"], kind="button", pressed=True)
        # First move crosses the drag threshold; second supplies its target.
        self.event(window=label["window"], kind="move", position=position)
        self.event(window=label["window"], kind="move", position=position)
        return label["window"]

    def finish_drag(self, window):
        self.event(window=window, kind="button", pressed=False)


def check(client, state_directory):
    preferences = Path(client.rpc("editor.state")["preferences"])
    assert preferences.resolve().parent == state_directory.resolve(), "Refusing personal preferences"
    state = client.wait(lambda: client.state() if client.state().get("bound") else None, "workspace")
    ids = {record["kind"]: identity for identity, record in state["layout"]["views"].items()}
    notes = ids.get("preview", ids.get("notes"))
    report = {"input_kind": "injected Bevy pointer/keyboard, not physical OS input", "steps": []}
    client.click(client.label(notes))
    draft_id = f"panel/ViewId({notes})/" + ("empty" if "preview" in ids else "draft")
    field = client.node(draft_id)
    assert field
    text = "Кедра 日本語 — retained native draft"
    if "preview" not in ids:
        client.click(draft_id)
        client.event(kind="text", text=text)

    def retained():
        current = client.node(draft_id, visible=False)
        assert current["entity"] == field["entity"], "Panel content was reconstructed"
        if "preview" not in ids:
            assert current["value"] == text, current

    for dpr in (1, 2):
        client.completed(client.operation("app.resize", window=client.window(), width=1200, height=800, dpr=dpr))
        client.wait(lambda: client.node(client.label(notes)), "DPR layout")
        group = client.group(notes)
        rail = client.node(f"dock/node/NodeId({group})/tabs")["rect"]
        tab = client.node(f"dock/tab/ViewId({notes})")["rect"]
        assert abs(rail[3]-rail[1]-32) < 1
        assert abs(tab[3]-tab[1]-28) < 1
        report["steps"].append({"kind": "figma-metrics", "dpr": dpr, "frame": client.frame(client.window())})
    retained()

    # A live candidate must equal the subsequently committed renderer rectangle.
    asset_group = client.group(ids["assets"])
    bounds = client.node(client.group_id(asset_group))["rect"]
    target = [bounds[0]+12, (bounds[1]+bounds[3])/2]
    window = client.start_drag(notes, target)
    preview = client.wait(lambda: client.node("dock.preview"), "prospective split rectangle")
    preview_rect = preview["rect"]
    client.finish_drag(window)
    client.wait(lambda: client.group(notes) != asset_group and client.group(notes) != state_group(state, notes), "split commit")
    committed = client.wait(lambda: client.node(client.group_id(client.group(notes))), "committed group")
    assert all(abs(a-b) <= 1 for a, b in zip(preview_rect, committed["rect"])), (preview_rect, committed["rect"])
    retained()
    report["steps"].append({"kind": "split-preview-equality", "preview": preview_rect, "committed": committed["rect"]})

    before = client.state()["layout"]
    asset_bounds = client.node(client.group_id(client.group(ids["assets"])))["rect"]
    window = client.start_drag(notes, [(asset_bounds[0]+asset_bounds[2])/2, (asset_bounds[1]+asset_bounds[3])/2])
    client.wait(lambda: client.node("dock.preview"), "merge preview")
    client.event(window=window, kind="key", key="escape")
    client.finish_drag(window)
    assert client.state()["layout"] == before, "Escape committed a candidate"
    retained()
    report["steps"].append({"kind": "escape-cancel"})

    divider = next(e for e in client.observe()["elements"] if e["visible"]
                   and e["id"].endswith("/divider") and e["rect"][2]-e["rect"][0] < 8)
    r = divider["rect"]
    position = [(r[0]+r[2])/2, (r[1]+r[3])/2]
    client.event(kind="move", position=position)
    client.event(kind="button", pressed=True)
    client.event(kind="move", position=[position[0]+70, position[1]])
    assert client.state()["layout"] == before, "Live resize published before release"
    client.event(kind="key", key="escape")
    client.event(kind="button", pressed=False)
    assert client.state()["layout"] == before, "Escape failed to roll back resize"
    client.event(kind="move", position=position)
    client.event(kind="button", pressed=True)
    client.event(kind="move", position=[position[0]+50, position[1]])
    client.event(kind="button", pressed=False)
    client.wait(lambda: client.state()["layout"] != before, "committed resize")
    before = client.state()["layout"]
    retained()
    report["steps"].append({"kind": "resize-live-cancel-and-commit"})

    group = client.group(notes)
    client.menu(notes,"Maximize group")
    maximized = client.wait(lambda: client.node(client.group_id(group)), "maximized group")
    host = client.node("workspace/host")["rect"]
    client.wait(lambda: all(abs(a-b) <= 1 for a, b in zip(client.node(client.group_id(group))["rect"], host)), "maximize fills host")
    client.menu(notes,"Restore group")
    assert client.state()["layout"] == before
    retained()
    report["steps"].append({"kind": "maximize-restore", "group": maximized["id"]})

    client.menu(notes, "Float group")
    client.wait(lambda: len(client.observe()["windows"]) == 2, "second native window")
    retained()
    floating = client.node(draft_id)["window"]
    assert floating != client.window()
    report["steps"].append({"kind": "float", "frame": client.frame(floating)})
    client.menu(notes, "Redock to main window", retired=True)
    client.wait(lambda: len(client.observe()["windows"]) == 1, "retired floating window")
    retained()
    report["steps"].append({"kind": "redock-retains-panel-content"})

    client.menu(notes, "Float group")
    client.wait(lambda: len(client.observe()["windows"]) == 2, "second float")
    client.click("floating-caption/window.close", retired=True)
    client.wait(lambda: len(client.observe()["windows"]) == 1, "floating close")
    assert notes in client.state()["layout"]["hidden"]
    retained()
    client.click("workspace/panels")
    item = client.wait(lambda: next((e for e in client.observe()["elements"]
                                     if e["visible"] and e.get("text") == ("Show Document Preview" if "preview" in ids else "Show Scratchpad")), None), "reopen tool")
    client.click(item["id"].removesuffix("/caption"))
    client.wait(lambda: client.node(draft_id), "reopened panel")
    retained()
    report["steps"].append({"kind": "floating-close-hide-reopen"})

    client.menu(notes, "Float group")
    client.wait(lambda: len(client.observe()["windows"]) == 2, "persisted floating host")
    retained()
    report["layout"] = client.state()["layout"]
    report["frames"] = [client.frame(w["id"]) for w in client.observe()["windows"]]
    report["physical_cross_monitor_input"] = "not verified"
    report["status"] = "passed"
    return report


def state_group(state, view):
    return next(group for group, node in state["layout"]["nodes"].items()
                if view in node.get("Group", {}).get("tabs", []))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("credentials", type=Path)
    parser.add_argument("--state-directory", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--restore-report", type=Path)
    args = parser.parse_args()
    client = Docking(args.credentials)
    if args.restore_report:
        previous = json.loads(args.restore_report.read_text())
        client.wait(lambda: client.state().get("bound"), "restored workspace")
        client.wait(lambda: len(client.observe()["windows"]) == len(previous["layout"]["hosts"]), "restored native hosts")
        assert client.state()["layout"] == previous["layout"], "Restored topology changed"
        report = {"status": "passed", "kind": "restart-restores-native-hosts-and-topology",
                  "frames": [client.frame(w["id"]) for w in client.observe()["windows"]]}
    else:
        report = check(client, args.state_directory)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2))
    print("PASS:", report.get("kind", "native docking input, geometry, retained state and floating hosts"))


if __name__ == "__main__":
    main()
