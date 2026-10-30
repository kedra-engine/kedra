//! Actual pointer/keyboard scene navigation, selection precedence and host transfer.
use super::{navigation_geometry as geometry, scene, support::*};
use crate::project::inventory;
use kedra_test::Result;
use serde_json::{Value, json};
use std::path::Path;

fn pose(c: &Editor<'_>, view: &str) -> Result<Value> {
    Ok(c.state()?["scene_views"][view].clone())
}
fn command(c: &Editor<'_>, p: &str, key: &str, caption: &str) -> Result<()> {
    c.click(&format!("{p}{key}"))?;
    let n = c.find(caption, |n| {
        n.id.starts_with("scene-menu/") && n.caption() == caption
    })?;
    c.click(n.id.strip_suffix("/caption").unwrap_or(&n.id))
}
fn record(c: &Editor<'_>, view: &str, p: &str, name: &str, frames: &mut Vec<Value>) -> Result<()> {
    let mut frame = scene::record(c, name, p)?;
    frame["pose"] = pose(c, view)?;
    frame["selection"] = c.selection()?;
    frames.push(frame);
    Ok(())
}

pub(super) fn run(c: &Editor<'_>, project: &Path, view: &str, other: &str) -> Result<Value> {
    let before = inventory(&project.join("assets"))?;
    let p = format!("panel/ViewId({view})/");
    c.click("workspace/reset")?;
    c.click(&Editor::label(view))?;
    scene::ready(c, &p)?;
    let window = c.node(&format!("{p}render"))?.window()?;
    let mut frames = Vec::new();
    let original = pose(c, view)?;
    let root = c.selection()?;
    let point = c.node(&format!("{p}render"))?.center();
    c.input(window, json!({"kind":"click","position":point}))?;
    c.until("nearest nested selection", || Ok(c.selection()? != root))?;
    let selected = c.selection()?;
    check(
        !array(&selected["instances"])?.is_empty(),
        "Nested spatial selection lost prefab address",
    )?;
    record(c, view, &p, "nested-selection", &mut frames)?;
    let hierarchy = c.view("hierarchy")?;
    let label = c.find("Hierarchy sphere", |n| {
        n.id.starts_with(&format!("hierarchy/ViewId({hierarchy})/")) && n.caption() == "Warm sphere"
    })?;
    let row = c
        .nodes()?
        .into_iter()
        .find(|n| Some(n.entity) == label.parent)
        .ok_or("Missing sphere hierarchy row")?;
    let unchanged = pose(c, view)?;
    c.click(&row.id)?;
    c.until("Hierarchy selection coordinator", || {
        Ok(c.selection()? != selected)
    })?;
    check(
        pose(c, view)? == unchanged,
        "Hierarchy selection moved camera without Frame",
    )?;
    record(c, view, &p, "hierarchy-selection", &mut frames)?;
    c.input(window, json!({"kind":"click","position":point}))?;
    c.until("viewport nested selection", || {
        Ok(c.selection()? == selected)
    })?;
    c.modifier("alt", true)?;
    c.move_to(point)?;
    c.held(true)?;
    c.move_to([point[0] + 35., point[1] + 20.])?;
    c.held(false)?;
    c.modifier("alt", false)?;
    c.until("native orbit", || {
        Ok(pose(c, view)?["rotation"] != original["rotation"])
    })?;
    check(
        c.selection()? == selected,
        "Orbit selected through viewport",
    )?;
    record(c, view, &p, "orbit", &mut frames)?;
    let point = c.node(&format!("{p}render"))?.center();
    c.modifier("alt", true)?;
    c.move_to(point)?;
    c.held(true)?;
    c.until("orbit lease", || {
        Ok(!c.state()?["navigation_input"]["capture"].is_null())
    })?;
    c.key("escape")?;
    c.until("cancelled orbit lease", || {
        Ok(c.state()?["navigation_input"]["capture"].is_null())
    })?;
    let stopped = pose(c, view)?;
    c.move_to([point[0] + 12., point[1] + 12.])?;
    c.held(false)?;
    c.modifier("alt", false)?;
    check(
        pose(c, view)? == stopped && c.selection()? == selected,
        "Cancelled orbit kept moving or selected surface",
    )?;
    record(c, view, &p, "escape-cancel", &mut frames)?;
    let old = pose(c, view)?;
    c.input(window, json!({"kind":"wheel","delta":[0,80]}))?;
    c.until("native dolly", || {
        Ok(pose(c, view)?["translation"] != old["translation"])
    })?;
    record(c, view, &p, "dolly", &mut frames)?;
    c.click(&format!("{p}projection"))?;
    c.until("orthographic projection", || {
        Ok(pose(c, view)?["orthographic"] == true)
    })?;
    c.click(&format!("{p}frame"))?;
    record(c, view, &p, "orthographic-frame-selection", &mut frames)?;
    command(c, &p, "settings", "Toggle grid")?;
    command(c, &p, "settings", "Editor lighting")?;
    scene::ready(c, &p)?;
    record(c, view, &p, "inspection-no-grid", &mut frames)?;
    command(c, &p, "settings", "Toggle grid")?;
    command(c, &p, "settings", "Scene lighting")?;
    scene::ready(c, &p)?;
    c.click(&format!("{p}projection"))?;
    front(c, view, &p)?;
    command(c, &p, "settings", "Frame all")?;
    geometry::controls(c, &p)?;
    controls(c, view, &p, &mut frames)?;
    front(c, view, &p)?;
    command(c, &p, "settings", "Frame all")?;
    let unchanged = pose(c, view)?;
    c.click(&format!("{p}settings"))?;
    let point = c.node(&format!("{p}render"))?.center();
    c.move_to(point)?;
    c.input(window, json!({"kind":"wheel","delta":[0,80]}))?;
    check(
        pose(c, view)? == unchanged,
        "Menu failed to block viewport wheel",
    )?;
    check(
        !c.nodes()?.iter().any(|n| n.caption() == "Dismiss"),
        "Scene menu exposed fake dismiss control",
    )?;
    c.input(window, json!({"kind":"click","position":point}))?;
    c.until("outside menu dismissal", || {
        Ok(!c.nodes()?.iter().any(|n| n.id.starts_with("scene-menu/")))
    })?;
    c.click(&Editor::label(&c.view("assets")?))?;
    let search = c.find("browser search", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/search")
    })?;
    c.field(&search.id, "WASD f")?;
    let before_hover = pose(c, view)?;
    c.move_to(point)?;
    c.input(window, json!({"kind":"wheel","delta":[0,80]}))?;
    c.until("hover dolly with search focus", || {
        Ok(pose(c, view)?["translation"] != before_hover["translation"])
    })?;
    check(
        c.node(&search.id)?.value() == "WASD f",
        "Viewport hover stole field input",
    )?;
    c.key("select_all")?;
    c.key("backspace")?;
    check(
        c.node(&search.id)?.value().is_empty(),
        "Hover wheel moved keyboard focus out of search",
    )?;
    front(c, view, &p)?;
    command(c, &p, "settings", "Frame all")?;
    record(c, view, &p, "menu-and-field-precedence", &mut frames)?;
    let rect = c.node(&format!("{p}render"))?.rect;
    c.input(
        window,
        json!({"kind":"click","position":[rect[0]+60.,rect[3]-12.]}),
    )?;
    c.until("empty-space deselection", || Ok(c.selection()?.is_null()))?;
    record(c, view, &p, "empty-selection", &mut frames)?;
    let retained = pose(c, view)?;
    c.click(&Editor::label(other))?;
    c.until("raw asset authority", || {
        Ok(c.state()?["documents"]["active"].is_null())
    })?;
    c.click(&Editor::label(view))?;
    scene::ready(c, &p)?;
    check(pose(c, view)? == retained, "Tab switch changed camera")?;
    c.menu(view, "Float tab")?;
    c.until("floating viewport", || {
        Ok(c.node(&format!("{p}render"))?.window()? != window)
    })?;
    scene::ready(c, &p)?;
    check(pose(c, view)? == retained, "Float changed camera")?;
    narrow(c, view, other, &p, &mut frames)?;
    scene::ready(c, &p)?;
    check(
        c.node(&format!("{p}render"))?.window()? == window && pose(c, view)? == retained,
        "Redock changed target or retained camera",
    )?;
    check(
        inventory(&project.join("assets"))? == before,
        "Navigation changed source bytes/sidecars",
    )?;
    record(c, view, &p, "retained-redock", &mut frames)?;
    Ok(
        json!({"status":"passed","frames":frames,"limitations":["Injected input, not physical OS input","RMB flight and MMB pan require dedicated acceptance"]}),
    )
}

fn front(c: &Editor<'_>, view: &str, p: &str) -> Result<()> {
    // Move off Z first: clicking an already aligned axis selects its opposite.
    for axis in ["x", "z"] {
        c.click(&format!("{p}axis-{axis}"))?;
        c.until("gizmo alignment settled", || {
            Ok(pose(c, view)?["aligning"] == false)
        })?;
    }
    Ok(())
}

fn controls(c: &Editor<'_>, view: &str, p: &str, frames: &mut Vec<Value>) -> Result<()> {
    let window = c.node(&format!("{p}viewport"))?.window()?;
    let o = c.client.observe()?;
    check(
        array(&o["windows"])?
            .iter()
            .any(|w| w["id"] == window && w["focused"] == true),
        "Camera test requires actual foreground native window",
    )?;
    let selection = c.selection()?;
    for (axis, expected) in [
        ("x", [1., 0., 0.]),
        ("y", [0., 1., 0.]),
        ("z", [0., 0., 1.]),
    ] {
        for side in [1., -1.] {
            c.click(&format!("{p}axis-{axis}"))?;
            c.until("signed camera alignment", || {
                let pose = pose(c, view)?;
                let a: [f64; 3] = serde_json::from_value(pose["translation"].clone())?;
                let b: [f64; 3] = serde_json::from_value(pose["pivot"].clone())?;
                let direction = std::array::from_fn::<_, 3, _>(|i| a[i] - b[i]);
                let length = direction.iter().map(|x| x * x).sum::<f64>().sqrt();
                Ok(pose["aligning"] == false
                    && length > 0.
                    && direction
                        .iter()
                        .zip(expected)
                        .all(|(a, b)| (a / length - side * b).abs() < 0.002))
            })?;
            check(
                c.selection()? == selection,
                "Axis click selected through overlay",
            )?;
            record(c, view, p, &format!("axis-{axis}-{side}"), frames)?;
        }
    }
    {
        let (name, changed) = ("orbit", "rotation");
        let point = c.node(&format!("{p}{name}"))?.center();
        let before = pose(c, view)?;
        c.input(window, json!({"kind":"move","position":point}))?;
        c.input(window, json!({"kind":"button","pressed":true}))?;
        c.until("navigation lease", || {
            Ok(!c.state()?["navigation_input"]["capture"].is_null())
        })?;
        c.input(
            window,
            json!({"kind":"move","position":[point[0]+18.,point[1]+12.]}),
        )?;
        c.until("navigation control changes camera", || {
            Ok(pose(c, view)?[changed] != before[changed])
        })?;
        record(c, view, p, &format!("{name}-captured"), frames)?;
        c.release(window)?;
        c.until("released navigation lease", || {
            Ok(c.state()?["navigation_input"]["capture"].is_null())
        })?;
        check(
            c.selection()? == selection,
            "Navigation selected through overlay",
        )?;
        let stopped = pose(c, view)?;
        c.input(
            window,
            json!({"kind":"move","position":[point[0]+24.,point[1]+16.]}),
        )?;
        check(
            pose(c, view)? == stopped,
            "Navigation continued after release",
        )?;
        let point = c.node(&format!("{p}{name}"))?.center();
        c.input(window, json!({"kind":"move","position":point}))?;
        c.input(window, json!({"kind":"button","pressed":true}))?;
        c.until("second navigation lease", || {
            Ok(!c.state()?["navigation_input"]["capture"].is_null())
        })?;
        c.input(window, json!({"kind":"key","key":"escape"}))?;
        c.until("navigation Escape", || {
            Ok(c.state()?["navigation_input"]["capture"].is_null())
        })?;
        let stopped = pose(c, view)?;
        c.input(
            window,
            json!({"kind":"move","position":[point[0]+12.,point[1]+12.]}),
        )?;
        c.release(window)?;
        check(
            pose(c, view)? == stopped && c.selection()? == selection,
            "Navigation continued after Escape or changed selection",
        )?;
    }
    let before = pose(c, view)?;
    c.click(&format!("{p}settings"))?;
    c.click("scene-menu/2")?;
    c.until(
        "hidden overlays",
        || Ok(pose(c, view)?["overlays"] == false),
    )?;
    check(
        c.selection()? == selection,
        "Overlay toggle changed selection",
    )?;
    record(c, view, p, "overlays-hidden", frames)?;
    c.click(&format!("{p}settings"))?;
    c.click("scene-menu/2")?;
    c.until("restored overlays", || {
        Ok(pose(c, view)?["overlays"] == true)
    })?;
    check(
        pose(c, view)? == before && c.selection()? == selection,
        "Overlay restore changed camera or selection",
    )?;
    c.click(&format!("{p}projection"))?;
    c.until("projection toggle", || {
        Ok(pose(c, view)?["orthographic"] != before["orthographic"])
    })?;
    check(
        c.selection()? == selection,
        "Projection selected through overlay",
    )?;
    c.click(&format!("{p}projection"))?;
    c.until("restored projection", || {
        Ok(pose(c, view)?["orthographic"] == before["orthographic"])
    })?;
    check(
        c.selection()? == selection,
        "Projection restore changed selection",
    )?;
    record(c, view, p, "navigation-controls-complete", frames)
}

fn narrow(c: &Editor<'_>, view: &str, other: &str, p: &str, frames: &mut Vec<Value>) -> Result<()> {
    let floating = c.node(&format!("{p}viewport"))?.window()?;
    c.resize(floating, 480, 430, 2)?;
    scene::ready(c, p)?;
    geometry::controls(c, p)?;
    record(c, view, p, "floating-480x360", frames)?;
    let point = c.node(&format!("{p}orbit"))?.center();
    // Integrated headers removed the duplicate tab row: a 320px native window
    // no longer yields a <230px canvas. Keep the native size guard and create
    // a genuinely short canvas through a real cross-host vertical split.
    c.resize(floating, 480, 394, 1)?;
    let label = c.node(&Editor::label(other))?;
    let start = label.center();
    c.drag(other, [start[0] + 12., start[1] + 12.])?;
    let target = c.node(&Editor::group_id(&c.group(view)?))?;
    c.input(
        floating,
        json!({"kind":"move","position":[target.center()[0],target.rect[3]-12.]}),
    )?;
    c.preview()?;
    c.release(floating)?;
    c.until("short split Scene View", || {
        let n = c.node(&format!("{p}viewport"))?;
        Ok(n.rect[3] - n.rect[1] < 230. && c.group(view)? != c.group(other)?)
    })?;
    c.activate(floating)?;
    c.click(&Editor::label(view))?;
    scene::ready(c, p)?;
    geometry::controls(c, p)?;
    let before = pose(c, view)?;
    c.input(floating, json!({"kind":"move","position":point}))?;
    c.input(floating, json!({"kind":"button","pressed":true}))?;
    check(
        c.state()?["navigation_input"]["capture"].is_null(),
        "Hidden orbit captured input",
    )?;
    c.input(
        floating,
        json!({"kind":"move","position":[point[0]+15.,point[1]+8.]}),
    )?;
    c.release(floating)?;
    check(pose(c, view)? == before, "Hidden orbit moved camera")?;
    record(c, view, p, "floating-short-dpr1", frames)?;
    c.menu(other, "Redock to main window")?;
    c.until("temporary split retired", || {
        Ok(c.node(&Editor::label(other))?.window()? == c.window()?)
    })?;
    c.activate(floating)?;
    c.click(&Editor::label(view))?;
    c.resize(floating, 480, 430, 2)?;
    scene::ready(c, p)?;
    geometry::controls(c, p)?;
    c.click(&format!("{p}settings"))?;
    c.wait_node("scene-menu/0")?;
    c.resize(floating, 320, 640, 2)?;
    c.until("320px floating scene", || {
        let n = c.node(&format!("{p}viewport"))?;
        Ok((n.rect[2] - n.rect[0] - 320.).abs() <= 1.)
    })?;
    c.frame(floating, "live-resize-menu")?;
    let menu = geometry::menu(c, p, "settings")?;
    frames.push(json!({"case":"floating-live-resize-menu","frame":c.frame(floating,"menu after live resize")?,"menu_geometry":menu}));
    c.input(floating, json!({"kind":"key","key":"escape"}))?;
    scene::ready(c, p)?;
    menus(c, view, p, "floating-320", frames)?;
    c.menu(view, "Redock to main window")?;
    c.until("redocked viewport", || {
        Ok(c.node(&format!("{p}viewport"))?.window()? == c.window()?)
    })?;
    c.click(&Editor::label(view))?;
    for width in [240., 280., 320.] {
        geometry::resize(c, view, width)?;
        c.until("narrow docked scene", || {
            let n = c.node(&format!("{p}viewport"))?;
            Ok((n.rect[2] - n.rect[0] - width).abs() <= 1.)
        })?;
        scene::ready(c, p)?;
        menus(c, view, p, &format!("redocked-{width}"), frames)?;
    }
    Ok(())
}
fn menus(c: &Editor<'_>, view: &str, p: &str, case: &str, frames: &mut Vec<Value>) -> Result<()> {
    let viewport = geometry::controls(c, p)?;
    {
        let control = "settings";
        c.click(&format!("{p}{control}"))?;
        c.wait_node("scene-menu/0")?;
        let m = geometry::menu(c, p, control)?;
        frames.push(json!({"case":format!("{case}-{control}"),"frame":c.frame(viewport.window()?,control)?,"menu_geometry":m}));
        {
            c.click("scene-menu/5")?;
            c.until("editor lighting", || {
                Ok(pose(c, view)?["editor_lighting"] == true)
            })?;
            scene::ready(c, p)?;
            geometry::controls(c, p)?;
            c.click(&format!("{p}settings"))?;
            c.wait_node("scene-menu/0")?;
            let m = geometry::menu(c, p, "settings")?;
            frames.push(json!({"case":format!("{case}-editor-lighting"),"frame":c.frame(viewport.window()?,"editor-lighting menu")?,"menu_geometry":m}));
            c.click("scene-menu/4")?;
            c.until("scene lighting", || {
                Ok(pose(c, view)?["editor_lighting"] == false)
            })?;
            scene::ready(c, p)?;
        }
    }
    command(c, p, "settings", "Toggle grid")?;
    command(c, p, "settings", "Toggle grid")?;
    Ok(())
}
