#!/usr/bin/env python3
"""Opt-in Inspector native QA; real frames and injected input, not physical acceptance."""
import argparse
import json
import tempfile
import uuid
from pathlib import Path

from docking import Docking
from scene_view import inventory


def inputs(client, prefix):
    observation = client.observe()
    assert not observation["truncated"], "Inspector observation was truncated"
    return [node for node in observation["elements"]
            if node["visible"] and node["id"].startswith(prefix)
            and node.get("value") is not None]


def readonly(client, node):
    client.click(node["id"])
    client.event(window=node["window"], kind="key", key="select_all")
    try:
        client.event(window=node["window"], kind="text", text="Must not mutate")
    except AssertionError as error:
        assert "read-only" in str(error).lower(), error
    current = client.node(node["id"])
    assert current and current["entity"] == node["entity"]
    assert current["value"] == node["value"], "Read-only native input accepted mutation"


def run_fixture(client):
    prefix = "fixture/settings/"
    client.wait(lambda: any(n["value"] == "25%" for n in inputs(client, prefix)),
                "real field specialization", timeout=40)
    values = inputs(client, prefix)
    assert any(n["value"] == "3 / 9" for n in values), "Missing exact-type specialization"
    assert any(n["value"] == "18446744073709551615" for n in values), "Integer precision lost"
    readonly(client, next(n for n in values if n["value"] == "25%"))
    frames = [client.frame(client.window())]
    boolean = next(n for n in inputs(client, prefix) if n["value"] == "true")
    client.click("fixture/refresh")
    client.wait(lambda: client.node(boolean["id"])["value"] == "false", "host refresh")
    assert client.node(boolean["id"])["entity"] == boolean["entity"]
    client.click("fixture/toggle")
    client.wait(lambda: any(n["value"] == "0.25" for n in inputs(client, prefix)), "default view")
    assert not any(n["value"] == "25%" for n in inputs(client, prefix))
    frames.append(client.frame(client.window()))
    client.completed(client.operation("app.resize", window=client.window(), width=320, height=740, dpr=1))
    client.wait(lambda: client.node("fixture/toggle"), "narrow native layout")
    frames.append(client.frame(client.window()))
    return {"frames": frames, "verified": ["type and field specialization", "explicit default view",
            "exact integer", "read-only native input", "same-target refresh", "narrow layout captured"],
            "limitations": ["injected input, not physical OS input", "OS clipboard and IME not certified",
                            "captures require visual review; not Figma pixel equivalence"]}


def source(identity, root, title, components, instances=""):
    return (f'(version:1,id:"{identity}",kind:Prefab,title:"{title}",root:"{root}",base:None,'
            f'nodes:[(id:"{root}",name:"{title}",parent:None,components:{{{components}}})],'
            f'instances:[{instances}],overrides:[],ports:{{}})')


def run_editor(client, state_directory):
    assert Path(client.rpc("editor.state")["preferences"]).parent == state_directory.resolve(), "Refusing personal preferences"
    client.wait(lambda: client.rpc("editor.state")["phase"] == "projects", "Projects", timeout=40)
    state_directory.mkdir(parents=True, exist_ok=True)
    parent = Path(tempfile.mkdtemp(prefix="inspector-", dir=state_directory))
    client.completed(client.operation("editor.project", command={"kind": "create", "parent": str(parent), "name": "Inspector QA"}))
    assets = parent / "Inspector QA" / "assets"
    child, child_root, main, main_root, other, other_root, instance = [str(uuid.uuid4()) for _ in range(7)]
    caption = '"ui.caption":(version:1,value:(text:"Inherited 日本語",justify:Left,wrap:false))'
    nested = (f'(id:"{instance}",asset:"{child}",parent:(instances:[],node:"{main_root}"),bindings:{{}},'
              f'overrides:[SetField(node:(instances:[],node:"{child_root}"),component:"ui.caption",version:1,path:"text",value:"Override 日本語")])')
    files = {"Child.prefab.ron": source(child, child_root, "Repeated name", caption),
             "Main.prefab.ron": source(main, main_root, "Main", '"ui.editable":(version:1,value:(autofocus:false,select_all:true))', nested),
             "Other.prefab.ron": source(other, other_root, "Repeated name", caption.replace("Inherited 日本語", "Other source"))}
    for name, text in files.items():
        (assets / name).write_text(text, encoding="utf-8")
    before = inventory(assets)
    tools = {v["kind"]: key for key, v in client.state()["layout"]["views"].items()}
    inspector, hierarchy, browser = tools["inspector"], tools["hierarchy"], tools["assets"]
    prefix = f"inspector/ViewId({inspector})/"

    def open_asset(name, identity):
        client.menu(browser, "Refresh sources")

        def item():
            nodes = [n for n in client.observe()["elements"] if n["visible"]]
            buttons = {n["entity"]: n["id"] for n in nodes if n["id"].startswith("browser/") and "/item/" in n["id"]}
            return next((buttons[n["parent"]] for n in nodes if n.get("text") == name and n["parent"] in buttons), None)
        client.click(client.wait(item, name))
        client.menu(browser, "Open selected source")
        client.wait(lambda: client.state()["documents"]["active"] == identity, "source activation")

    client.wait(lambda: client.node(prefix + "message"), "no-selection guidance")
    open_asset("Main.prefab.ron", main)

    def nested_row():
        nodes = [n for n in client.observe()["elements"] if n["visible"]]
        label = next((n for n in nodes if n["id"].startswith(f"hierarchy/ViewId({hierarchy})/") and n.get("text") == "Repeated name"), None)
        return next((n["id"] for n in nodes if label and n["entity"] == label["parent"]), None)
    client.click(client.wait(nested_row, "nested source node"))
    field = client.wait(lambda: next((n for n in inputs(client, prefix) if n["value"] == "Override 日本語"), None), "effective overridden field")
    readonly(client, field)
    client.menu(inspector, "Lock Inspector")
    open_asset("Other.prefab.ron", other)
    client.wait(lambda: client.node(field["id"]), "locked source survives document navigation")
    assert client.node(field["id"])["entity"] == field["entity"]
    client.menu(inspector, "Search properties")
    client.click(prefix + "search")
    client.event(kind="text", text="text")
    client.wait(lambda: client.node(prefix + "search")["value"] == "text", "property search")
    assert client.node(prefix + "name")["text"] == "Repeated name"
    search_entity = client.node(prefix + "search")["entity"]
    frames = [client.frame(client.window())]
    client.menu(inspector, "Float group")
    client.wait(lambda: client.node(prefix + "search")["window"] != client.window(), "floating Inspector")
    floating = client.node(prefix + "search")["window"]
    client.completed(client.operation("app.resize", window=floating, width=320, height=640, dpr=1))
    frames.append(client.frame(floating))
    assert client.node(field["id"])["entity"] == field["entity"]
    assert client.node(prefix + "search")["entity"] == search_entity
    client.menu(inspector, "Redock to main window", retired=True)
    client.menu(inspector, "Hide group")
    client.wait(lambda: inspector in client.state()["layout"]["hidden"], "hidden Inspector")
    client.click("workspace/panels")
    show = client.wait(lambda: next((n for n in client.observe()["elements"] if n["visible"] and n.get("text") == "Show Inspector"), None), "Show Inspector")
    client.click(show["id"].removesuffix("/caption"))
    client.wait(lambda: client.node(prefix + "search"), "reopened Inspector")
    assert client.node(field["id"])["entity"] == field["entity"]
    assert client.node(prefix + "search")["entity"] == search_entity
    client.click(prefix + "close-search")
    # Redock can place Inspector beside documents in the same tab group. Restore
    # separate default tool/document groups before testing visible unlocked follow.
    client.click("workspace/reset")
    other_view = next(d["view"] for d in client.state()["documents"]["documents"] if d["document"] == other)
    client.click(client.label(other_view))
    client.wait(lambda: client.state()["documents"]["active"] == other, "reactivated source document")
    client.menu(inspector, "Unlock Inspector")
    client.wait(lambda: any(n["value"] == "Other source" for n in inputs(client, prefix)), "unlocked document following")
    assert before == inventory(assets), "Read-only Inspector changed source bytes or sidecars"
    return {"project": str(assets.parent), "frames": frames,
            "verified": ["effective nested override", "source-only inspection without renderer delivery", "read-only native input",
                         "identity-based lock", "search context", "retained float/redock/hide/reopen", "no source writes"],
            "limitations": ["injected input, not physical OS input", "OS clipboard/IME and screen readers not certified",
                            "captures need visual review; not Figma pixel equivalence"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("credentials", type=Path)
    parser.add_argument("--fixture", action="store_true")
    parser.add_argument("--state-directory", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if not args.fixture and args.state_directory is None:
        parser.error("editor QA requires --state-directory isolation")
    client = Docking(args.credentials)
    report = run_fixture(client) if args.fixture else run_editor(client, args.state_directory)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"PASS: native Inspector checks; report: {args.report}")


if __name__ == "__main__":
    main()
