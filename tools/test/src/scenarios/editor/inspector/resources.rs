//! Native typed picker search, cancellation, no-op history, retained preview and
//! optional image assignment. Source bytes remain untouched until separate Save.
use super::*;
use crate::scenarios::editor::scene;
mod drops;
fn magenta_model(value: &Value) -> bool {
    value["path"] == "game://marker.glb#Scene0"
        && value["materials"].as_array().is_some_and(|materials| {
            materials.len() == 1
                && materials[0].as_array().is_some_and(|color| {
                    color.len() == 4
                        && color.iter().zip([1., 0., 1., 1.]).all(|(value, expected)| {
                            value
                                .as_f64()
                                .is_some_and(|value| (value - expected).abs() < 0.00001)
                        })
                })
        })
}

fn select(c: &Editor<'_>, hierarchy: &str, name: &str) -> Result<()> {
    let label = c.find("Resource owner in Hierarchy", |node| {
        node.id
            .starts_with(&format!("hierarchy/ViewId({hierarchy})/"))
            && node.caption() == name
    })?;
    let row = c
        .nodes()?
        .into_iter()
        .find(|node| Some(node.entity) == label.parent)
        .ok_or("Resource hierarchy row")?;
    c.click(&row.id)
}
fn well(c: &Editor<'_>, prefix: &str, component: &str) -> Result<Node> {
    c.find("Native resource path well", |node| {
        node.id.starts_with(prefix)
            && node.id.contains(&format!("/{component}/"))
            && node.id.ends_with("/path/value")
    })
}
fn scope(well: &Node) -> Result<&str> {
    well.id
        .strip_suffix("/path/value")
        .ok_or("Resource scope".into())
}
fn pick(c: &Editor<'_>, well: &Node, query: &str, path: &str) -> Result<()> {
    let scope = scope(well)?;
    c.click(&format!("{scope}/picker"))?;
    let search = c.wait_node(&format!("{scope}/search/value"))?;
    c.field(&search.id, query)?;
    let item = c.find("Typed resource result", |node| {
        node.id.starts_with(&format!("{scope}/menu/")) && node.caption() == path
    })?;
    c.click(item.id.strip_suffix("/caption").unwrap_or(&item.id))
}
pub(super) fn run(
    c: &Editor<'_>,
    assets: &std::path::Path,
    inspector: &str,
    hierarchy: &str,
    prefix: &str,
) -> Result<()> {
    let bytes = inventory(assets)?;
    let document = c.current_document()?;
    let view = text(&document["view"])?;
    let preview = format!("panel/ViewId({view})/");
    let camera = c.state()?["scene_views"][view].clone();
    select(c, hierarchy, "Composed spatial scene")?;
    let environment = well(c, prefix, "scene.environment")?;
    check(
        environment.value() == "game://day.environment.ron",
        "Environment baseline path differs",
    )?;
    let scope = scope(&environment)?;
    c.click(&format!("{scope}/picker"))?;
    let search = c.wait_node(&format!("{scope}/search/value"))?;
    c.field(&search.id, "checker.png")?;
    c.until("typed picker excludes an image from environments", || {
        Ok(!c.nodes()?.iter().any(|node| {
            node.id.starts_with(&format!("{scope}/menu/")) && node.caption().starts_with("game://")
        }))
    })?;
    c.input(search.window()?, json!({"kind":"key","key":"escape"}))?;
    c.until("resource picker Escape", || Ok(!c.present(&search.id)?))?;
    check(
        c.node(&environment.id)?.value() == "game://day.environment.ron",
        "Picker cancellation assigned a resource",
    )?;
    pick(c, &environment, "night", "game://night.environment.ron")?;
    c.until("typed environment publication", || {
        Ok(c.node(&environment.id)?.value() == "game://night.environment.ron")
    })?;
    scene::ready(c, &preview)?;
    c.frame(
        c.window()?,
        "typed environment assignment and retained camera",
    )?;
    c.client.capture(c.window()?)?;
    select(c, hierarchy, "Generated two-material model")?;
    let model = well(c, prefix, "scene.model")?;
    pick(c, &model, "ship", "game://ship.glb#Scene0")?;
    check(
        c.node(&model.id)?.value() == "game://ship.glb#Scene0",
        "Model no-op changed its exact native label",
    )?;
    c.menu(inspector, "Undo source edit")?;
    select(c, hierarchy, "Composed spatial scene")?;
    c.until("model no-op preserves environment Undo", || {
        Ok(well(c, prefix, "scene.environment")?.value() == "game://day.environment.ron")
    })?;
    check(
        c.state()?["documents"]["can_undo"] == false,
        "Same-path model assignment consumed source history",
    )?;
    select(c, hierarchy, "Generated two-material model")?;
    let model = well(c, prefix, "scene.model")?;
    let selected = c.selection()?;
    let projected_model = || -> Result<Value> {
        array(&c.state()?["scene_views"][view]["source_spatial"])?
            .iter()
            .find(|node| node["node"] == selected)
            .map(|node| node["model"].clone())
            .ok_or("Projected model missing".into())
    };
    let original_model = projected_model()?;
    pick(c, &model, "marker", "game://marker.glb#Scene0")?;
    c.until("different model owns the replacement native forest", || {
        let projected = projected_model()?;
        Ok(c.node(&model.id)?.value() == "game://marker.glb#Scene0" && magenta_model(&projected))
    })?;
    scene::ready(c, &preview)?;
    c.frame(
        c.window()?,
        "single magenta model replaces cyan/yellow forest",
    )?;
    c.client.capture(c.window()?)?;
    c.menu(inspector, "Undo source edit")?;
    c.until("model replacement Undo restores original forest", || {
        Ok(projected_model()? == original_model)
    })?;
    c.menu(inspector, "Redo source edit")?;
    c.until("model replacement Redo restores magenta forest", || {
        Ok(magenta_model(&projected_model()?))
    })?;
    c.menu(inspector, "Undo source edit")?;
    c.until("model history returns to original forest", || {
        Ok(projected_model()? == original_model)
    })?;
    drops::model(c, &model)?;
    c.until(
        "bare Asset Browser model resolves its one native scene",
        || Ok(magenta_model(&projected_model()?)),
    )?;
    c.menu(inspector, "Undo source edit")?;
    c.until("model drop Undo restores original forest", || {
        Ok(projected_model()? == original_model)
    })?;
    select(c, hierarchy, "Warm sphere")?;
    let texture = well(c, prefix, "scene.surface")?;
    check(texture.value() == "None", "Optional image baseline differs")?;
    pick(c, &texture, "checker", "game://checker.png")?;
    c.until("optional texture assignment", || {
        Ok(c.node(&texture.id)?.value() == "game://checker.png")
    })?;
    scene::ready(c, &preview)?;
    pick(c, &texture, "", "Clear")?;
    c.until("optional Clear", || {
        Ok(c.node(&texture.id)?.value() == "None")
    })?;
    c.menu(inspector, "Undo source edit")?;
    c.until("Clear Undo", || {
        Ok(c.node(&texture.id)?.value() == "game://checker.png")
    })?;
    c.menu(inspector, "Undo source edit")?;
    c.until("texture assignment Undo", || {
        Ok(c.node(&texture.id)?.value() == "None" && c.state()?["documents"]["can_undo"] == false)
    })?;
    drops::run(c, inspector, &texture)?;
    for key in ["translation", "rotation", "pivot"] {
        check(
            c.state()?["scene_views"][view][key] == camera[key],
            "Resource assignment changed the preview camera",
        )?;
    }
    check(
        inventory(assets)? == bytes,
        "Resource picker wrote source bytes or sidecars without Save",
    )?;
    Ok(())
}
