#!/usr/bin/env python3
"""Native read-only Scene View orbit, zoom, selection and retained camera QA."""
import argparse
import json
from pathlib import Path
from docking import Docking
from scene_view import inventory


def toolbar_geometry(c, prefix):
    """Check complete hit targets, not merely visible/clipped element flags."""
    viewport = c.node(prefix + "viewport")
    assert viewport, "Scene viewport missing"
    bounds = viewport["rect"]
    for name in ("projection", "lighting", "grid", "views"):
        control = c.node(prefix + name)
        assert control and control["window"] == viewport["window"], name
        rect = control["rect"]
        assert bounds[0] <= rect[0] < rect[2] <= bounds[2], (name, rect, bounds)
        assert rect[3] <= bounds[1] + 1, (name, rect, bounds)
        assert abs(rect[3] - rect[1] - 26) <= 1, (name, rect)
    return viewport


def anchored_menu(c, prefix, control):
    anchor = c.node(prefix + control)
    popup = c.node("dock-menu")
    assert popup and anchor and popup["window"] == anchor["window"]
    assert popup["parent"] == anchor["entity"], "Menu lost its invoking control"
    a, p = anchor["rect"], popup["rect"]
    assert min(abs(p[1] - a[3] - 4), abs(a[1] - p[3] - 4)) <= 1, (a, p)
    assert p[0] <= a[2] and p[2] >= a[0], (a, p)
    window = next(w for w in c.observe()["windows"] if w["id"] == anchor["window"])
    # app.observe uses logical window size, the same domain as element rectangles.
    assert p[0] >= 3 and p[1] >= 3, p
    assert p[2] <= window["width"] - 3 and p[3] <= window["height"] - 3, (p, window)
    return {"anchor": a, "popup": p, "window": window["id"]}


def narrow_hosts(c, view, prefix, frames):
    """Real Float/Resize/Redock input; the resize API retains its >=320px guard."""
    def menus(case):
        viewport = toolbar_geometry(c, prefix)
        evidence = []
        for control in ("projection", "lighting", "views"):
            c.click(prefix + control)
            c.wait(lambda: c.node("scene-menu/0"), "reachable " + control)
            evidence.append(anchored_menu(c, prefix, control))
            frames.append({"case": case + "-" + control, "frame": c.frame(viewport["window"]),
                           "menu_geometry": evidence[-1]})
            if control == "lighting":
                c.click("scene-menu/1")  # Editor lighting expands the select to 88px.
                c.wait(lambda: c.node(prefix + "lighting-label").get("text") == "Editor", "editor lighting caption")
                c.wait(lambda: not c.node(prefix + "information"), "editor lighting ready")
                toolbar_geometry(c, prefix)
                c.click(prefix + "lighting")
                c.wait(lambda: c.node("scene-menu/0"), "expanded lighting menu")
                geometry = anchored_menu(c, prefix, "lighting")
                frames.append({"case": case + "-editor-lighting", "frame": c.frame(viewport["window"]), "menu_geometry": geometry})
                c.click("scene-menu/0")
                c.wait(lambda: c.node(prefix + "lighting-label").get("text") == "Lit", "scene lighting caption")
                c.wait(lambda: not c.node(prefix + "information"), "scene lighting ready")
                continue
            c.event(window=viewport["window"], kind="key", key="escape")
            c.wait(lambda: not c.node("scene-menu/0"), "dismiss " + control)
        c.click(prefix + "grid")
        c.click(prefix + "grid")

    # The caller floated the active scene. Verify menu placement before and after
    # a live resize, while the same native popup remains open.
    floating = c.node(prefix + "viewport")["window"]
    c.click(prefix + "projection")
    c.wait(lambda: c.node("scene-menu/0"), "projection before resize")
    c.completed(c.operation("app.resize", window=floating, width=322, height=640, dpr=2))
    c.wait(lambda: c.node(prefix + "viewport") and
           abs(c.node(prefix + "viewport")["rect"][2] - c.node(prefix + "viewport")["rect"][0] - 320) <= 1,
           "320px floating Scene View")
    c.frame(floating)
    geometry = anchored_menu(c, prefix, "projection")
    frames.append({"case": "floating-live-resize-menu", "frame": c.frame(floating), "menu_geometry": geometry})
    c.event(window=floating, kind="key", key="escape")
    c.wait(lambda: not c.node(prefix + "information"), "resized floating scene ready")
    menus("floating-320")
    c.menu(view, "Redock to main window", retired=True)
    c.wait(lambda: c.node(prefix + "viewport") and c.node(prefix + "viewport")["window"] == c.window(), "redocked narrow scenario")
    c.click(c.label(view))
    for width in (240, 280, 320):
        group = c.node(c.group_id(c.group(view)))["rect"]
        dividers = [n for n in c.observe()["elements"] if n["visible"] and
                    n["id"].endswith("/divider") and n["rect"][2] - n["rect"][0] <= 8]
        divider = min(dividers, key=lambda n: abs(n["rect"][0] - group[2]))
        r = divider["rect"]
        assert abs(r[0] - group[2]) <= 8, "Expected a divider at Scene View's right edge"
        point = [(r[0] + r[2]) / 2, (r[1] + r[3]) / 2]
        c.event(kind="move", position=point)
        c.event(kind="button", pressed=True)
        c.event(kind="move", position=[group[0] + width + (r[2] - r[0]) / 2, point[1]])
        c.event(kind="button", pressed=False)
        c.wait(lambda: abs(c.node(prefix + "viewport")["rect"][2] - c.node(prefix + "viewport")["rect"][0] - width) <= 1,
               f"{width}px docked Scene View")
        c.wait(lambda: not c.node(prefix + "information"), "narrow docked frame ready")
        menus(f"redocked-{width}")


def run(c, project, state_directory):
    assert Path(c.rpc("editor.state")["preferences"]).parent == state_directory.resolve()
    before = inventory(project / "assets")
    view = next(i for i, v in c.state()["layout"]["views"].items()
                if v.get("restore_key") == "Composed.scene.ron")
    prefix = f"panel/ViewId({view})/"
    c.click("workspace/reset")
    c.click(c.label(view))
    c.frame(c.window())
    c.wait(lambda: c.node(prefix + "render") and not c.node(prefix + "information"), "ready scene")
    window = c.node(prefix + "render")["window"]
    frames = []

    def pose():
        return c.state()["scene_views"][view]

    def selection():
        docs = c.state()["documents"]
        return next(d["selection"] for d in docs["documents"] if d["document"] == docs["active"])

    def center():
        rect = c.node(prefix + "render")["rect"]
        return [(rect[0]+rect[2])/2, (rect[1]+rect[3])/2]

    def command(caption, control="views"):
        c.click(prefix + control)
        item = c.wait(lambda: next((n["id"].removesuffix("/caption") for n in c.observe()["elements"]
                                   if n["visible"] and n["id"].startswith("scene-menu/") and n.get("text") == caption), None), caption)
        c.click(item)

    def record(case):
        frames.append({"case": case, "frame": c.frame(window), "pose": pose(), "selection": selection()})

    original = pose()
    root = selection()
    point = center()
    c.event(window=window, kind="click", position=point)
    selected = c.wait(lambda: selection() if selection() != root else None, "nearest spatial selection")
    assert selected["instances"], "Fixture cube must preserve its nested prefab address"
    record("nested-selection")

    hierarchy = next(i for i, v in c.state()["layout"]["views"].items() if v["kind"] == "hierarchy")
    label = c.wait(lambda: next((n for n in c.observe()["elements"] if n["visible"] and n["id"].startswith(f"hierarchy/ViewId({hierarchy})/")
                                and n.get("text") == "Warm sphere"), None), "Hierarchy sphere")
    row = next(n for n in c.observe()["elements"] if n["entity"] == label["parent"])
    unchanged = pose()
    c.click(row["id"])
    c.wait(lambda: selection() != selected, "Hierarchy selection publishes to the shared coordinator")
    assert pose() == unchanged, "Hierarchy selection moved the camera without Frame"
    record("hierarchy-selection")
    c.event(window=window, kind="click", position=point)
    c.wait(lambda: selection() == selected, "viewport restores nested selection")

    # Alt+LMB navigation must never publish another selection.
    c.event(window=window, kind="modifier", modifier="alt", pressed=True)
    c.event(window=window, kind="move", position=point)
    c.event(window=window, kind="button", pressed=True)
    c.event(window=window, kind="move", position=[point[0]+35, point[1]+20])
    c.event(window=window, kind="button", pressed=False)
    c.event(window=window, kind="modifier", modifier="alt", pressed=False)
    c.wait(lambda: pose()["rotation"] != original["rotation"], "native orbit")
    assert selection() == selected
    record("orbit")

    point = center()
    c.event(window=window, kind="modifier", modifier="alt", pressed=True)
    c.event(window=window, kind="move", position=point)
    c.event(window=window, kind="button", pressed=True)
    c.wait(lambda: c.state()["navigation_input"]["capture"], "orbit lease")
    c.event(window=window, kind="key", key="escape")
    c.wait(lambda: c.state()["navigation_input"]["capture"] is None, "Escape retires orbit lease")
    stopped = pose()
    c.event(window=window, kind="move", position=[point[0]+12, point[1]+12])
    c.event(window=window, kind="button", pressed=False)
    c.event(window=window, kind="modifier", modifier="alt", pressed=False)
    assert pose() == stopped, "Cancelled navigation kept moving"
    assert selection() == selected, "Cancelled navigation selected a surface"
    record("escape-cancel")

    old = pose()
    c.event(window=window, kind="wheel", delta=[0, 80])
    c.wait(lambda: pose()["translation"] != old["translation"], "native dolly")
    record("dolly")

    command("Orthographic", "projection")
    c.wait(lambda: pose()["orthographic"], "orthographic projection")
    command("Frame selection")
    record("orthographic-frame-selection")
    for name in ("Front", "Back", "Left", "Right", "Top", "Bottom"):
        command(name)
        record("view-" + name.lower())
    c.click(prefix + "grid")
    command("Editor lighting", "lighting")
    c.wait(lambda: not c.node(prefix + "information"), "editor lighting frame")
    record("inspection-no-grid")
    c.click(prefix + "grid")
    command("Scene lighting", "lighting")
    c.wait(lambda: not c.node(prefix + "information"), "scene lighting frame")
    command("Perspective", "projection")
    command("Front")
    command("Frame all")

    unchanged = pose()
    c.click(prefix + "views")
    c.event(window=window, kind="move", position=center())
    c.event(window=window, kind="wheel", delta=[0, 80])
    assert pose() == unchanged, "Open menu did not block viewport wheel"
    assert not any(n["visible"] and n.get("text")=="Dismiss" for n in c.observe()["elements"])
    c.event(window=window,kind="click",position=center())
    c.wait(lambda:not any(n["visible"] and n["id"].startswith("scene-menu/") for n in c.observe()["elements"]),"outside click dismisses viewport menu")
    assets = next(i for i, v in c.state()["layout"]["views"].items() if v["kind"] == "assets")
    c.click(c.label(assets))
    search = next(n["id"] for n in c.observe()["elements"] if n["visible"] and n["id"].startswith("browser/") and n["id"].endswith("/search"))
    c.click(search)
    c.event(window=window, kind="key", key="select_all")
    c.event(window=window, kind="text", text="WASD f")
    c.event(window=window, kind="move", position=center())
    c.event(window=window, kind="wheel", delta=[0, 80])
    assert c.node(search)["value"] == "WASD f" and pose() == unchanged, "Hover stole field input ownership"
    c.click(search)
    c.event(window=window, kind="key", key="select_all")
    c.event(window=window, kind="key", key="backspace")
    record("menu-and-field-precedence")
    rect = c.node(prefix + "render")["rect"]
    c.event(window=window, kind="click", position=[rect[2]-8, rect[1]+8])
    c.wait(lambda: selection() is None, "empty-space deselection")
    record("empty-selection")

    retained = pose()
    other = next(i for i, v in c.state()["layout"]["views"].items() if v.get("restore_key") == "ship.glb")
    c.click(c.label(other))
    c.wait(lambda: c.state()["documents"]["active"] is None, "asset tab has no authoring authority")
    c.click(c.label(view))
    c.wait(lambda: not c.node(prefix + "information"), "reactivated scene frame")
    assert pose() == retained, "Tab switch changed the retained camera"
    c.menu(view, "Float active tab")
    c.wait(lambda: c.node(prefix + "render") and c.node(prefix + "render")["window"] != window, "floating viewport")
    c.wait(lambda: not c.node(prefix + "information"), "floating frame")
    assert pose() == retained, "Host transfer changed the camera"
    narrow_hosts(c, view, prefix, frames)
    c.wait(lambda: c.node(prefix + "render") and c.node(prefix + "render")["window"] == window, "redocked viewport")
    c.wait(lambda: not c.node(prefix + "information"), "redocked frame")
    assert pose() == retained
    assert before == inventory(project / "assets"), "Navigation wrote source files"
    record("retained-redock")
    return {"status": "passed", "frames": frames,
            "limitations": ["injected Bevy input, not physical OS input", "RMB flight and MMB pan need physical or dedicated native input acceptance"]}


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("credentials", type=Path)
    p.add_argument("--project", type=Path, required=True)
    p.add_argument("--state-directory", type=Path, required=True)
    p.add_argument("--report", type=Path, required=True)
    args = p.parse_args()
    report = run(Docking(args.credentials), args.project, args.state_directory)
    args.report.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(f"PASS: native Scene View navigation; {args.report}")
