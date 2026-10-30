//! Native context menus and node movement exercise the shared source history.
use super::support::*;
use kedra_test::Result;
use serde_json::{Value, json};

fn row(c: &Editor<'_>, name: &str) -> Result<Node> {
    c.find("Hierarchy node", |n| {
        n.id.starts_with("hierarchy/") && n.id.ends_with("/name") && n.caption() == name
    })
}
pub(super) fn run(c: &Editor<'_>) -> Result<Value> {
    let project = c.open_project()?;
    let search = c.find("Assets search", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/search")
    })?;
    c.field(&search.id, "Cuboid.prefab.ron")?;
    c.open_asset("Cuboid.prefab.ron")?;
    c.until("source document admitted", || {
        Ok(c.current_document().is_ok())
    })?;
    let path = project.join("assets/Cuboid.prefab.ron");
    let before = std::fs::read(&path)?;
    for (path, name) in [
        ("Empty Node", "Object"),
        ("3D Object/Cube", "Cube"),
        ("3D Object/Sphere", "Sphere"),
        ("3D Object/Plane", "Plane"),
        ("Light/Directional Light", "Directional light"),
        ("Camera", "Camera"),
    ] {
        c.hierarchy_menu(None, &format!("Create/{path}"))?;
        row(c, name)?;
    }
    let view = c.current_document()?["view"]
        .as_str()
        .ok_or("View missing")?
        .to_owned();
    super::scene::ready(c, &format!("panel/ViewId({view})/"))?;
    check(
        std::fs::read(&path)? == before,
        "Node creation wrote before Save",
    )?;
    c.click(&row(c, "Camera")?.id)?;
    c.key("f2")?;
    c.wait_node("document-create/name")?;
    c.field("document-create/name", "Player Camera")?;
    c.click("document-create/Rename")?;
    row(c, "Player Camera")?;
    c.click(&row(c, "Plane")?.id)?;
    c.key("delete")?;
    c.until("keyboard node deletion", || {
        Ok(!c
            .nodes()?
            .iter()
            .any(|n| n.visible && n.id.starts_with("hierarchy/") && n.caption() == "Plane"))
    })?;
    c.hierarchy_menu(None, "Undo")?;
    row(c, "Plane")?;
    c.hierarchy_menu(Some("Object"), "Rename")?;
    c.field("document-create/name", "Group 日本語")?;
    c.click("document-create/Rename")?;
    let parent = row(c, "Group 日本語")?;
    let cube = row(c, "Cube")?;
    c.input(
        cube.window()?,
        json!({"kind":"drag", "from":cube.center(), "to":parent.center()}),
    )?;
    c.until("node reparented", || {
        Ok(row(c, "Cube")?.rect[0] > row(c, "Group 日本語")?.rect[0] + 4.)
    })?;
    c.frame(c.window()?, "Hierarchy node reparented")?;
    c.hierarchy_menu(Some("Group 日本語"), "Delete")?;
    c.until("subtree removed", || {
        Ok(!c.nodes()?.iter().any(|n| {
            n.visible
                && n.id.starts_with("hierarchy/")
                && matches!(n.caption(), "Cube" | "Group 日本語")
        }))
    })?;
    c.main_menu("Undo")?;
    row(c, "Cube")?;
    row(c, "Group 日本語")?;
    c.main_menu("Redo")?;
    c.until("subtree removal redone", || {
        Ok(!c
            .nodes()?
            .iter()
            .any(|n| n.visible && n.id.starts_with("hierarchy/") && n.caption() == "Group 日本語"))
    })?;
    c.key("save")?;
    c.until("hierarchy edits saved", || {
        Ok(c.current_document()?["dirty"] == false)
    })?;
    let saved = std::fs::read_to_string(&path)?;
    check(
        saved.contains("scene.camera")
            && saved.contains("scene.sun")
            && saved.contains("RemoveNode"),
        "Save lost source node edits",
    )?;
    c.frame(c.window()?, "Hierarchy creation and source Save")?;
    // Sources retain ordered overrides, including earlier names of removed nodes.
    // Verify the saved effective graph by admitting it in a fresh project lifetime.
    c.client
        .operation("editor.project", json!({"command":{"kind":"projects"}}))?;
    c.until("project closed", || {
        Ok(c.rpc("editor.state")?["phase"] == "projects")
    })?;
    c.open_project()?;
    row(c, "Player Camera")?;
    check(
        !c.nodes()?.iter().any(|n| {
            n.visible
                && n.id.starts_with("hierarchy/")
                && matches!(n.caption(), "Group 日本語" | "Cube")
        }),
        "Saved subtree deletion did not survive source readmission",
    )?;
    Ok(json!({"status":"passed", "frames":c.captures.borrow().clone()}))
}
