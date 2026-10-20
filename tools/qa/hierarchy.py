#!/usr/bin/env python3
"""Opt-in native read-only Hierarchy acceptance; isolated sources, input and frames."""
import argparse
import json
import tempfile
import uuid
from pathlib import Path

from docking import Docking


def source(title, kind="Prefab", count=0):
    identity, root = str(uuid.uuid4()), str(uuid.uuid4())
    nodes = [f'(id:"{root}",name:"{title}",parent:None,components:{{}})']
    leaves = []
    for index in range(count):
        node = str(uuid.uuid4())
        leaves.append(node)
        name = "Straße leaf" if index == 0 else f"Node {index:03}"
        nodes.append(f'(id:"{node}",name:"{name}",parent:Some((instances:[],node:"{root}")),components:{{}})')
    return identity, root, nodes, leaves


def encode(identity, root, title, nodes, instances=(), kind="Prefab", order=()):
    children = ",".join(f'(instances:[],node:"{node}")' for node in order)
    overrides = f'SetChildOrder(node:(instances:[],node:"{root}"),children:[{children}])' if order else ""
    return (f'(version:1,id:"{identity}",kind:{kind},title:"{title}",root:"{root}",base:None,'
            f'nodes:[{",".join(nodes)}],instances:[{",".join(instances)}],overrides:[{overrides}],ports:{{}})')


def run(c, state_directory):
    state_directory = state_directory.resolve()
    assert Path(c.rpc("editor.state")["preferences"]).parent == state_directory
    c.wait(lambda: c.rpc("editor.state")["phase"] == "projects", "Projects")
    state_directory.mkdir(parents=True, exist_ok=True)
    parent = Path(tempfile.mkdtemp(prefix="hierarchy-", dir=state_directory))
    c.completed(c.operation("editor.project", command={"kind": "create", "parent": str(parent), "name": "Hierarchy QA"}))
    assets = parent / "Hierarchy QA" / "assets"
    child, child_root, child_nodes, leaves = source("Shared prefab", count=180)
    main, main_root, main_nodes, _ = source("Main scene", kind="Scene")
    scopes = [str(uuid.uuid4()), str(uuid.uuid4())]
    instances = [f'(id:"{scope}",asset:"{child}",parent:(instances:[],node:"{main_root}"),bindings:{{}},overrides:[])' for scope in scopes]
    second, second_root, second_nodes, _ = source("Other root")
    files = {
        "Main.scene.ron": encode(main, main_root, "Main scene", main_nodes, instances, "Scene"),
        "Shared.prefab.ron": encode(child, child_root, "Shared prefab", child_nodes, order=leaves),
        "Other.prefab.ron": encode(second, second_root, "Other root", second_nodes),
    }
    for name, data in files.items():
        (assets / name).write_text(data, encoding="utf-8")

    def nodes():
        observation = c.observe()
        assert not observation["truncated"], "Hierarchy exceeded observation limits"
        return [n for n in observation["elements"] if n["visible"]]

    def tool(kind):
        return next(identity for identity, view in c.state()["layout"]["views"].items() if view["kind"] == kind)

    hierarchy = c.wait(lambda: tool("hierarchy"), "Hierarchy tool")
    prefix = f"hierarchy/ViewId({hierarchy})/"

    def control(key):
        return prefix + key

    def labels(name):
        return [n for n in nodes() if n["id"].startswith(prefix) and n.get("text") == name]

    def rows(name):
        current = nodes()
        return [n for n in current if any(n["entity"] == label["parent"] for label in labels(name))]

    def search(value):
        field = control("search")
        if not c.node(field):
            c.menu(hierarchy, "Search nodes")
        c.click(field)
        c.event(kind="key", key="select_all")
        c.event(kind="text", text=value)
        c.wait(lambda: c.node(field)["value"] == value, "Hierarchy search")

    def current():
        documents = c.state()["documents"]
        return next(d for d in documents["documents"] if d["document"] == documents["active"])

    def open_asset(name, expected=True):
        assets_id = tool("assets")
        c.menu(assets_id, "Refresh sources")
        def item():
            current_nodes = nodes()
            buttons = {n["entity"]: n["id"] for n in current_nodes if n["id"].startswith("browser/") and "/item/" in n["id"]}
            return next((buttons[n["parent"]] for n in current_nodes if n.get("text") == name and n["parent"] in buttons), None)
        c.click(c.wait(item, name))
        c.menu(assets_id, "Open selected source")
        if expected:
            c.wait(lambda: c.state()["documents"]["active"] == {"Main.scene.ron": main, "Other.prefab.ron": second}[name], "source activation")

    c.wait(lambda: c.node(control("message")), "no-document guidance")
    open_asset("Main.scene.ron")
    c.wait(lambda: labels("Main scene"), "composed root")
    assert current()["selection"] == {"instances": [], "node": main_root}
    assert len([n for n in nodes() if n["id"].startswith(prefix + "node/")]) < 100
    search("STRASSE")
    c.wait(lambda: len(rows("Straße leaf")) == 2, "both scoped prefab leaves")
    first, other = rows("Straße leaf")
    c.click(first["id"])
    one = current()["selection"]
    c.click(other["id"])
    two = current()["selection"]
    assert one["node"] == two["node"] == leaves[0]
    assert one["instances"] != two["instances"]
    search("nothing matches")
    c.wait(lambda: any(n.get("text") == "No nodes match this search." for n in nodes()), "empty filter state")
    assert current()["selection"] == two
    c.menu(hierarchy, "Reveal selected node")
    c.wait(lambda: c.node(control("search"))["value"] == "", "Reveal clears filter")
    c.wait(lambda: any(n["focused"] for n in rows("Straße leaf")), "Reveal scroll/focus")
    c.event(kind="key", key="end")
    c.wait(lambda: current()["selection"]["node"] == leaves[-1], "End reaches virtual tail")
    end_selection = current()["selection"]
    full_frame = c.frame(c.window())
    c.event(kind="key", key="home")
    c.wait(lambda: current()["selection"]["node"] == main_root, "Home returns to root")
    c.event(kind="key", key="left")
    c.wait(lambda: len([n for n in nodes() if n["id"].startswith(prefix + "node/")]) == 1, "Left collapses root")
    c.event(kind="key", key="right")
    c.wait(lambda: len([n for n in nodes() if n["id"].startswith(prefix + "node/")]) > 1, "Right expands root")
    search("STRASSE")
    c.wait(lambda: len(rows("Straße leaf")) == 2, "filter restored")
    c.click(rows("Straße leaf")[0]["id"])
    saved = current()["selection"]
    field_entity = c.node(control("search"))["entity"]
    (assets / "Other.prefab.ron").write_text("changed", encoding="utf-8")
    open_asset("Other.prefab.ron", expected=False)
    c.wait(lambda: c.node(control("message")) and "Showing the previously admitted document" in c.node(control("message"))["text"], "failed Open preserves identified hierarchy")
    assert current()["selection"] == saved
    (assets / "Other.prefab.ron").write_text(files["Other.prefab.ron"], encoding="utf-8")
    open_asset("Other.prefab.ron")
    c.wait(lambda: labels("Other root"), "document switch")
    assert current()["selection"]["node"] == second_root
    main_view = next(d["view"] for d in c.state()["documents"]["documents"] if d["document"] == main)
    c.click(c.label(main_view))
    c.wait(lambda: c.node(control("search"))["value"] == "STRASSE", "per-document filter retained")
    assert current()["selection"] == saved
    c.menu(hierarchy, "Float group")
    c.wait(lambda: c.node(control("search"))["window"] != c.window(), "floating Hierarchy")
    floating = c.node(control("search"))["window"]
    c.completed(c.operation("app.resize", window=floating, width=320, height=640, dpr=2))
    floated_frame = c.frame(floating)
    assert c.node(control("search"))["entity"] == field_entity
    assert current()["selection"] == saved
    c.menu(hierarchy, "Redock to main window", retired=True)
    c.menu(hierarchy, "Hide group")
    c.wait(lambda: hierarchy in c.state()["layout"]["hidden"], "hidden Hierarchy")
    assert current()["selection"] == saved
    c.click("workspace/panels")
    item = c.wait(lambda: next((n for n in nodes() if n.get("text") == "Show Hierarchy"), None), "Show Hierarchy")
    c.click(item["id"].removesuffix("/caption"))
    c.wait(lambda: c.node(control("search")), "reopened Hierarchy")
    assert c.node(control("search"))["entity"] == field_entity
    assert c.node(control("search"))["value"] == "STRASSE"
    c.click("workspace/reset")
    c.wait(lambda: c.node(control("search")), "Reset preserves Hierarchy")
    assert c.node(control("search"))["entity"] == field_entity
    assert current()["selection"] == saved
    for name, data in files.items():
        assert (assets / name).read_text(encoding="utf-8") == data
        assert not (assets / (name + ".meta")).exists()
    return {"project": str(parent / "Hierarchy QA"), "frames": [full_frame, floated_frame],
            "selection_examples": [one, two, end_selection],
            "verified": ["composed nodes", "distinct nested instances", "Unicode search with ancestors", "hidden selection retained", "Reveal", "virtualized End/Home", "Left/Right", "failed Open preserves tree", "document switching", "retained search", "float/redock", "hide/reopen", "Reset", "no source writes"],
            "limitations": ["injected Bevy input, not physical OS input", "not Figma pixel equivalence", "no source edits", "no Inspector or rendered preview"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("credentials", type=Path)
    parser.add_argument("--state-directory", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    report = run(Docking(args.credentials), args.state_directory)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"PASS: native read-only Hierarchy; report: {args.report}")


if __name__ == "__main__":
    main()
