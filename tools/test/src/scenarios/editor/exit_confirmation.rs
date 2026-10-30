//! Dirty native close requests require a visible choice and preserve sources on Cancel.
use super::support::*;
use crate::project::inventory;
use kedra_test::{Result, process::NativeProcess};
use serde_json::{Value, json};

pub(super) fn run(c: &Editor<'_>) -> Result<Value> {
    let project = c.open_project()?;
    let search = c.find("Assets search", |node| {
        node.id.starts_with("browser/") && node.id.ends_with("/search")
    })?;
    c.field(&search.id, "Cuboid.prefab.ron")?;
    c.open_asset("Cuboid.prefab.ron")?;
    c.until("source admitted", || Ok(c.current_document().is_ok()))?;
    let before = inventory(&project.join("assets"))?;
    c.hierarchy_menu(None, "Create/Empty Node")?;
    c.until("unsaved node created", || {
        Ok(c.current_document()?["dirty"] == true)
    })?;
    let window = c.window()?;
    c.input(window, json!({"kind":"close_window"}))?;
    for choice in ["save", "discard", "cancel"] {
        let button = c.wait_node(&format!("close-dialog/{choice}"))?;
        check(
            button.enabled,
            "Exit choice unavailable for an accepted source edit",
        )?;
    }
    c.frame(window, "Unsaved source exit confirmation")?;
    c.click("close-dialog/cancel")?;
    c.until("exit cancelled", || Ok(!c.present("close-dialog/cancel")?))?;
    check(
        c.current_document()?["dirty"] == true,
        "Cancel lost the working edit",
    )?;
    check(
        inventory(&project.join("assets"))? == before,
        "Cancel wrote source files",
    )?;
    c.input(window, json!({"kind":"close_window"}))?;
    c.wait_node("close-dialog/save")?;
    Ok(json!({"status":"passed", "frames":c.captures.borrow().clone()}))
}

pub(super) fn finish(c: &Editor<'_>, native: NativeProcess, save: bool) -> Result<()> {
    let project = c.open_project()?;
    let assets = project.join("assets");
    let before = inventory(&assets)?;
    let id = if save {
        "close-dialog/save"
    } else {
        "close-dialog/discard"
    };
    let button = c.wait_node(id)?;
    let window = button.window()?;
    native.finish(|| {
        // Successful process exit is the acknowledgment when transport retires with the window.
        let _ = c.input(window, json!({"kind":"click", "element":id}));
        Ok(())
    })?;
    let after = inventory(&assets)?;
    if save {
        check(
            std::fs::read_to_string(assets.join("Cuboid.prefab.ron"))?.contains("Object"),
            "Save and exit omitted the new node",
        )?;
        check(before != after, "Save and exit left the source unchanged")?;
        for (path, hash) in &before {
            if path != std::path::Path::new("Cuboid.prefab.ron") {
                check(
                    after.get(path) == Some(hash),
                    "Exit Save changed another source",
                )?;
            }
        }
    } else {
        check(before == after, "Exit without saving wrote source files")?;
    }
    Ok(())
}
