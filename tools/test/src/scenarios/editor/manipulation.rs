//! Real pointer transforms, cancellation and source-history publication.
use super::support::*;
use kedra_test::Result;
use serde_json::{Value, json};

fn state(c: &Editor<'_>) -> Result<Value> {
    Ok(c.state()?["navigation_input"]["manipulator"].clone())
}
fn selected_transform(c: &Editor<'_>) -> Result<Value> {
    let doc = c.current_document()?;
    let state = c.state()?;
    let nodes = array(&state["scene_views"][text(&doc["view"])?]["source_transforms"])?;
    Ok(nodes
        .iter()
        .find(|n| n["node"] == doc["selection"])
        .ok_or("Selected spatial projection missing")?
        .clone())
}
fn navigation_gestures(c: &Editor<'_>, prefix: &str, window: u64) -> Result<()> {
    let source = selected_transform(c)?;
    let selection = c.selection()?;
    c.click(&format!("{prefix}hand"))?;
    c.until("hand disables transform handles", || {
        let state = state(c)?;
        Ok(state["enabled"] == false && state["active"] == false && state["handles"].is_null())
    })?;
    check(
        selected_transform(c)? == source
            && c.selection()? == selection
            && c.current_document()?["dirty"] == false,
        "Hand changed the selected source instead of disabling transforms",
    )?;
    let document = c.current_document()?;
    let view = text(&document["view"])?;
    for name in ["pan", "zoom"] {
        let viewport = c.node(&format!("{prefix}viewport"))?;
        let before = c.state()?["scene_views"][view].clone();
        let source = selected_transform(c)?;
        let from = viewport.center();
        if name == "pan" {
            c.input(window, json!({"kind":"move","position":from}))?;
            c.input(
                window,
                json!({"kind":"button","button":"middle","pressed":true}),
            )?;
            c.until("middle-button navigation acquired", || {
                Ok(!c.state()?["navigation_input"]["capture"].is_null())
            })?;
            c.input(
                window,
                json!({"kind":"move","position":[from[0]+20.,from[1]+20.]}),
            )?;
            c.input(
                window,
                json!({"kind":"button","button":"middle","pressed":false}),
            )?;
        } else {
            c.input(window, json!({"kind":"move","position":from}))?;
            c.input(window, json!({"kind":"wheel","delta":[0,80]}))?;
        }
        c.until("navigation gesture moves camera", || {
            let after = c.state()?["scene_views"][view].clone();
            Ok(after["translation"] != before["translation"] || after["pivot"] != before["pivot"])
        })?;
        check(
            c.state()?["navigation_input"]["capture"].is_null(),
            "Navigation release retained capture",
        )?;
        check(
            selected_transform(c)? == source && c.current_document()?["dirty"] == false,
            "Navigation gesture edited the selected object",
        )?;
        c.frame(window, &format!("{name} native gesture"))?;
    }
    Ok(())
}
fn tool_drag(c: &Editor<'_>, window: u64, mode: &str, uniform: bool) -> Result<()> {
    let data = state(c)?;
    let handle = array(&data["handles"])?
        .first()
        .ok_or("No transform handle")?;
    let point = |v: &Value| -> Result<[f64; 2]> { Ok([number(&v[0])?, number(&v[1])?]) };
    let (start, end) = if uniform {
        let start = point(&handle["center"])?;
        (start, [start[0] + 38., start[1]])
    } else if mode == "rotate" {
        (point(&handle["points"][8])?, point(&handle["points"][20])?)
    } else {
        let a = point(&handle["start"])?;
        let b = point(&handle["end"])?;
        ([a[0] * 0.4 + b[0] * 0.6, a[1] * 0.4 + b[1] * 0.6], b)
    };
    c.input(window, json!({"kind":"move","position":start}))?;
    c.input(window, json!({"kind":"button","pressed":true}))?;
    c.until("transform acquired", || Ok(state(c)?["active"] == true))?;
    c.input(window, json!({"kind":"move","position":end}))?;
    c.frame(window, &format!("{mode} preview, uniform={uniform}"))?;
    Ok(())
}
fn tools(c: &Editor<'_>, prefix: &str, window: u64, source: &std::path::Path) -> Result<()> {
    for (mode, uniform, field) in [
        ("rotate", false, "rotation"),
        ("scale", false, "scale"),
        ("scale", true, "scale"),
    ] {
        c.click(&format!("{prefix}{mode}"))?;
        c.until("tool selected", || {
            Ok(state(c)?["mode"] == mode
                && state(c)?["handles"]
                    .as_array()
                    .is_some_and(|v| !v.is_empty()))
        })?;
        let before = selected_transform(c)?;
        let bytes = std::fs::read(source)?;
        tool_drag(c, window, mode, uniform)?;
        check(
            selected_transform(c)?[field] != before[field],
            "Transform preview did not change its field",
        )?;
        check(
            c.current_document()?["dirty"] == false,
            "Transform preview published early",
        )?;
        c.input(window, json!({"kind":"key","key":"escape"}))?;
        c.input(window, json!({"kind":"button","pressed":false}))?;
        c.until("cancel restores projection", || {
            Ok(selected_transform(c)? == before && state(c)?["active"] == false)
        })?;
        check(
            c.current_document()?["dirty"] == false && std::fs::read(source)? == bytes,
            "Cancel changed source",
        )?;
        tool_drag(c, window, mode, uniform)?;
        c.input(window, json!({"kind":"button","pressed":false}))?;
        c.until("transform committed", || {
            Ok(c.current_document()?["dirty"] == true)
        })?;
        let changed = selected_transform(c)?;
        check(
            changed[field] != before[field],
            "Transform release lost preview",
        )?;
        for other in ["translation", "rotation", "scale"] {
            if other != field {
                check(
                    changed[other] == before[other],
                    "Transform altered another field",
                )?;
            }
        }
        if uniform {
            let ratio = number(&changed["scale"][0])? / number(&before["scale"][0])?;
            for axis in 1..3 {
                near(
                    number(&changed["scale"][axis])? / number(&before["scale"][axis])?,
                    ratio,
                    1e-5,
                )?;
            }
        }
        c.input(window, json!({"kind":"key","key":"undo"}))?;
        c.until("single Undo restores transform", || {
            Ok(c.current_document()?["dirty"] == false && selected_transform(c)? == before)
        })?;
        c.input(window, json!({"kind":"key","key":"redo"}))?;
        c.until("Redo restores transform", || {
            Ok(c.current_document()?["dirty"] == true && selected_transform(c)? == changed)
        })?;
        check(
            std::fs::read(source)? == bytes,
            "Transform history wrote before Save",
        )?;
        c.input(window, json!({"kind":"key","key":"save"}))?;
        c.until("transform saved", || {
            Ok(c.current_document()?["dirty"] == false)
        })?;
        check(
            std::fs::read(source)? != bytes,
            "Transform Save omitted edit",
        )?;
    }
    Ok(())
}
fn drag(c: &Editor<'_>, window: u64) -> Result<()> {
    let handles = state(c)?;
    let handle = array(&handles["handles"])?
        .first()
        .ok_or("No Move handle")?;
    let start = [number(&handle["start"][0])?, number(&handle["start"][1])?];
    let end = [number(&handle["end"][0])?, number(&handle["end"][1])?];
    let point = [start[0] * 0.4 + end[0] * 0.6, start[1] * 0.4 + end[1] * 0.6];
    c.input(window, json!({"kind":"move","position":point}))?;
    c.input(window, json!({"kind":"button","pressed":true}))?;
    c.until("Move acquired", || Ok(state(c)?["active"] == true))?;
    c.input(window, json!({"kind":"move","position":[point[0]+(end[0]-start[0])*0.6,point[1]+(end[1]-start[1])*0.6]}))?;
    c.frame(window, "Move preview")?;
    Ok(())
}
pub(super) fn run(c: &Editor<'_>) -> Result<Value> {
    let project = c.open_project()?;
    let search = c.find("Assets search", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/search")
    })?;
    c.field(&search.id, "Cuboid.prefab.ron")?;
    c.open_asset("Cuboid.prefab.ron")?;
    c.until("source admitted", || Ok(c.current_document().is_ok()))?;
    let document = c.current_document()?;
    let view = text(&document["view"])?;
    let prefix = format!("panel/ViewId({view})/");
    super::scene::ready(c, &prefix)?;
    let row = c.find("Cuboid hierarchy root", |n| {
        n.id.starts_with("hierarchy/")
            && n.id.ends_with("/name")
            && n.caption() == "Textured cuboid"
    })?;
    c.click(&row.id)?;
    c.click(&format!("{prefix}frame"))?;
    c.click(&format!("{prefix}move"))?;
    c.until("Move handles", || {
        Ok(state(c)?["handles"]
            .as_array()
            .is_some_and(|v| !v.is_empty()))
    })?;
    let window = c.node(&format!("{prefix}viewport"))?.window()?;
    let path = project.join("assets/Cuboid.prefab.ron");
    let before = std::fs::read(&path)?;
    drag(c, window)?;
    check(
        c.current_document()?["dirty"] == false,
        "Preview published a source edit",
    )?;
    c.key("escape")?;
    c.input(window, json!({"kind":"button","pressed":false}))?;
    check(
        state(c)?["active"] == false && c.current_document()?["dirty"] == false,
        "Cancel retained gesture or source edit",
    )?;
    check(
        std::fs::read(&path)? == before,
        "Preview/cancel wrote source",
    )?;
    drag(c, window)?;
    c.input(window, json!({"kind":"button","pressed":false}))?;
    c.until("Move committed", || {
        Ok(c.current_document()?["dirty"] == true)
    })?;
    check(std::fs::read(&path)? == before, "Move wrote before Save")?;
    c.hierarchy_menu(None, "Undo")?;
    c.until("single Undo restores source", || {
        Ok(c.current_document()?["dirty"] == false)
    })?;
    c.hierarchy_menu(None, "Redo")?;
    c.until("Move redone", || Ok(c.current_document()?["dirty"] == true))?;
    c.key("save")?;
    c.until("Move saved", || Ok(c.current_document()?["dirty"] == false))?;
    check(std::fs::read(&path)? != before, "Save omitted Move")?;
    c.frame(window, "Move saved")?;
    tools(c, &prefix, window, &path)?;
    navigation_gestures(c, &prefix, window)?;
    let prefab_saved = std::fs::read(&path)?;
    c.field(&search.id, "Composed.scene.ron")?;
    c.open_asset("Composed.scene.ron")?;
    c.until("composed source admitted", || {
        Ok(c.current_document()?["path"] == "Composed.scene.ron")
    })?;
    let document = c.current_document()?;
    let view = text(&document["view"])?;
    let prefix = format!("panel/ViewId({view})/");
    super::scene::ready(c, &prefix)?;
    let row = c.find("Nested cuboid", |n| {
        n.id.starts_with("hierarchy/")
            && n.id.ends_with("/name")
            && n.caption() == "Textured cuboid"
    })?;
    c.click(&row.id)?;
    check(
        !array(&c.current_document()?["selection"]["instances"])?.is_empty(),
        "Expected a prefab-instance selection",
    )?;
    c.click(&format!("{prefix}frame"))?;
    c.click(&format!("{prefix}move"))?;
    c.menu(view, "Float tab")?;
    c.until("floating Move viewport", || {
        Ok(c.node(&format!("{prefix}viewport"))?.window()? != c.window()?)
    })?;
    let floating = c.node(&format!("{prefix}viewport"))?.window()?;
    c.resize(floating, 800, 600, 2)?;
    c.activate(floating)?;
    super::scene::ready(c, &prefix)?;
    c.until("floating Move handles", || {
        Ok(state(c)?["handles"]
            .as_array()
            .is_some_and(|v| !v.is_empty()))
    })?;
    let scene_path = project.join("assets/Composed.scene.ron");
    let scene_before = std::fs::read(&scene_path)?;
    drag(c, floating)?;
    c.input(floating, json!({"kind":"button","pressed":false}))?;
    c.until("instance Move committed", || {
        Ok(c.current_document()?["dirty"] == true)
    })?;
    check(
        std::fs::read(&scene_path)? == scene_before && std::fs::read(&path)? == prefab_saved,
        "Instance Move wrote sources before Save",
    )?;
    c.input(floating, json!({"kind":"key","key":"save"}))?;
    c.until("instance Move saved", || {
        Ok(c.current_document()?["dirty"] == false)
    })?;
    check(
        std::fs::read(&scene_path)? != scene_before && std::fs::read(&path)? == prefab_saved,
        "Instance override Save changed the prefab or omitted the scene",
    )?;
    c.frame(floating, "Floating prefab-instance Move saved")?;
    super::navigation_geometry::controls(c, &prefix)?;
    tools(c, &prefix, floating, &scene_path)?;
    navigation_gestures(c, &prefix, floating)?;
    check(
        std::fs::read(&path)? == prefab_saved,
        "Instance Rotate/Scale changed referenced prefab",
    )?;
    c.menu(view, "Redock to main window")?;
    Ok(json!({"status":"passed","frames":c.captures.borrow().clone()}))
}
