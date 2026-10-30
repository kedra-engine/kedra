//! Immediate classes, real grip drag/keyboard reorder, rejected text and guarded Save.
use super::*;
use std::path::Path;
fn projected(c: &Editor<'_>, view: &str, selected: &Value) -> Result<Value> {
    array(&c.state()?["scene_views"][view]["source_ui_collections"])?
        .iter()
        .find(|node| &node["node"] == selected)
        .cloned()
        .ok_or("Projected classes unavailable".into())
}
fn control(c: &Editor<'_>, prefix: &str, suffix: &str) -> Result<Node> {
    c.find("Native Classes control", |node| {
        node.id.starts_with(prefix) && node.id.contains("/ui.classes/") && node.id.ends_with(suffix)
    })
}
fn value(c: &Editor<'_>, prefix: &str, text: &str) -> Result<Node> {
    c.find("Native class value", |node| {
        node.id.starts_with(prefix) && node.id.contains("/ui.classes/") && node.value() == text
    })
}
fn grip(c: &Editor<'_>, field: &Node) -> Result<Node> {
    c.wait_node(&format!(
        "{}/grip",
        field.id.strip_suffix("/[]/value").ok_or("Class scope")?
    ))
}
fn move_key(c: &Editor<'_>, grip: &Node, key: &str) -> Result<()> {
    c.click(&grip.id)?;
    c.input(
        grip.window()?,
        json!({"kind":"modifier","modifier":"alt","pressed":true}),
    )?;
    c.input(grip.window()?, json!({"kind":"key","key":key}))?;
    c.input(
        grip.window()?,
        json!({"kind":"modifier","modifier":"alt","pressed":false}),
    )?;
    Ok(())
}
pub(super) fn run(c: &Editor<'_>, assets: &Path, inspector: &str, prefix: &str) -> Result<()> {
    let document = c.current_document()?;
    let view = text(&document["view"])?;
    let selected = c.selection()?;
    let before = inventory(assets)?;
    check(
        projected(c, view, &selected)?["classes"] == json!(["hud-title"]),
        "Class baseline mismatch",
    )?;
    c.click(&control(c, prefix, "/add")?.id)?;
    c.until("immediate class insertion", || {
        Ok(projected(c, view, &selected)?["classes"] == json!(["hud-title", "class"]))
    })?;
    let field = value(c, prefix, "class")?;
    c.field(&field.id, "Кедра-日本語")?;
    c.until("live class value without Enter", || {
        Ok(projected(c, view, &selected)?["classes"] == json!(["hud-title", "Кедра-日本語"]))
    })?;
    let moved = grip(c, &field)?;
    let first = grip(c, &value(c, prefix, "hud-title")?)?;
    c.input(
        moved.window()?,
        json!({"kind":"drag","from":moved.center(),"to":[first.center()[0],first.rect[1]+1.]}),
    )?;
    c.until("completed native grip reorder", || {
        Ok(projected(c, view, &selected)?["classes"] == json!(["Кедра-日本語", "hud-title"]))
    })?;
    check(
        c.node(&field.id)?.entity == field.entity && c.node(&field.id)?.value() == "Кедра-日本語",
        "Reorder replaced or rebound the moved input",
    )?;
    c.main_menu("Undo")?;
    c.until("reorder source Undo", || {
        Ok(projected(c, view, &selected)?["classes"] == json!(["hud-title", "Кедра-日本語"]))
    })?;
    c.until("Undo restores moved item focus and indexed binding", || {
        Ok(c.node(&field.id)?.entity == field.entity
            && c.node(&field.id)?.focused
            && value(c, prefix, "Кедра-日本語")?.id == field.id)
    })?;
    c.main_menu("Redo")?;
    c.until("reorder source Redo", || {
        Ok(projected(c, view, &selected)?["classes"] == json!(["Кедра-日本語", "hud-title"]))
    })?;
    c.until("Redo restores moved item focus", || {
        Ok(c.node(&field.id)?.focused)
    })?;
    let field = value(c, prefix, "Кедра-日本語")?;
    move_key(c, &grip(c, &field)?, "down")?;
    c.until("native Alt Down reorder", || {
        Ok(projected(c, view, &selected)?["classes"] == json!(["hud-title", "Кедра-日本語"]))
    })?;
    c.click(&control(c, prefix, "/remove")?.id)?;
    c.until("immediate selected item removal", || {
        Ok(projected(c, view, &selected)?["classes"] == json!(["hud-title"]))
    })?;
    c.main_menu("Undo")?;
    c.until("removal Undo restores selected item and focus", || {
        Ok(
            projected(c, view, &selected)?["classes"] == json!(["hud-title", "Кедра-日本語"])
                && value(c, prefix, "Кедра-日本語")?.focused,
        )
    })?;
    c.main_menu("Redo")?;
    c.until("removal Redo retains survivor", || {
        Ok(projected(c, view, &selected)?["classes"] == json!(["hud-title"]))
    })?;
    c.click(&control(c, prefix, "/add")?.id)?;
    c.until("new valid class", || {
        Ok(projected(c, view, &selected)?["classes"] == json!(["hud-title", "class"]))
    })?;
    let invalid = value(c, prefix, "class")?;
    c.field(&invalid.id, "hud-title")?;
    c.until("duplicate class rejection", || {
        Ok(c.node(&invalid.id)?.settled == Some(false)
            && c.nodes()?.iter().any(|node| {
                node.id.starts_with(prefix)
                    && node.id.ends_with("/diagnostic")
                    && node.caption().contains("unique")
            }))
    })?;
    check(
        projected(c, view, &selected)?["classes"] == json!(["hud-title", "class"]),
        "Invalid class changed source",
    )?;
    c.menu(inspector, "Close tab")?;
    c.until("hidden invalid class", || c.hidden(inspector))?;
    c.input(c.window()?, json!({"kind":"key","key":"save"}))?;
    c.input(c.window()?, json!({"kind":"close_window"}))?;
    c.click("close-dialog/cancel")?;
    check(
        inventory(assets)? == before,
        "Hidden invalid class was saved or discarded",
    )?;
    c.main_menu("Windows/Inspector")?;
    c.click("workspace/reset")?;
    check(
        c.wait_node(&invalid.id)?.entity == invalid.entity
            && c.node(&invalid.id)?.value() == "hud-title",
        "Hiding lost invalid native text",
    )?;
    c.click(&invalid.id)?;
    c.input(invalid.window()?, json!({"kind":"key","key":"escape"}))?;
    c.until("cancel rejected class", || {
        Ok(c.node(&invalid.id)?.value() == "class" && c.node(&invalid.id)?.settled == Some(true))
    })?;
    c.field(&invalid.id, "Кедра-日本語")?;
    c.until("corrected live class", || {
        Ok(projected(c, view, &selected)?["classes"] == json!(["hud-title", "Кедра-日本語"]))
    })?;
    move_key(c, &grip(c, &invalid)?, "up")?;
    c.until("native Alt Up reorder", || {
        Ok(projected(c, view, &selected)?["classes"] == json!(["Кедра-日本語", "hud-title"]))
    })?;
    c.frame(
        c.window()?,
        "direct arrays, grip reorder and rejected native text",
    )?;
    c.client.capture(c.window()?)?;
    let retained = value(c, prefix, "Кедра-日本語")?;
    c.menu(inspector, "Float tab")?;
    c.until("floating compact array", || {
        Ok(c.node(&retained.id)?.window()? != c.window()?)
    })?;
    let window = c.node(&retained.id)?.window()?;
    c.resize(window, 320, 740, 1)?;
    let narrow = c.node(&retained.id)?;
    let handle = grip(c, &retained)?;
    check(
        narrow.entity == retained.entity
            && handle.rect[2] <= narrow.rect[0]
            && (narrow.center()[1] - handle.center()[1]).abs() <= 2.
            && narrow.rect[2] <= 320.,
        "Narrow array wrapped, clipped or replaced its input",
    )?;
    c.frame(window, "compact 320px native array")?;
    c.client.capture(window)?;
    c.menu(inspector, "Redock to main window")?;
    c.until("redocked compact array", || {
        Ok(c.node(&retained.id)?.window()? == c.window()?)
    })?;
    c.main_menu("Save")?;
    c.until("direct collection Save", || {
        Ok(c.current_document()?["dirty"] == false
            && c.state()?["documents"]["save_pending"] == false
            && c.state()?["documents"]["saving"] == false)
    })?;
    let saved = std::fs::read_to_string(assets.join("Hud.prefab.ron"))?;
    check(
        saved.contains("Кедра-日本語"),
        "Save omitted accepted classes",
    )?;
    let after = inventory(assets)?;
    check(
        after.keys().eq(before.keys()),
        "Collection Save changed source inventory",
    )?;
    for (path, hash) in &before {
        if path != Path::new("Hud.prefab.ron") {
            check(
                after.get(path) == Some(hash),
                "Collection Save changed a foreign source/sidecar",
            )?;
        }
    }
    c.main_menu("Undo")?;
    c.until("saved collection Undo", || {
        Ok(c.current_document()?["dirty"] == true)
    })?;
    c.main_menu("Redo")?;
    c.until("saved collection baseline", || {
        Ok(projected(c, view, &selected)?["classes"] == json!(["Кедра-日本語", "hud-title"]))
    })?;
    check(
        std::fs::read_to_string(assets.join("Hud.prefab.ron"))? == saved,
        "Discard rewrote saved classes",
    )?;
    Ok(())
}
