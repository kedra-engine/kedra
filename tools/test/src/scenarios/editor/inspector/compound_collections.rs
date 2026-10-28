//! Immediate compound fields, passive image/shadow projection, history and Save.
use super::*;
use std::path::Path;

fn projected(c: &Editor<'_>, key: &str, selected: &Value) -> Result<Value> {
    let document = c.current_document()?;
    array(&c.state()?["scene_views"][text(&document["view"])?][key])?
        .iter()
        .find(|node| &node["node"] == selected)
        .cloned()
        .ok_or("Projected compound collection unavailable".into())
}
fn control(c: &Editor<'_>, prefix: &str, component: &str, suffix: &str) -> Result<Node> {
    c.find("Compound collection control", |node| {
        node.id.starts_with(prefix)
            && node.id.contains(&format!("/{component}/"))
            && node.id.ends_with(suffix)
    })
}
fn click(c: &Editor<'_>, prefix: &str, component: &str, suffix: &str) -> Result<()> {
    c.click(&control(c, prefix, component, suffix)?.id)
}
fn item_grip(input: &str) -> Result<String> {
    let (array, item) = input.split_once("/items/").ok_or("Array item scope")?;
    let id = item.split('/').next().ok_or("Array item identity")?;
    Ok(format!("{array}/items/{id}/grip"))
}
fn field(c: &Editor<'_>, prefix: &str, component: &str, label: &str, suffix: &str) -> Result<Node> {
    // Compact inline density rows intentionally hide their redundant field labels.
    // Resolve the retained binding label, then wait for its real visible editor.
    let label: Node = c.client.wait(
        "Compound item binding",
        std::time::Duration::from_secs(40),
        || {
            let observation = c.client.observe()?;
            array(&observation["elements"])?
                .iter()
                .filter_map(|node| serde_json::from_value::<Node>(node.clone()).ok())
                .find(|node| {
                    node.id.starts_with(prefix)
                        && node.id.contains(&format!("/{component}/"))
                        && node.id.contains("/items/")
                        && node.id.ends_with("/label")
                        && node.caption() == label
                })
                .map_or(Ok(None), |node| Ok(Some(node)))
        },
    )?;
    c.wait_node(&format!(
        "{}{suffix}",
        label.id.strip_suffix("/label").ok_or("Item label scope")?
    ))
}
fn accept(c: &Editor<'_>, field: &Node, value: &str) -> Result<()> {
    c.field(&field.id, value)?;
    c.input(field.window()?, json!({"kind":"key","key":"enter"}))?;
    c.until("accepted live compound field", || {
        Ok(c.node(&field.id)?.value() == value && c.node(&field.id)?.settled == Some(true))
    })
}
fn save(
    c: &Editor<'_>,
    assets: &Path,
    inspector: &str,
    before: &std::collections::BTreeMap<std::path::PathBuf, String>,
) -> Result<()> {
    c.menu(inspector, "Save active document")?;
    c.until("compound collection Save", || {
        Ok(c.current_document()?["dirty"] == false
            && c.state()?["documents"]["saving"] == false
            && c.state()?["documents"]["save_pending"] == false)
    })?;
    let after = inventory(assets)?;
    check(
        before.keys().eq(after.keys()),
        "Compound Save changed source inventory",
    )?;
    for (path, hash) in before {
        if path != Path::new("Hud.prefab.ron") {
            check(
                after.get(path) == Some(hash),
                "Compound Save changed foreign source/sidecar",
            )?;
        }
    }
    Ok(())
}
pub(super) fn run(
    c: &Editor<'_>,
    assets: &Path,
    inspector: &str,
    hierarchy: &str,
    prefix: &str,
) -> Result<()> {
    c.click("workspace/reset")?;
    c.click(&format!("dock/tab/ViewId({hierarchy})"))?;
    let label = c.find("Hierarchy image", |node| {
        node.id
            .starts_with(&format!("hierarchy/ViewId({hierarchy})/"))
            && node.caption() == "Image inside UI"
    })?;
    let row = c
        .nodes()?
        .into_iter()
        .find(|node| Some(node.entity) == label.parent)
        .ok_or("Image hierarchy row")?;
    c.click(&row.id)?;
    let selected = c.selection()?;
    c.menu(inspector, "Search properties")?;
    c.field(&format!("{prefix}search"), "ui.image")?;
    let baseline = projected(c, "source_ui_images", &selected)?;
    let before = inventory(assets)?;
    check(
        baseline["densities"] == json!([1.0]) && baseline["rendered_image"] == "game://checker.png",
        "Native density baseline mismatch",
    )?;
    click(c, prefix, "ui.image", "/add")?;
    c.until("immediate image density insertion", || {
        Ok(projected(c, "source_ui_images", &selected)?["densities"] == json!([1.0, 2.0]))
    })?;
    let accepted = projected(c, "source_ui_images", &selected)?;
    check(
        accepted["images"] == json!(["game://checker.png", "game://checker.png"])
            && accepted["rendered_image"] == baseline["rendered_image"],
        "Density construction replaced/lost the real native image",
    )?;
    c.menu(inspector, "Undo source edit")?;
    c.until("density Undo", || {
        Ok(projected(c, "source_ui_images", &selected)? == baseline)
    })?;
    c.menu(inspector, "Redo source edit")?;
    c.until("density Redo", || {
        Ok(projected(c, "source_ui_images", &selected)? == accepted)
    })?;
    let density = field(c, prefix, "ui.image", "density", "/value")?;
    c.field(&density.id, "3")?;
    c.until("unordered density rejection", || {
        Ok(c.nodes()?.iter().any(|node| {
            node.id.starts_with(prefix) && node.caption().contains("positive and ordered")
        }))
    })?;
    check(
        projected(c, "source_ui_images", &selected)? == accepted && inventory(assets)? == before,
        "Rejected density changed preview/files",
    )?;
    check(
        c.node(&density.id)?.value() == "3",
        "Density rejection erased local draft",
    )?;
    c.input(density.window()?, json!({"kind":"key","key":"escape"}))?;
    let second = c.find("Second density", |node| {
        node.id.starts_with(prefix)
            && node.id.contains("/ui.image/")
            && node.id.contains("/items/")
            && node.value() == "2"
    })?;
    c.click(&item_grip(&second.id)?)?;
    click(c, prefix, "ui.image", "/remove")?;
    c.until("density removal", || {
        Ok(projected(c, "source_ui_images", &selected)? == baseline)
    })?;
    click(c, prefix, "ui.image", "/add")?;
    c.until("density restored", || {
        Ok(projected(c, "source_ui_images", &selected)? == accepted)
    })?;
    c.frame(
        c.window()?,
        "native image density collection and exact resource handles",
    )?;
    c.client.capture(c.window()?)?;
    c.menu(inspector, "Float group")?;
    c.until("floating image density details", || {
        Ok(field(c, prefix, "ui.image", "density", "/value")?.window()? != c.window()?)
    })?;
    let window = field(c, prefix, "ui.image", "density", "/value")?.window()?;
    c.resize(window, 320, 740, 1)?;
    let density = field(c, prefix, "ui.image", "density", "/value")?;
    let grip = c.node(&item_grip(&density.id)?)?;
    let well = c.node(&format!(
        "{}/editor",
        density
            .id
            .strip_suffix("/value")
            .ok_or("Density field scope")?
    ))?;
    check(
        density.rect[2] <= 320.
            && well.rect[2] - well.rect[0] >= 48.
            && density.rect[0] >= well.rect[0]
            && density.rect[2] <= well.rect[2]
            && grip.rect[2] <= density.rect[0]
            && (grip.center()[1] - density.center()[1]).abs() <= 2.,
        "Narrow density controls must stay inline with their grip",
    )?;
    c.frame(window, "narrow image density details")?;
    c.client.capture(window)?;
    c.menu(inspector, "Redock to main window")?;
    c.click("workspace/reset")?;
    save(c, assets, inspector, &before)?;
    check(
        std::fs::read_to_string(assets.join("Hud.prefab.ron"))?.contains("checker.png"),
        "Density Save lost symbolic image path",
    )?;

    c.field(&format!("{prefix}search"), "ui.shadow")?;
    let baseline = projected(c, "source_ui_shadows", &selected)?;
    check(baseline["values"] == json!([]), "Shadow baseline mismatch")?;
    click(c, prefix, "ui.shadow", "/add")?;
    c.until("immediate native shadow insertion", || {
        Ok(projected(c, "source_ui_shadows", &selected)?["values"]
            .as_array()
            .is_some_and(|values| !values.is_empty()))
    })?;
    // Native defaults retain their units; a unit choice is a separate source edit.
    choose(
        c,
        &field(c, prefix, "ui.shadow", "x_offset", "/unit")?,
        "Px",
    )?;
    c.until("accepted shadow X unit", || {
        Ok(field(c, prefix, "ui.shadow", "x_offset", "/unit/caption")?.caption() == "Px")
    })?;
    let x = field(c, prefix, "ui.shadow", "x_offset", "/amount/value")?;
    accept(c, &x, "12")?;
    choose(
        c,
        &field(c, prefix, "ui.shadow", "blur_radius", "/unit")?,
        "Px",
    )?;
    c.until("accepted shadow blur unit", || {
        Ok(field(c, prefix, "ui.shadow", "blur_radius", "/unit/caption")?.caption() == "Px")
    })?;
    let blur = field(c, prefix, "ui.shadow", "blur_radius", "/amount/value")?;
    let before_blur = projected(c, "source_ui_shadows", &selected)?;
    c.field(&blur.id, "NaN")?;
    c.input(blur.window()?, json!({"kind":"key","key":"enter"}))?;
    c.until("invalid shadow length rejection", || {
        Ok(c.node(&blur.id)?.value() == "NaN" && c.node(&blur.id)?.settled == Some(false))
    })?;
    check(
        projected(c, "source_ui_shadows", &selected)? == before_blur,
        "Invalid shadow mutated preview",
    )?;
    c.input(blur.window()?, json!({"kind":"key","key":"escape"}))?;
    c.until("cancelled shadow length", || {
        Ok(c.node(&blur.id)?.settled == Some(true))
    })?;
    accept(c, &blur, "8")?;
    c.until("immediate native shadow edit", || {
        Ok(projected(c, "source_ui_shadows", &selected)?["values"][0]["x"] == "Px(12.0)")
    })?;
    let accepted = projected(c, "source_ui_shadows", &selected)?;
    check(
        accepted["values"][0]["blur"] == "Px(8.0)",
        "Shadow blur did not reach native preview",
    )?;
    c.menu(inspector, "Undo source edit")?;
    c.until("shadow Undo", || {
        Ok(projected(c, "source_ui_shadows", &selected)? == before_blur)
    })?;
    c.menu(inspector, "Redo source edit")?;
    c.until("shadow Redo", || {
        Ok(projected(c, "source_ui_shadows", &selected)? == accepted)
    })?;
    let blur = field(c, prefix, "ui.shadow", "blur_radius", "/amount/value")?;
    c.click(&item_grip(&blur.id)?)?;
    click(c, prefix, "ui.shadow", "/remove")?;
    c.until("native shadow removal", || {
        Ok(projected(c, "source_ui_shadows", &selected)? == baseline)
    })?;
    c.menu(inspector, "Undo source edit")?;
    c.until("native shadow restoration", || {
        Ok(projected(c, "source_ui_shadows", &selected)? == accepted)
    })?;
    c.until("restored shadow details expanded", || {
        Ok(c.nodes()?.iter().any(|node| {
            node.id.starts_with(prefix)
                && node.id.contains("/ui.shadow/")
                && node.id.ends_with("/amount/value")
        }))
    })?;
    c.frame(c.window()?, "native full-width shadow details")?;
    c.client.capture(c.window()?)?;
    c.menu(inspector, "Float group")?;
    c.until("floating shadow details", || {
        Ok(field(c, prefix, "ui.shadow", "x_offset", "/amount/value")?.window()? != c.window()?)
    })?;
    let window = field(c, prefix, "ui.shadow", "x_offset", "/amount/value")?.window()?;
    c.resize(window, 320, 740, 1)?;
    let x = field(c, prefix, "ui.shadow", "x_offset", "/amount/value")?;
    let unit = field(c, prefix, "ui.shadow", "x_offset", "/unit")?;
    check(
        x.rect[2] <= unit.rect[0] + 1.
            && (x.center()[1] - unit.center()[1]).abs() < 2.
            && x.rect[2] - x.rect[0] >= 48.,
        "Narrow shadow amount/unit is squeezed or wrapped",
    )?;
    c.frame(window, "narrow shadow lengths and labels")?;
    c.client.capture(window)?;
    c.menu(inspector, "Redock to main window")?;
    c.click("workspace/reset")?;
    save(c, assets, inspector, &inventory(assets)?)?;
    c.click(&format!("{prefix}close-search"))?;
    Ok(())
}
