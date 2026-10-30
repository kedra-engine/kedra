//! Existing 3D source values through native fields, retained rendering and history.
use super::*;
use crate::scenarios::editor::scene;

fn projected(c: &Editor<'_>, view: &str, selected: &Value) -> Result<Value> {
    array(&c.state()?["scene_views"][view]["source_spatial"])?
        .iter()
        .find(|node| &node["node"] == selected)
        .cloned()
        .ok_or("Projected spatial value unavailable".into())
}
fn field(c: &Editor<'_>, prefix: &str, component: &str, value: &str) -> Result<Node> {
    c.find("typed spatial field", |node| {
        node.id.starts_with(prefix) && node.id.contains(component) && node.value() == value
    })
}
fn accept(c: &Editor<'_>, node: &Node, text: &str) -> Result<()> {
    c.field(&node.id, text)?;
    c.input(node.window()?, json!({"kind":"key","key":"enter"}))?;
    Ok(())
}
pub(super) fn run(
    c: &Editor<'_>,
    assets: &std::path::Path,
    hierarchy: &str,
    prefix: &str,
) -> Result<()> {
    let before = inventory(assets)?;
    let document = c.current_document()?;
    let view = text(&document["view"])?;
    let preview = format!("panel/ViewId({view})/");
    scene::ready(c, &preview)?;
    let label = c.find("Hierarchy sphere", |node| {
        node.id
            .starts_with(&format!("hierarchy/ViewId({hierarchy})/"))
            && node.caption() == "Warm sphere"
    })?;
    let row = c
        .nodes()?
        .into_iter()
        .find(|node| Some(node.entity) == label.parent)
        .ok_or("Sphere row")?;
    c.click(&row.id)?;
    let selected = c.selection()?;
    let original = projected(c, view, &selected)?;
    let variant = c.find("Native Primitive variant", |node| {
        node.id.starts_with(prefix)
            && node.id.contains("scene.primitive")
            && node.id.ends_with("/[]")
    })?;
    choose(c, &variant, "Cuboid")?;
    c.until("native cuboid constructor", || {
        Ok(projected(c, view, &selected)?["mesh_half_extents"] == json!([0.5, 0.5, 0.5]))
    })?;
    choose(c, &variant, "Sphere")?;
    c.until("native sphere constructor", || {
        Ok(projected(c, view, &selected)?["mesh_half_extents"] == json!([0.5, 0.5, 0.5]))
    })?;
    for _ in 0..2 {
        c.main_menu("Undo")?;
    }
    c.until("variant Undo restores payload", || {
        Ok(projected(c, view, &selected)? == original)
    })?;
    let radius = field(c, prefix, "scene.primitive", "1")?;
    let roughness = field(c, prefix, "scene.surface", "0.4")?;
    let color = c.find("native color field", |node| {
        node.id.starts_with(prefix)
            && node.id.contains("scene.surface")
            && node.value().starts_with('#')
    })?;
    let camera = c.state()?["scene_views"][view].clone();
    accept(c, &radius, "1.25")?;
    c.until("primitive mesh refresh", || {
        Ok(projected(c, view, &selected)?["mesh_half_extents"][0] == 1.25)
    })?;
    accept(c, &roughness, "0.75")?;
    c.until("material roughness refresh", || {
        Ok(projected(c, view, &selected)?["material"]["roughness"] == 0.75)
    })?;
    accept(c, &color, "#00FF00")?;
    c.until("material color refresh", || {
        Ok(projected(c, view, &selected)?["material"]["rgba"] == json!([0., 1., 0., 1.]))
    })?;
    c.until("canonical color display", || {
        Ok(c.node(&color.id)?.value() == "#00FF00")
    })?;
    let swatch = c.node(&format!(
        "{}/swatch",
        color.id.strip_suffix("/value").ok_or("Color input scope")?
    ))?;
    let current_color = c.node(&color.id)?;
    check(
        swatch.rect[2] <= current_color.rect[0] + 1.
            && (swatch.rect[3] - swatch.rect[1] - 16.).abs() <= 1.
            && (swatch.rect[2] - swatch.rect[0] - 28.).abs() <= 1.,
        "Color swatch must precede hex text and retain the canonical 28x16 geometry",
    )?;
    c.click(&swatch.id)?;
    let scope = color.id.strip_suffix("/value").ok_or("Color scope")?;
    let alpha = c.wait_node(&format!("{scope}/palette/alpha/value"))?;
    accept(c, &alpha, "0.5")?;
    c.until("exact native alpha", || {
        Ok(projected(c, view, &selected)?["material"]["rgba"][3] == 0.5)
    })?;
    let space = c.wait_node(&format!("{scope}/palette/space"))?;
    choose(c, &space, "Linear RGBA")?;
    c.until("native color space selection", || {
        Ok(c.node(&format!("{}/caption", space.id))?.caption() == "Linear RGBA")
    })?;
    let palette = c.wait_node(&format!("{scope}/palette/presets"))?;
    choose(c, &palette, "Purple")?;
    c.until("palette preserves alpha", || {
        let rgba = projected(c, view, &selected)?["material"]["rgba"].clone();
        Ok([0.5, 0., 1., 0.5]
            .into_iter()
            .enumerate()
            .all(|(index, expected)| {
                (rgba[index].as_f64().unwrap_or(-1.) - expected).abs() < 0.00001
            }))
    })?;
    scene::ready(c, &preview)?;
    c.frame(c.window()?, "native palette, alpha and color-space fields")?;
    let palette_capture = c.client.capture(c.window()?)?;
    check(
        palette_capture
            .path
            .canonicalize()?
            .starts_with(c.artifacts.canonicalize()?),
        "Palette review capture escaped the native session",
    )?;
    for _ in 0..3 {
        c.main_menu("Undo")?;
    }
    c.until("palette Undo", || {
        Ok(projected(c, view, &selected)?["material"]["rgba"] == json!([0., 1., 0., 1.]))
    })?;
    check(
        c.node(&alpha.id)?.entity == alpha.entity,
        "Palette source refresh rebuilt native channel inputs",
    )?;
    c.click(&swatch.id)?;
    c.until("closed native palette", || Ok(!c.present(&alpha.id)?))?;
    scene::ready(c, &preview)?;
    c.input(c.window()?, json!({"kind":"move","position":[600.,400.]}))?;
    c.frame(
        c.window()?,
        "native primitive dimensions and material color",
    )?;
    let capture = c.client.capture(c.window()?)?;
    check(
        capture
            .path
            .canonicalize()?
            .starts_with(c.artifacts.canonicalize()?),
        "Manual review capture escaped the native session",
    )?;
    let accepted = projected(c, view, &selected)?;
    for (input, invalid) in [(&radius, "0"), (&roughness, "2"), (&color, "#xyzxyz")] {
        accept(c, input, invalid)?;
        c.until("rejected typed value", || {
            Ok(c.nodes()?.iter().any(|node| {
                node.id.starts_with(prefix)
                    && node.id.ends_with("/diagnostic")
                    && !node.caption().is_empty()
            }))
        })?;
        check(
            projected(c, view, &selected)? == accepted,
            "Rejected spatial value changed native projection",
        )?;
        c.input(input.window()?, json!({"kind":"key","key":"escape"}))?;
    }
    for _ in 0..3 {
        c.main_menu("Undo")?;
    }
    c.until("spatial source Undo", || {
        Ok(projected(c, view, &selected)? == original)
    })?;
    for _ in 0..3 {
        c.main_menu("Redo")?;
    }
    c.until("spatial source Redo", || {
        Ok(projected(c, view, &selected)? == accepted)
    })?;
    for key in ["translation", "rotation", "pivot"] {
        check(
            c.state()?["scene_views"][view][key] == camera[key],
            "Spatial value edits reset the camera",
        )?;
    }
    for input in [&radius, &roughness, &color] {
        check(
            c.node(&input.id)?.entity == input.entity,
            "Typed edit rebuilt a native field",
        )?;
    }
    c.input(color.window()?, json!({"kind":"key","key":"save"}))?;
    c.until("spatial source Save", || {
        Ok(c.current_document()?["dirty"] == false
            && c.state()?["documents"]["save_pending"] == false)
    })?;
    let saved = inventory(assets)?;
    check(
        saved.keys().eq(before.keys()),
        "Spatial Save created a source or sidecar",
    )?;
    for (path, hash) in &before {
        if path == std::path::Path::new("Composed.scene.ron") {
            check(
                saved.get(path) != Some(hash),
                "Spatial Save omitted accepted values",
            )?;
        } else {
            check(
                saved.get(path) == Some(hash),
                "Spatial Save changed another source",
            )?;
        }
    }
    accept(c, &roughness, "0.25")?;
    c.until("new source change after Save", || {
        Ok(projected(c, view, &selected)?["material"]["roughness"] == 0.25)
    })?;
    c.main_menu("Undo")?;
    c.until("spatial saved-baseline restored", || {
        Ok(projected(c, view, &selected)? == accepted)
    })?;
    check(
        inventory(assets)? == saved,
        "Discard rewrote saved spatial bytes",
    )?;
    scene::ready(c, &preview)?;
    Ok(())
}
