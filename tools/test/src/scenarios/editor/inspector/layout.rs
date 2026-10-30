//! Grouped native Layout through real input and retained passive renderer geometry.
use super::*;
use crate::scenarios::editor::scene;
fn projected(c: &Editor<'_>, view: &str, selected: &Value) -> Result<Value> {
    array(&c.state()?["scene_views"][view]["source_ui_layouts"])?
        .iter()
        .find(|node| &node["node"] == selected)
        .cloned()
        .ok_or("Projected UI layout unavailable".into())
}
pub(super) fn run(c: &Editor<'_>, prefix: &str) -> Result<()> {
    let document = c.current_document()?;
    let view = text(&document["view"])?;
    let preview = format!("panel/ViewId({view})/");
    let selected = c.selection()?;
    scene::ready(c, &preview)?;
    let original = projected(c, view, &selected)?;
    let camera = c.state()?["scene_views"][view].clone();
    let label = c.find("Native Layout width label", |node| {
        node.id.starts_with(prefix)
            && node.id.contains("/ui.layout/")
            && node.id.contains("/layout/0/")
            && node.caption() == "width"
    })?;
    let scope = label
        .id
        .strip_suffix("/label")
        .ok_or("Native width row scope")?;
    let unit = c.wait_node(&format!("{scope}/unit"))?;
    choose(c, &unit, "Px")?;
    c.until("native length unit acceptance", || {
        Ok(projected(c, view, &selected)?["width"] == "Px(0.0)")
    })?;
    scene::ready(c, &preview)?;
    let amount = c.wait_node(&format!("{scope}/amount/value"))?;
    c.field(&amount.id, "280")?;
    c.input(amount.window()?, json!({"kind":"key","key":"enter"}))?;
    c.until("live authored UI width", || {
        let current = projected(c, view, &selected)?;
        let rect = &current["rect"];
        Ok(current["width"] == "Px(280.0)"
            && (rect[2].as_f64().unwrap_or(0.) - rect[0].as_f64().unwrap_or(0.) - 280.).abs()
                < 0.01)
    })?;
    check(
        amount.rect[2] <= unit.rect[0] + 1. && (amount.center()[1] - unit.center()[1]).abs() <= 1.,
        "Native length amount and unit must share one inline row",
    )?;
    let accepted = projected(c, view, &selected)?;
    c.field(&amount.id, "NaN")?;
    c.input(amount.window()?, json!({"kind":"key","key":"enter"}))?;
    c.until("nonfinite native length rejection", || {
        Ok(c.nodes()?.iter().any(|node| {
            node.id.starts_with(prefix)
                && node.id.ends_with("/diagnostic")
                && node.caption().contains("finite")
        }))
    })?;
    check(
        projected(c, view, &selected)? == accepted,
        "Rejected Layout draft changed native geometry",
    )?;
    c.input(amount.window()?, json!({"kind":"key","key":"escape"}))?;
    c.until("cancelled native Layout input", || {
        Ok(c.node(&amount.id)?.value() == "280")
    })?;
    c.client.capture(c.window()?)?;
    let root_scope = scope
        .split("/layout/0/")
        .next()
        .ok_or("Native Layout group scope")?;
    let toggle = format!("{root_scope}/layout/0/toggle");
    c.click(&toggle)?;
    if let Err(error) = c.until("collapsed Size group", || Ok(!c.present(&amount.id)?)) {
        let frame = c.client.capture(c.window()?)?;
        let nodes = c.client.observe()?;
        std::fs::write(
            c.artifacts.join("layout-collapse.json"),
            serde_json::to_vec_pretty(&nodes)?,
        )?;
        return Err(format!("{error}; retained frame: {}", frame.path.display()).into());
    }
    c.click(&toggle)?;
    c.until("expanded Size group", || c.present(&amount.id))?;
    check(
        c.node(&amount.id)?.entity == amount.entity,
        "Grouped Layout rebuilt a retained amount field",
    )?;
    for _ in 0..2 {
        c.main_menu("Undo")?;
    }
    c.until("native Layout Undo", || {
        Ok(projected(c, view, &selected)?["width"] == original["width"])
    })?;
    for _ in 0..2 {
        c.main_menu("Redo")?;
    }
    c.until("native Layout Redo", || {
        Ok(projected(c, view, &selected)? == accepted)
    })?;
    scene::ready(c, &preview)?;
    c.frame(
        c.window()?,
        "native grouped Layout and inline unit controls",
    )?;
    let capture = c.client.capture(c.window()?)?;
    check(
        capture
            .path
            .canonicalize()?
            .starts_with(c.artifacts.canonicalize()?),
        "Layout review capture escaped the native session",
    )?;
    for key in ["translation", "rotation", "pivot"] {
        check(
            c.state()?["scene_views"][view][key] == camera[key],
            "UI Layout changed the preview camera",
        )?;
    }
    for _ in 0..2 {
        c.main_menu("Undo")?;
    }
    c.until("native Layout baseline restored", || {
        Ok(projected(c, view, &selected)?["width"] == original["width"])
    })?;
    Ok(())
}
