#!/usr/bin/env python3
"""Opt-in native browser/Open QA. Creates only isolated project fixture sources.

Run a fresh editor at the project manager with an isolated --state-directory and
--remote-credentials. Tests injected Bevy input and captures actual renderer frames,
not physical OS input, IME or independent Figma pixel equivalence.
"""
import argparse
import json
import tempfile
import uuid
from pathlib import Path

from docking import Docking


def document(title, kind="Prefab"):
    identity, node = uuid.uuid4(), uuid.uuid4()
    return (f'(version:1,id:"{identity}",kind:{kind},title:"{title}",root:"{node}",'
            f'base:None,nodes:[(id:"{node}",name:"Root",parent:None,components:{{}})],'
            'instances:[],overrides:[],ports:{})')


def run(client, state_directory):
    state_directory = state_directory.resolve()
    assert Path(client.rpc("editor.state")["preferences"]) == state_directory / "recent-projects.toml"
    client.wait(lambda: client.rpc("editor.state")["phase"] == "projects", "project manager", 30)
    state_directory.mkdir(parents=True, exist_ok=True)
    parent = Path(tempfile.mkdtemp(prefix="browser-", dir=state_directory))
    client.completed(client.operation("editor.project", command={"kind": "create", "parent": str(parent), "name": "Browser QA"}))
    assets = parent / "Browser QA" / "assets"
    folder = assets / "Prefabs"
    folder.mkdir()
    source = folder / "Example.prefab.ron"
    source.write_text("invalid source", encoding="utf-8")
    other = folder / "Main.scene.ron"
    other_bytes = document("Main", "Scene")
    other.write_text(other_bytes, encoding="utf-8")
    (assets / "Straße.txt").write_text("not executed", encoding="utf-8")
    (assets / "hidden.meta").write_text("not an independent source", encoding="utf-8")
    for index in range(200):
        (assets / f"z{index:03}.txt").write_text("source", encoding="utf-8")
    tree_folders = tuple(assets / f"tree-{index:02}" for index in range(40))
    for tree_folder in tree_folders:
        tree_folder.mkdir()

    def nodes():
        snapshot = client.observe()
        assert not snapshot["truncated"]
        return [node for node in snapshot["elements"] if node["visible"]]

    def control(suffix):
        return next((node["id"] for node in nodes() if node["id"].startswith("browser/") and node["id"].endswith("/" + suffix)), None)

    def text(suffix, value):
        return any(node.get("text") and value in node["text"] and node["id"].endswith("/" + suffix) for node in nodes())

    def click(suffix):
        if suffix in ("refresh", "open"):
            view = next(identity for identity, record in client.state()["layout"]["views"].items() if record["kind"] == "assets")
            client.menu(view, "Refresh sources" if suffix == "refresh" else "Open selected source")
            return
        client.click(client.wait(lambda: control(suffix), suffix))

    def select(name):
        def target():
            current = nodes()
            parents = {node["entity"]: node["id"] for node in current if "/item/" in node["id"]}
            label = next((node for node in current if node.get("text") == name and node["id"].endswith("/name") and node["parent"] in parents), None)
            return parents.get(label["parent"]) if label else None
        client.click(client.wait(target, name))

    def search(value):
        click("search")
        field = client.node(control("search"))
        client.event(window=field["window"], kind="key", key="select_all")
        client.event(window=field["window"], kind="text", text=value)
        client.wait(lambda: client.node(control("search"))["value"] == value, "search input")

    def documents():
        return client.state()["documents"]

    click("refresh")
    client.wait(lambda: text("name", "Prefabs"), "root entries")
    tree_root = client.wait(lambda: control("folder/0"), "native folder root")
    prefix = tree_root.removesuffix("0")
    client.click(tree_root)

    def revealed_folder(index):
        node = client.node(prefix + str(index))
        viewport = client.node(control("folders"))
        if not node or not viewport or not node["focused"]:
            return False
        return viewport["rect"][1] - 1 <= node["rect"][1] and node["rect"][3] <= viewport["rect"][3] + 1

    for key, index in [("end", 41), ("home", 0), ("down", 1),
                       ("end", 41), ("up", 40), ("down", 41), ("left", 0)]:
        client.event(kind="key", key=key)
        client.wait(lambda: revealed_folder(index), f"folder {key} reveals native focus")
    tree_frame = client.frame(client.window())
    # Keep tree stress independent of the original virtualized file-flow fixture.
    # Only retire our exact empty directories; unexpected contents must fail safely.
    for tree_folder in tree_folders:
        tree_folder.rmdir()
    click("refresh")
    client.wait(lambda: text("count", "202/202 items"), "tree fixture retired and root listing refreshed")
    disabled = client.node(control("new"))
    assert not disabled["enabled"]
    bounds = disabled["rect"]
    client.event(kind="move", position=[(bounds[0] + bounds[2]) / 2, (bounds[1] + bounds[3]) / 2])
    client.wait(lambda: text("tooltip", "New is unavailable"), "disabled command reason")
    assert not text("name", "hidden.meta")
    assert len([n for n in nodes() if "/item/" in n["id"]]) < 100, "Listing was not virtualized"
    search("STRASSE")
    client.wait(lambda: text("name", "Straße.txt"), "Unicode case folding")
    client.event(kind="text", text="bad\nquery")
    assert client.node(control("search"))["value"] == "STRASSE", "Search admitted a control character"
    select("Straße.txt")
    client.wait(lambda:text("status", str(Path("assets") / "Straße.txt")), "project-relative status path")
    assert not any(str(assets) in (n.get("text") or "") for n in nodes()), "Absolute source root leaked into editor UI"
    client.wait(lambda:any(n["id"].endswith("/tooltip") and n.get("text")=="Straße.txt" for n in nodes()),"asset tooltip shows filename only")
    label = next(n for n in nodes() if n["id"].endswith("/name") and n.get("text") == "Straße.txt")
    tile = next(n for n in nodes() if n["entity"] == label["parent"])
    x0, y0, x1, y1 = tile["rect"]
    pointer = [(x0 + x1) / 2, (y0 + y1) / 2]
    client.event(window=tile["window"], kind="move", position=pointer)
    def compact_hint():
        hint = next((n for n in nodes() if n["id"].endswith("/tooltip")), None)
        if not hint:
            return False
        left, top, right, bottom = hint["rect"]
        return 20 < right - left <= 160 and 16 <= bottom - top <= 40 and abs(left - pointer[0]) <= 170 and abs(top - pointer[1]) <= 52
    client.wait(compact_hint, "compact tooltip beside cursor, not a panel-width banner")
    client.event(kind="move", position=[8, 40])
    client.wait(lambda:not any(n["id"].endswith("/tooltip") for n in nodes()), "tooltip retires after pointer leaves")
    assert documents()["active"] is None
    search("no matches")
    client.wait(lambda: text("message", "No sources match"), "empty filter results")
    client.wait(lambda: text("status", "filtered selection"), "hidden selection remains identified")
    click("clear")
    click("list")
    select("Straße.txt")
    client.event(kind="key", key="end")
    client.wait(lambda: text("status", "z199.txt"), "keyboard End through virtual list")
    list_frame = client.frame(client.window())
    thumb = client.wait(lambda: client.node(control("content-thumb")), "native scrollbar thumb")
    viewport = client.node(control("entries"))["rect"]
    x = (thumb["rect"][0] + thumb["rect"][2]) / 2
    y = (thumb["rect"][1] + thumb["rect"][3]) / 2
    client.event(kind="move", position=[x, y])
    client.event(kind="button", pressed=True)
    client.event(kind="move", position=[x, viewport[1] + 10])
    client.event(kind="button", pressed=False)
    client.wait(lambda: text("name", "Prefabs"), "native thumb drag reveals top of virtual list")
    click("grid")
    search("Prefabs")
    select("Prefabs")
    click("open")
    client.wait(lambda: text("name", "Example.prefab.ron"), "folder navigation")
    click("back")
    client.wait(lambda: client.node(control("search"))["value"] == "Prefabs", "folder history restores filter")
    click("forward")
    select("Example.prefab.ron")
    click("open")
    client.wait(lambda: text("message", "Cannot open source"), "malformed source admission")
    assert documents()["active"] is None
    source.write_text(document("Example"), encoding="utf-8")
    select("Example.prefab.ron")
    select("Example.prefab.ron")
    client.wait(lambda: len(documents()["documents"]) == 1, "admitted source document")
    assert not any(str(assets) in (n.get("text") or "") for n in nodes()), "Absolute source root leaked after Open"
    assert not any(node["id"].startswith("panel/") and node.get("value") is not None for node in nodes()), "Read-only document mounted an editable field"
    active = documents()["active"]
    click("open")
    assert len(documents()["documents"]) == 1, "Duplicate Open created another document"
    other.write_text("changed after admission", encoding="utf-8")
    select("Main.scene.ron")
    client.event(kind="key", key="enter")
    client.wait(lambda: text("message", "changed since admission"), "stale source refusal")
    assert documents()["active"] == active
    other.write_text(other_bytes, encoding="utf-8")
    click("open")
    client.wait(lambda: len(documents()["documents"]) == 2, "second document shares admitted scope")
    active_view = next(d["view"] for d in documents()["documents"] if d["document"] == documents()["active"])
    original_group = client.group(active_view)
    client.click(client.label(active_view))
    label = client.node(client.label(active_view))
    bounds = label["rect"]
    client.event(kind="move", position=[(bounds[0] + bounds[2]) / 2, (bounds[1] + bounds[3]) / 2])
    client.click(f"dock/view/ViewId({active_view})/close")
    client.wait(lambda: active_view in client.state()["layout"]["hidden"], "closed read-only document view")
    assert len(documents()["documents"]) == 2
    click("open")
    client.wait(lambda: active_view not in client.state()["layout"]["hidden"], "reopened accepted document")
    assert client.group(active_view) == original_group
    clip, count = client.node(control("status-clip")), client.node(control("count"))
    assert clip["rect"][2] < count["rect"][0], "Source path overlaps the item count"
    selected_icon = next(n for n in nodes() if n["id"].endswith("/selected-folder"))
    folder_label = next(n for n in nodes() if n["parent"] == selected_icon["parent"] and n.get("text") == "Prefabs")
    assert selected_icon["rect"][2] < folder_label["rect"][0], "Selected folder icon moved after its label"
    source_frame = client.frame(client.window())
    admitted = documents()
    intended = next(d for d in admitted["documents"] if d["view"] == active_view)
    assert admitted["active"] == intended["document"]
    assets_view = next(identity for identity, record in client.state()["layout"]["views"].items() if record["kind"] == "assets")
    search_id = control("search")
    search_entity = client.node(search_id)["entity"]
    client.menu(assets_view, "Float group")
    client.wait(lambda: len(client.observe()["windows"]) == 2, "floating browser")
    assert client.node(search_id)["entity"] == search_entity
    floating = client.node(search_id)["window"]
    client.completed(client.operation("app.resize", window=floating, width=480, height=560, dpr=2))
    narrow_frame = client.frame(floating)
    client.menu(assets_view, "Redock to main window", retired=True)
    client.wait(lambda: len(client.observe()["windows"]) == 1, "redocked browser")
    client.menu(assets_view, "Hide group")
    client.wait(lambda: assets_view in client.state()["layout"]["hidden"], "hidden browser")
    client.click("workspace/panels")
    item = client.wait(lambda: next((n for n in nodes() if n.get("text") == "Show Assets"), None), "reopen browser")
    client.click(item["id"].removesuffix("/caption"))
    client.wait(lambda: client.node(search_id), "reopened browser")
    assert client.node(search_id)["entity"] == search_entity
    assert text("status", str(Path("assets") / "Prefabs" / "Main.scene.ron")), "Browser lost its selected source"
    assert documents()["documents"] == admitted["documents"], "Hide/reopen changed admitted documents or selections"
    # Redock can merge Assets with document tabs; Hide group then hides both.
    # Restore the selected admitted source through real Open before testing Reset.
    click("open")
    client.wait(lambda: documents()["active"] == intended["document"]
                and intended["view"] not in client.state()["layout"]["hidden"],
                "selected admitted document visible and active before Reset")
    assert documents()["documents"] == admitted["documents"], "Reopen duplicated documents or lost selections"
    assert client.state()["layout"]["nodes"][client.group(intended["view"])]["Group"]["active"] == intended["view"]
    opened = documents()
    client.click("workspace/reset")
    client.wait(lambda: client.node(search_id), "reset retains browser")
    assert client.node(search_id)["entity"] == search_entity
    assert documents()["documents"] == opened["documents"]
    assert documents()["active"] == opened["active"]
    active_view = next(d["view"] for d in documents()["documents"] if d["document"] == opened["active"])
    assert client.state()["layout"]["nodes"][client.group(active_view)]["Group"]["active"] == active_view
    assert client.group(active_view) != client.group(assets_view)
    assert (assets / "Straße.txt").read_text(encoding="utf-8") == "not executed"
    assert not source.with_name(source.name + ".meta").exists()
    return {"project": str(parent / "Browser QA"), "frames": [tree_frame, list_frame, source_frame, narrow_frame],
            "verified": ["folder keyboard focus/reveal", "Unicode folder search", "hidden selection", "grid/list", "virtualized keyboard navigation", "native scrollbar drag", "history filter retention", "malformed source failure", "real source admission", "double-click/Enter", "duplicate Open", "changed source refusal", "shared document scope", "float/redock", "hide/reopen", "retained native search"],
            "not_verified": ["physical OS input", "independent Figma pixel equivalence", "IME", "rendered document preview", "source edits/Save"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("credentials", type=Path)
    parser.add_argument("--state-directory", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    result = run(Docking(args.credentials), args.state_directory)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"Source browser checks passed; report: {args.report}")


if __name__ == "__main__":
    main()
