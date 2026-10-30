//! Luau composes controls; real native acceptance edits the source through Rust.
use super::*;
use crate::scenarios::editor::scene;

fn projected(c: &Editor<'_>, view: &str, selected: &Value) -> Result<Value> {
    array(&c.state()?["scene_views"][view]["source_captions"])?
        .iter()
        .find(|node| &node["node"] == selected)
        .cloned()
        .ok_or("Projected caption unavailable".into())
}
fn field(c: &Editor<'_>, prefix: &str, index: usize) -> Result<Node> {
    c.find("Luau native field", |node| {
        node.id.starts_with(prefix)
            && node.id.contains("/ui.caption/")
            && node.id.ends_with(&format!("/field-{index}/value"))
    })
}
fn choice(c: &Editor<'_>, prefix: &str, index: usize) -> Result<Node> {
    c.find("Luau native choice", |node| {
        node.id.starts_with(prefix)
            && node.id.contains("/ui.caption/")
            && node.id.ends_with(&format!("/field-{index}/choice"))
    })
}
pub(super) fn select_title(c: &Editor<'_>, hierarchy: &str) -> Result<Value> {
    let label = c.find("Hierarchy caption", |node| {
        node.id
            .starts_with(&format!("hierarchy/ViewId({hierarchy})/"))
            && node.caption() == "title"
    })?;
    let row = c
        .nodes()?
        .into_iter()
        .find(|node| Some(node.entity) == label.parent)
        .ok_or("Caption hierarchy row missing")?;
    c.click(&row.id)?;
    c.selection()
}
fn accept(c: &Editor<'_>, field: &Node, text: &str) -> Result<()> {
    c.field(&field.id, text)?;
    c.input(field.window()?, json!({"kind":"key","key":"enter"}))?;
    Ok(())
}
pub(super) fn run(c: &Editor<'_>, inspector: &str, hierarchy: &str, prefix: &str) -> Result<()> {
    // Composed.scene retains its mixed 3D/UI projection from the Transform scenario.
    let document = c.current_document()?;
    let view = text(&document["view"])?;
    let preview = format!("panel/ViewId({view})/");
    let selected = select_title(c, hierarchy)?;
    let text_field = field(c, prefix, 0)?;
    let alignment = choice(c, prefix, 1)?;
    let wrap = choice(c, prefix, 2)?;
    let baseline = projected(c, view, &selected)?;
    check(
        text_field.value() == "SCENE / SYSTEM CHECK"
            && c.node(&format!("{}/caption", alignment.id))?.caption() == "Left"
            && wrap.checked == Some(false),
        "Luau drawer did not bind native caption values",
    )?;
    let camera = c.state()?["scene_views"][view].clone();
    c.click(&alignment.id)?;
    c.find("Native alignment variants", |node| {
        node.id.starts_with(&format!("{}/menu/", alignment.id)) && node.caption() == "Center"
    })?;
    check(
        projected(c, view, &selected)? == baseline && c.current_document()?["dirty"] == false,
        "Opening a native choice mutated source/history",
    )?;
    c.input(alignment.window()?, json!({"kind":"key","key":"escape"}))?;
    c.until("native choice cancellation", || {
        Ok(!c
            .nodes()?
            .iter()
            .any(|node| node.visible && node.id.starts_with(&format!("{}/menu/", alignment.id))))
    })?;
    c.field(&text_field.id, "Кедра · 日本語")?;
    c.until("scripted live text acceptance without Enter", || {
        Ok(projected(c, view, &selected)?["text"] == "Кедра · 日本語")
    })?;
    scene::ready(c, &preview)?;
    choose(c, &alignment, "Center")?;
    c.until("scripted native alignment", || {
        Ok(projected(c, view, &selected)?["justify"] == "Center")
    })?;
    c.click(&wrap.id)?;
    c.until("scripted native wrapping", || {
        Ok(projected(c, view, &selected)?["wrap"] == true)
    })?;
    scene::ready(c, &preview)?;
    c.frame(text_field.window()?, "Luau caption in mixed Scene View")?;
    for (part, expected) in [
        ("wrap", json!(false)),
        ("justify", json!("Left")),
        ("text", baseline["text"].clone()),
    ] {
        c.main_menu("Undo")?;
        c.until("scripted source Undo", || {
            Ok(projected(c, view, &selected)?[part] == expected)
        })?;
    }
    for (part, expected) in [
        ("text", json!("Кедра · 日本語")),
        ("justify", json!("Center")),
        ("wrap", json!(true)),
    ] {
        c.main_menu("Redo")?;
        c.until("scripted source Redo", || {
            Ok(projected(c, view, &selected)?[part] == expected)
        })?;
    }
    check(
        c.node(&text_field.id)?.entity == text_field.entity
            && c.node(&alignment.id)?.entity == alignment.entity
            && c.node(&wrap.id)?.entity == wrap.entity,
        "Scripted source history rebuilt native controls",
    )?;
    let current = c.state()?["scene_views"][view].clone();
    for key in ["translation", "rotation", "pivot"] {
        check(
            current[key] == camera[key],
            "Caption edits reset preview camera",
        )?;
    }
    c.menu(inspector, "Use default reflected view")?;
    c.until("scripted default view agreement", || {
        Ok(inputs(c, prefix)?
            .iter()
            .any(|node| node.value() == "Кедра · 日本語"))
    })?;
    c.menu(inspector, "Use custom presentations")?;
    check(
        field(c, prefix, 0)?.value() == "Кедра · 日本語",
        "Script re-admission lost accepted source",
    )?;
    for _ in 0..3 {
        c.main_menu("Undo")?;
    }
    c.until("scripted source baseline restored", || {
        Ok(projected(c, view, &selected)? == baseline)
    })?;
    scene::ready(c, &preview)?;
    // UI-only projection uses the same source protocol, without a mixed canvas.
    let search = c.find("Assets search", |node| {
        node.id.starts_with("browser/") && node.id.ends_with("/search")
    })?;
    c.field(&search.id, "Hud.prefab.ron")?;
    c.open_asset("Hud.prefab.ron")?;
    let document = c.current_document()?;
    let view = text(&document["view"])?;
    let preview = format!("panel/ViewId({view})/");
    scene::ready(c, &preview)?;
    let selected = select_title(c, hierarchy)?;
    let input = field(c, prefix, 0)?;
    check(
        input.value() == "SCENE / SYSTEM CHECK",
        "Nested source edit modified the prefab source",
    )?;
    accept(c, &input, "UI-only Luau caption")?;
    c.until("UI-only caption refresh", || {
        Ok(projected(c, view, &selected)?["text"] == "UI-only Luau caption")
    })?;
    scene::ready(c, &preview)?;
    c.frame(input.window()?, "Luau caption in UI-only Scene View")?;
    c.main_menu("Undo")?;
    scene::ready(c, &preview)?;
    Ok(())
}
