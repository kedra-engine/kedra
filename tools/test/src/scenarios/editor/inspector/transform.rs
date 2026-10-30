//! One real source-edit path through native input, renderer refresh and history.
use super::*;
use crate::scenarios::editor::scene;
fn projected(c: &Editor<'_>, view: &str, selected: &Value) -> Result<Value> {
    array(&c.state()?["scene_views"][view]["source_transforms"])?
        .iter()
        .find(|node| &node["node"] == selected)
        .cloned()
        .ok_or("Projected Transform unavailable".into())
}
fn axis(c: &Editor<'_>, prefix: &str, part: &str, axis: &str) -> Result<Node> {
    c.find("source Transform axis", |node| {
        node.id.starts_with(prefix) && node.id.ends_with(&format!("/{part}/{axis}/value"))
    })
}
fn accept(c: &Editor<'_>, input: &Node, value: &str) -> Result<()> {
    c.field(&input.id, value)?;
    c.input(input.window()?, json!({"kind":"key","key":"enter"}))?;
    Ok(())
}
pub(super) fn run(c: &Editor<'_>, hierarchy: &str, prefix: &str) -> Result<()> {
    let root = c.find("Assets root", |node| {
        node.id.starts_with("browser/") && node.id.ends_with("/folder/0")
    })?;
    c.click(&root.id)?;
    let search = c.find("Assets search", |node| {
        node.id.starts_with("browser/") && node.id.ends_with("/search")
    })?;
    c.field(&search.id, "Composed.scene.ron")?;
    c.open_asset("Composed.scene.ron")?;
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
        .ok_or("Sphere hierarchy row missing")?;
    c.click(&row.id)?;
    let selected = c.selection()?;
    let x = axis(c, prefix, "translation", "x")?;
    let y = axis(c, prefix, "translation", "y")?;
    let z = axis(c, prefix, "translation", "z")?;
    let camera = c.state()?["scene_views"][view].clone();
    check(
        x.value() == "3" && y.value() == "1" && z.value() == "0",
        "Source Transform baseline differs",
    )?;
    // Invalid and incomplete drafts must not publish the previous valid value.
    for invalid in ["-", "NaN", "1000001"] {
        accept(c, &x, invalid)?;
        c.until("rejected source Transform", || {
            Ok(c.node(&x.id)?.value() == invalid
                && c.nodes()?.iter().any(|node| {
                    node.id.starts_with(prefix)
                        && node.id.ends_with("/translation/diagnostic")
                        && !node.caption().is_empty()
                }))
        })?;
        check(
            projected(c, view, &selected)?["translation"] == json!([3., 1., 0.]),
            "Rejected Transform reached the renderer",
        )?;
        check(
            c.state()?["documents"]["can_undo"] == false,
            "Rejected Transform created source history",
        )?;
        c.input(x.window()?, json!({"kind":"key","key":"escape"}))?;
        c.until("cancel source Transform draft", || {
            Ok(c.node(&x.id)?.value() == "3")
        })?;
    }
    c.field(&x.id, "8")?;
    c.until("live source translation without Enter", || {
        Ok(projected(c, view, &selected)?["translation"] == json!([8., 1., 0.]))
    })?;
    c.input(x.window()?, json!({"kind":"text","text":"."}))?;
    c.until("noncanonical native numeric spelling", || {
        Ok(c.node(&x.id)?.value() == "8.")
    })?;
    check(
        projected(c, view, &selected)?["translation"] == json!([8., 1., 0.]),
        "Equivalent live number changed another source axis",
    )?;
    c.input(x.window()?, json!({"kind":"text","text":"5"}))?;
    c.until("accepted source translation", || {
        Ok(projected(c, view, &selected)?["translation"] == json!([8.5, 1., 0.]))
    })?;
    scene::ready(c, &preview)?;
    c.frame(x.window()?, "accepted source Transform")?;
    check(
        c.selection()? == selected,
        "Transform acceptance cleared selection",
    )?;
    check(
        c.node(&x.id)?.entity == x.entity
            && c.node(&y.id)?.entity == y.entity
            && c.node(&z.id)?.entity == z.entity,
        "Source acceptance rebuilt retained axes",
    )?;
    check(
        c.node(&y.id)?.value() == "1" && c.node(&z.id)?.value() == "0",
        "Source axis edit overwrote untouched axes",
    )?;
    c.input(x.window()?, json!({"kind":"key","key":"undo"}))?;
    c.until("settled live field source Undo", || {
        Ok(
            projected(c, view, &selected)?["translation"] == json!([3., 1., 0.])
                && c.state()?["documents"]["can_undo"] == false,
        )
    })?;
    c.input(x.window()?, json!({"kind":"key","key":"redo"}))?;
    c.until("settled live field source Redo", || {
        Ok(projected(c, view, &selected)?["translation"] == json!([8.5, 1., 0.]))
    })?;
    let rotation = axis(c, prefix, "rotation", "y")?;
    accept(c, &rotation, "45")?;
    c.until("accepted source quaternion", || {
        Ok(projected(c, view, &selected)?["rotation"] != json!([0., 0., 0., 1.]))
    })?;
    let rotated = projected(c, view, &selected)?["rotation"].clone();
    let scale = axis(c, prefix, "scale", "z")?;
    accept(c, &scale, "2")?;
    c.until("accepted source scale", || {
        Ok(projected(c, view, &selected)?["scale"] == json!([1., 1., 2.]))
    })?;
    for (part, expected) in [
        ("scale", json!([1., 1., 1.])),
        ("rotation", json!([0., 0., 0., 1.])),
        ("translation", json!([3., 1., 0.])),
    ] {
        c.main_menu("Undo")?;
        c.until("source Undo reprojection", || {
            Ok(projected(c, view, &selected)?[part] == expected)
        })?;
        scene::ready(c, &preview)?;
        check(
            c.selection()? == selected,
            format!("Undo {part} cleared selection"),
        )?;
    }
    check(
        c.node(&x.id)?.value() == "3" && c.node(&x.id)?.entity == x.entity,
        "Document Undo did not refresh retained Inspector",
    )?;
    for (part, expected) in [
        ("translation", json!([8.5, 1., 0.])),
        ("rotation", rotated),
        ("scale", json!([1., 1., 2.])),
    ] {
        c.main_menu("Redo")?;
        c.until("source Redo reprojection", || {
            Ok(projected(c, view, &selected)?[part] == expected)
        })?;
        scene::ready(c, &preview)?;
        check(
            c.selection()? == selected,
            format!("Redo {part} cleared selection"),
        )?;
    }
    let final_camera = c.state()?["scene_views"][view].clone();
    c.input(c.window()?, json!({"kind":"close_window"}))?;
    c.click("close-dialog/cancel")?;
    check(
        c.selection()? == selected,
        "Exit cancellation cleared selection",
    )?;
    check(
        c.node(&x.id)?.entity == x.entity,
        "Main-window Close discarded unsaved source changes",
    )?;
    for key in ["translation", "rotation", "pivot"] {
        check(
            camera[key] == final_camera[key],
            "Source edits/history reset the Scene View camera",
        )?;
    }
    check(
        c.node(&x.id)?.entity == x.entity,
        "Document Redo rebuilt the source field",
    )?;
    c.frame(x.window()?, "source Transform Undo Redo")?;
    for _ in 0..3 {
        c.main_menu("Undo")?;
    }
    c.until("source history restores baseline", || {
        Ok(
            projected(c, view, &selected)?["translation"] == json!([3., 1., 0.])
                && c.state()?["documents"]["can_undo"] == false,
        )
    })?;
    scene::ready(c, &preview)?;
    Ok(())
}
