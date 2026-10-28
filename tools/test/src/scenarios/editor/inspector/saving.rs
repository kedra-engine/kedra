//! Explicit Save uses native input and writes only the private fixture's active source.
use super::*;
use crate::scenarios::editor::scene;
use std::path::Path;
fn input(c: &Editor<'_>, prefix: &str) -> Result<Node> {
    c.find("Save caption input", |node| {
        node.id.starts_with(prefix)
            && node.id.contains("/ui.caption/")
            && node.id.ends_with("/field-0/value")
    })
}
fn accept(c: &Editor<'_>, field: &Node, text: &str) -> Result<()> {
    c.field(&field.id, text)?;
    c.input(field.window()?, json!({"kind":"key","key":"enter"}))?;
    c.until("accepted Save candidate", || {
        Ok(c.node(&field.id)?.value() == text && c.state()?["documents"]["can_undo"] == true)
    })
}
fn clean(c: &Editor<'_>) -> Result<bool> {
    let document = c.current_document()?;
    Ok(document["dirty"] == false
        && c.state()?["documents"]["saving"] == false
        && c.state()?["documents"]["save_pending"] == false)
}
pub(super) fn run(c: &Editor<'_>, assets: &Path, inspector: &str, prefix: &str) -> Result<()> {
    let before = inventory(assets)?;
    let path = assets.join("Hud.prefab.ron");
    let field = input(c, prefix)?;
    let document = c.current_document()?;
    let view = text(&document["view"])?;
    let preview = format!("panel/ViewId({view})/");
    accept(c, &field, "Saved through Luau")?;
    scene::ready(c, &preview)?;
    // Any single-line caption is now accepted live. Exercise Save's draft guard
    // with a genuinely invalid native number, not another valid caption string.
    let group = c.find("Flex group title", |node| {
        node.id.starts_with(prefix)
            && node.id.contains("/ui.layout/")
            && node.id.ends_with("/title")
            && node.caption() == "Flex & alignment"
    })?;
    let toggle = format!(
        "{}/toggle",
        group.id.strip_suffix("/title").ok_or("Flex scope")?
    );
    c.click(&toggle)?;
    let label = c.find("Flex grow label", |node| {
        node.id.starts_with(prefix)
            && node.id.contains("/ui.layout/")
            && node.caption() == "flex_grow"
    })?;
    let draft = c.wait_node(&format!(
        "{}/value",
        label.id.strip_suffix("/label").ok_or("Flex grow scope")?
    ))?;
    let original = draft.value().to_owned();
    c.field(&draft.id, "NaN")?;
    c.until("rejected draft before Save", || {
        Ok(c.nodes()?.iter().any(|node| {
            node.id.starts_with(prefix)
                && node.id.ends_with("/diagnostic")
                && node.caption().contains("finite")
        }))
    })?;
    c.input(draft.window()?, json!({"kind":"key","key":"save"}))?;
    check(
        inventory(assets)? == before,
        "Save silently accepted or wrote a pending draft",
    )?;
    check(
        c.node(&draft.id)?.value() == "NaN",
        "Save lost the native draft",
    )?;
    c.input(draft.window()?, json!({"kind":"key","key":"escape"}))?;
    c.until("cancel draft before Save", || {
        Ok(c.node(&draft.id)?.value() == original)
    })?;
    c.click(&toggle)?;
    c.click(&field.id)?;
    // Cmd/Ctrl+S must work while the clean accepted field still owns keyboard focus.
    c.input(field.window()?, json!({"kind":"key","key":"save"}))?;
    c.until("source Save acknowledged", || clean(c))?;
    let saved = std::fs::read_to_string(&path)?;
    check(
        saved.contains("Saved through Luau"),
        "Save did not write the accepted source",
    )?;
    let saved_inventory = inventory(assets)?;
    check(
        saved_inventory.keys().eq(before.keys()),
        "Save created a source/catalog/sidecar",
    )?;
    for (path, hash) in &before {
        if path != Path::new("Hud.prefab.ron") {
            check(
                saved_inventory.get(path) == Some(hash),
                "Save changed another source or sidecar",
            )?;
        }
    }
    c.frame(field.window()?, "Source saved through Luau")?;
    c.menu(inspector, "Undo source edit")?;
    c.until("saved source Undo is dirty", || {
        Ok(c.current_document()?["dirty"] == true)
    })?;
    c.menu(inspector, "Discard all unsaved source changes")?;
    c.until("discard restores saved baseline", || {
        Ok(input(c, prefix)?.value() == "Saved through Luau")
    })?;
    check(
        std::fs::read_to_string(&path)? == saved,
        "Discard rewrote the saved file",
    )?;
    let field = input(c, prefix)?;
    accept(c, &field, "Working change retained after conflict")?;
    let external = format!("{saved}\n// External writer changed these exact bytes.\n");
    std::fs::write(&path, &external)?;
    c.menu(inspector, "Save active document")?;
    c.until("external source conflict", || {
        Ok(c.state()?["documents"]["save_error"]
            .as_str()
            .is_some_and(|error| error.contains("Save refused")))
    })?;
    check(
        std::fs::read_to_string(&path)? == external,
        "Save overwrote an external change",
    )?;
    check(
        c.node(&field.id)?.value() == "Working change retained after conflict"
            && c.current_document()?["dirty"] == true,
        "Save conflict lost working changes",
    )?;
    // The external test actor explicitly restores the admitted file. The editor
    // offers no implicit overwrite/reconciliation decision and still holds the draft.
    std::fs::write(&path, &saved)?;
    c.menu(inspector, "Save active document")?;
    c.until("explicit Save after conflict resolution", || clean(c))?;
    check(
        std::fs::read_to_string(&path)?.contains("Working change retained after conflict"),
        "Resolved Save omitted accepted changes",
    )?;
    scene::ready(c, &preview)?;
    c.frame(field.window()?, "Source Save after external conflict")?;
    Ok(())
}
