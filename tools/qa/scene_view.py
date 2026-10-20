#!/usr/bin/env python3
"""Read-only PNG/GLB/UI/mixed Scene View native acceptance on an isolated fixture."""
import argparse
import hashlib
import json
from pathlib import Path

from docking import Docking


def inventory(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob("*") if p.is_file()}


def run(c, project, state_directory):
    assert Path(c.rpc("editor.state")["preferences"]).parent == state_directory.resolve()
    c.wait(lambda: c.rpc("editor.state")["phase"] == "editor", "Editor", timeout=30)
    project = project.resolve()
    before = inventory(project / "assets")
    browser = next(i for i, v in c.state()["layout"]["views"].items() if v["kind"] == "assets")
    frames = []

    def open_source(name):
        search = next(n["id"] for n in c.observe()["elements"] if n["visible"] and n["id"].startswith("browser/") and n["id"].endswith("/search"))
        c.click(search)
        c.event(kind="key", key="select_all")
        c.event(kind="text", text=name)
        c.wait(lambda: c.node(search)["value"] == name, "asset filter")
        def item():
            nodes = [n for n in c.observe()["elements"] if n["visible"]]
            buttons = {n["entity"]: n["id"] for n in nodes
                       if n["id"].startswith("browser/") and "/item/" in n["id"]}
            return next((buttons[n["parent"]] for n in nodes
                         if n.get("text") == name and n["parent"] in buttons), None)
        c.click(c.wait(item, name))
        c.menu(browser, "Open selected source")
        view = c.wait(lambda: next((i for i, v in c.state()["layout"]["views"].items()
                                   if v.get("restore_key") == name), None), f"view for {name}")
        prefix = f"panel/ViewId({view})/"

        def ready():
            error = c.node(prefix + "information")
            if error and any(word in (error.get("text") or "") for word in
                             ("failed", "mismatch", "does not render", "requires", "unavailable")):
                raise AssertionError(error["text"])
            return c.node(prefix + "render") and not error
        c.wait(ready, f"renderer readiness: {name}", timeout=40)
        assert not any(n["visible"] and n["id"].startswith("browser/") and "Opening read-only asset preview" in (n.get("text") or "")
                       for n in c.observe()["elements"]), "Successful Open left the browser in a loading state"
        return view, prefix

    def capture(name, prefix):
        image = c.node(prefix + "render")
        window = image["window"]
        win = next(w for w in c.observe()["windows"] if w["id"] == window)
        frame = c.frame(window)
        frames.append({"case": name, "frame": frame, "rect": image["rect"], "dpr": win["dpr"]})

    views = {}
    for name in ("checker.png", "ship.glb", "Hud.prefab.ron", "Composed.scene.ron"):
        view, prefix = open_source(name)
        views[name] = view
        capture(name, prefix)
        assert not any(n["visible"] and (n["id"].startswith("preview/") or n.get("text") == "SCENE / SYSTEM CHECK")
                       for n in c.observe()["elements"]), "Projected UI leaked observation IDs into the serving editor"
        if name.endswith((".png", ".glb")):
            assert c.state()["documents"]["active"] is None, "Raw asset invented a source document"

    main = views["Composed.scene.ron"]
    prefix = f"panel/ViewId({main})/"
    before_documents = c.state()["documents"]
    c.menu(main, "Float active tab")
    c.wait(lambda: c.node(prefix + "render") and c.node(prefix + "render")["window"] != c.window(), "floating render target")
    window = c.node(prefix + "render")["window"]
    c.completed(c.operation("app.resize", window=window, width=640, height=480, dpr=2))
    c.wait(lambda: c.node(prefix + "render") and not c.node(prefix + "information"), "resized renderer readiness")
    capture("mixed-float", prefix)
    c.menu(main, "Redock to main window", retired=True)
    c.wait(lambda: c.node(prefix + "render") and c.node(prefix + "render")["window"] == c.window(), "redocked target")
    assert c.state()["documents"] == before_documents

    c.click(c.label(views["Hud.prefab.ron"]))
    c.click(c.label(browser))
    field = next(n for n in c.observe()["elements"] if n["visible"] and n["id"].startswith("browser/") and n["id"].endswith("/search"))
    c.click(field["id"])
    c.event(kind="key", key="select_all")
    c.event(kind="text", text="ship")
    c.wait(lambda: c.node(field["id"])["value"] == "ship", "serving editor keeps text focus")
    assert before == inventory(project / "assets"), "Read-only preview changed source bytes or sidecars"
    c.event(kind="key", key="select_all")
    c.event(kind="key", key="backspace")
    c.click(c.label(main))
    c.wait(lambda: c.node(prefix + "render") and not c.node(prefix + "information"), "restored mixed projection")
    capture("mixed-restored", prefix)
    return {"status": "passed", "project": str(project), "frames": frames,
            "verified": ["PNG", "static GLB", "passive UI prefab", "mixed scene", "float/redock", "serving input", "no source writes"],
             "limitations": ["injected input, not physical OS input", "run scene_navigation.py separately for camera navigation and scoped opaque-mesh selection", "model-specific pixels require the generated cyan/yellow fixture and scene_view_frames assertions"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("credentials", type=Path)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--state-directory", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    report = run(Docking(args.credentials), args.project, args.state_directory)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(f"PASS: native Scene View; report: {args.report}")


if __name__ == "__main__":
    main()
