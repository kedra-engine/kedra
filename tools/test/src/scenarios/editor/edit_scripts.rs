//! Explicit project-script launch, common source history and native group Save input.
use super::support::*;
use kedra_test::Result;
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn run(c: &Editor<'_>) -> Result<Value> {
    let project = c.project.ok_or("Missing script project")?;
    let assets = project.join("assets");
    let canonical = c.root.join("tests/fixtures/test_project/assets");
    let before = crate::project::inventory(&canonical)?;
    c.until("explicit editor script completion", || {
        let state = c.state()?;
        if state["editor_script"]["state"] == "failed" {
            return Err(format!("Editor script failed: {}", state["editor_script"]).into());
        }
        Ok(state["editor_script"]["state"] == "completed")
    })?;
    check(
        crate::project::inventory(&assets)? == before,
        "Script commit wrote source files before Save",
    )?;
    let inspector = c.view("inspector")?;
    c.until("script publication", || {
        Ok(c.state()?["documents"]["can_undo"] == true)
    })?;
    c.frame(c.window()?, "Explicit two-document script commit")?;
    c.menu(&inspector, "Undo source edit")?;
    c.until("common batch Undo", || {
        let state = c.state()?;
        Ok(state["documents"]["can_undo"] == false && state["documents"]["can_redo"] == true)
    })?;
    check(
        c.current_document()?["dirty"] == false,
        "Undo to admitted content remained dirty",
    )?;
    c.menu(&inspector, "Redo source edit")?;
    c.until("common batch Redo", || {
        Ok(c.current_document()?["dirty"] == true)
    })?;
    c.input(c.window()?, json!({"kind":"key","key":"save"}))?;
    c.until("native group Save receipt", || {
        let state = c.state()?;
        Ok(c.current_document()?["dirty"] == false
            && state["documents"]["saving"] == false
            && state["documents"]["save_pending"] == false)
    })?;
    let after = crate::project::inventory(&assets)?;
    check(
        after.keys().eq(before.keys()),
        "Batch Save changed source inventory",
    )?;
    for (path, digest) in &before {
        if !matches!(path.to_str(), Some("Hud.prefab.ron" | "Cuboid.prefab.ron")) {
            check(
                after.get(path) == Some(digest),
                "Batch Save changed an ungranted file or sidecar",
            )?;
        }
    }
    for (path, name) in [
        ("Hud.prefab.ron", "Script HUD 入口"),
        ("Cuboid.prefab.ron", "Script cuboid"),
    ] {
        check(
            std::fs::read_to_string(assets.join(path))?.contains(name),
            "Batch Save omitted a committed member",
        )?;
        check(
            after.get(Path::new(path)) != before.get(Path::new(path)),
            "Batch member bytes did not change",
        )?;
    }
    c.frame(c.window()?, "Two-document Save acknowledged")?;
    c.menu(&inspector, "Undo source edit")?;
    c.until("saved group Undo", || {
        Ok(c.current_document()?["dirty"] == true)
    })?;
    check(
        crate::project::inventory(&assets)? == after,
        "Undo wrote saved files",
    )?;
    c.menu(&inspector, "Redo source edit")?;
    c.until("Redo to saved checkpoint", || {
        Ok(c.current_document()?["dirty"] == false)
    })?;
    check(
        c.state()?["editor_script"]["state"] == "completed",
        "History restarted script execution",
    )?;
    Ok(json!({"status":"passed", "frames":c.captures.borrow().clone()}))
}
