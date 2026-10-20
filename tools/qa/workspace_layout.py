#!/usr/bin/env python3
"""Verify the authoring preset with injected native input and renderer frames.

Requires an open project and an explicitly isolated --state-directory. Leaves the
editor in its startup preset for manual inspection; never modifies project sources.
"""
import argparse
import json
from pathlib import Path

from docking import Docking


def run(client, state_directory):
    preferences = Path(client.rpc("editor.state")["preferences"])
    assert preferences.resolve().parent == state_directory.resolve(), "Refusing personal preferences"
    client.wait(lambda: client.state().get("bound"), "workspace")
    client.completed(client.operation("app.resize", window=client.window(), width=1440, height=900, dpr=2))
    layout = client.state()["layout"]
    ids = {view["kind"]: identity for identity, view in layout["views"].items()}
    assert set(ids) == {"hierarchy", "preview", "inspector", "assets"}
    roots = {}
    for kind in ("preview", "inspector"):
        identity = f"panel/ViewId({ids[kind]})/empty"
        root = client.wait(lambda: client.node(identity), kind)
        assert not any(element["parent"] == root["entity"] for element in client.observe()["elements"]), "Placeholder is not empty"
        roots[kind] = (identity, root["entity"])
    identity = f"hierarchy/ViewId({ids['hierarchy']})/entries"
    root = client.wait(lambda: client.node(identity), "Hierarchy viewport")
    roots["hierarchy"] = (identity, root["entity"])

    def placement():
        groups = {kind: client.node(client.group_id(client.group(identity))) for kind, identity in ids.items()}
        if not all(groups.values()):
            return False
        h, p, i, a = (groups[k]["rect"] for k in ("hierarchy", "preview", "inspector", "assets"))
        assert h[2] <= p[0] and p[2] <= i[0]
        assert p[3] <= a[1]
        assert abs(p[0] - a[0]) <= 1 and abs(p[2] - a[2]) <= 1
        assert abs(h[1] - i[1]) <= 1 and abs(h[3] - i[3]) <= 1
        assert all(client.node(client.label(identity)) for identity in ids.values())
        return {kind: node["rect"] for kind, node in groups.items()}

    geometry = client.wait(placement, "startup placement")
    frames = [client.frame(client.window())]
    inspector = ids["inspector"]
    identity, entity = roots["inspector"]
    client.menu(inspector, "Float group")
    client.wait(lambda: len(client.observe()["windows"]) == 2, "floating Inspector")
    assert client.node(identity)["entity"] == entity
    frames.append(client.frame(client.node(identity)["window"]))
    client.menu(inspector, "Redock to main window", retired=True)
    client.wait(lambda: len(client.observe()["windows"]) == 1, "redocked Inspector")
    client.menu(inspector, "Hide group")
    client.wait(lambda: inspector in client.state()["layout"]["hidden"], "hidden Inspector")
    client.click("workspace/panels")
    item = client.wait(lambda: next((e for e in client.observe()["elements"] if e["visible"] and e.get("text") == "Show Inspector"), None), "reopen Inspector")
    client.click(item["id"].removesuffix("/caption"))
    client.wait(lambda: client.node(identity), "reopened Inspector")
    assert client.node(identity)["entity"] == entity
    client.click("workspace/reset")
    client.wait(placement, "reset placement")
    for identity, entity in roots.values():
        assert client.node(identity)["entity"] == entity, "Reset rebuilt a placeholder"
    frames.append(client.frame(client.window()))
    return {"status": "passed", "geometry": geometry, "frames": frames,
            "verified": ["four visible panels", "startup placement", "empty authored content", "float/redock", "hide/reopen", "reset retains identity"],
            "limitations": ["injected Bevy input, not physical OS input", "not Figma pixel acceptance"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("credentials", type=Path)
    parser.add_argument("--state-directory", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    report = run(Docking(args.credentials), args.state_directory)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(f"PASS: authoring startup layout; report: {args.report}")


if __name__ == "__main__":
    main()
