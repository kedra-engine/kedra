//! Real Asset Browser drag admission, hover, cancellation, rejection and publication.
use super::*;

fn start(c: &Editor<'_>, name: &str, target: &Node) -> Result<u64> {
    let search = c.find("Asset Browser search", |node| {
        node.id.starts_with("browser/") && node.id.ends_with("/search")
    })?;
    c.field(&search.id, name)?;
    let label = c.find("Dragged browser resource", |node| {
        node.id.starts_with("browser/") && node.id.ends_with("/name") && node.caption() == name
    })?;
    let row = c
        .nodes()?
        .into_iter()
        .find(|node| Some(node.entity) == label.parent)
        .ok_or("Browser resource row")?;
    let window = row.window()?;
    check(
        window == target.window()?,
        "Resource drop crosses untested windows",
    )?;
    c.input(window, json!({"kind":"move","position":label.center()}))?;
    c.input(window, json!({"kind":"button","pressed":true}))?;
    c.input(
        window,
        json!({"kind":"move","position":[label.center()[0]+8.,label.center()[1]]}),
    )?;
    c.input(window, json!({"kind":"move","position":target.center()}))?;
    c.input(window, json!({"kind":"move","position":target.center()}))?;
    Ok(window)
}
pub(super) fn run(c: &Editor<'_>, inspector: &str, texture: &Node) -> Result<()> {
    let window = start(c, "checker.png", texture)?;
    check(
        c.node(&texture.id)?.value() == "None" && c.state()?["documents"]["can_undo"] == false,
        "Resource hover changed source/history",
    )?;
    c.input(window, json!({"kind":"key","key":"escape"}))?;
    c.release(window)?;
    check(
        c.node(&texture.id)?.value() == "None" && c.state()?["documents"]["can_undo"] == false,
        "Cancelled resource drag published",
    )?;
    let window = start(c, "preview.css", texture)?;
    c.release(window)?;
    check(
        c.node(&texture.id)?.value() == "None" && c.state()?["documents"]["can_undo"] == false,
        "Incompatible stylesheet drop changed image/history",
    )?;
    let window = start(c, "checker.png", texture)?;
    c.release(window)?;
    c.until("typed Asset Browser image drop", || {
        Ok(c.node(&texture.id)?.value() == "game://checker.png")
    })?;
    c.frame(window, "Asset Browser image drop into optional texture")?;
    c.client.capture(window)?;
    c.menu(inspector, "Undo source edit")?;
    c.until("resource drop is one source Undo", || {
        Ok(c.node(&texture.id)?.value() == "None" && c.state()?["documents"]["can_undo"] == false)
    })?;
    c.menu(inspector, "Redo source edit")?;
    c.until("resource drop Redo", || {
        Ok(c.node(&texture.id)?.value() == "game://checker.png")
    })?;
    c.menu(inspector, "Undo source edit")?;
    c.until("resource drop returns to baseline", || {
        Ok(c.node(&texture.id)?.value() == "None")
    })?;
    Ok(())
}
pub(super) fn model(c: &Editor<'_>, model: &Node) -> Result<()> {
    let window = start(c, "marker.glb", model)?;
    check(
        c.node(&model.id)?.value() == "game://ship.glb#Scene0",
        "Model hover published a scene",
    )?;
    c.release(window)
}
