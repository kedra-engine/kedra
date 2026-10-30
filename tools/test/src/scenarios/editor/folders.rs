//! Native folder name form and browser menu; OS deletion confirmation is manual.
use super::support::*;
use kedra_test::Result;
use serde_json::{Value, json};

#[path = "tile_names.rs"]
mod tile_names;

pub(super) fn run(c: &Editor<'_>) -> Result<Value> {
    let project = c.open_project()?;
    let assets = project.join("assets");
    let browser = c.view("assets")?;
    tile_names::run(c, &browser)?;
    let search = c.find("Browser search", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/search")
    })?;
    c.field(&search.id, "NoMatchingAssetForContextMenu")?;
    let viewport = c.find("Assets viewport", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/entries")
    })?;
    let point = [viewport.rect[0] + 30., viewport.rect[1] + 30.];
    c.input(
        viewport.window()?,
        json!({"kind":"click","position":point,"button":"right"}),
    )?;
    let popup = c.wait_node("dock-menu")?;
    near(popup.rect[0], point[0], 2.)?;
    check(
        (popup.rect[1] - point[1]).abs() <= 2. || (popup.rect[3] - point[1]).abs() <= 2.,
        "Context menu is detached from the pointer",
    )?;
    let item = c.wait_node("folder-menu/0")?;
    check(
        item.rect[0] >= popup.rect[0]
            && item.rect[2] <= popup.rect[2]
            && item.rect[1] >= popup.rect[1]
            && item.rect[3] <= popup.rect[3],
        "Context menu item escaped the surface",
    )?;
    c.frame(c.window()?, "empty-space context menu")?;
    c.key("right")?;
    c.wait_node("folder-menu/0/submenu/0")?;
    check(
        c.present("dock-menu")?,
        "Opening a submenu closed its parent",
    )?;
    c.key("left")?;
    c.until("Left returns to parent", || {
        Ok(!c.present("folder-menu/0/submenu")? && c.node("folder-menu/0")?.focused)
    })?;
    c.input(
        viewport.window()?,
        json!({"kind":"move","position":item.center()}),
    )?;
    c.wait_node("folder-menu/0/submenu/0")?;
    c.key("escape")?;
    c.until("Escape closes one menu level", || {
        Ok(!c.present("folder-menu/0/submenu")? && c.present("dock-menu")?)
    })?;
    c.key("escape")?;
    c.until("context menu dismissed", || Ok(!c.present("dock-menu")?))?;
    c.menu(&browser, "Maximize group")?;
    let viewport = c.find("maximized Assets viewport", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/entries")
    })?;
    let point = [viewport.rect[2] - 8., viewport.rect[3] - 8.];
    c.input(
        viewport.window()?,
        json!({"kind":"click","position":point,"button":"right"}),
    )?;
    let popup = c.wait_node("dock-menu")?;
    check(
        popup.rect[0] >= 0. && popup.rect[1] >= 0. && popup.rect[2] <= point[0] + 2.,
        "Edge context menu did not flip left",
    )?;
    check(
        (popup.rect[1] - point[1]).abs() <= 2. || (popup.rect[3] - point[1]).abs() <= 2.,
        "Edge context menu detached from pointer",
    )?;
    c.key("right")?;
    let submenu = c.wait_node("folder-menu/0/submenu")?;
    check(
        submenu.rect[2] <= c.node("folder-menu/0")?.rect[0] + 1.,
        "Submenu failed to flip left at the window edge",
    )?;
    c.frame(c.window()?, "submenu flipped at window edge")?;
    c.key("escape")?;
    c.key("escape")?;
    c.menu(&browser, "Restore group")?;
    c.field(&search.id, "")?;
    c.menu(&browser, "Create")?;
    let folder = c.find("Create folder", |n| {
        n.id.ends_with("/submenu/2/caption") && n.caption() == "Folder"
    })?;
    c.click(&folder.id)?;
    c.wait_node("document-create/name")?;
    c.field("document-create/name", "CancelledFolder")?;
    c.click("document-create/Cancel")?;
    c.until("folder form cancelled", || {
        Ok(!c.present("document-create/name")?)
    })?;
    check(
        !assets.join("CancelledFolder").exists(),
        "Cancelled folder was created",
    )?;
    c.menu(&browser, "Create")?;
    let folder = c.find("Create folder", |n| {
        n.id.ends_with("/submenu/2/caption") && n.caption() == "Folder"
    })?;
    c.click(&folder.id)?;
    c.wait_node("document-create/name")?;
    c.field("document-create/name", "../Escape")?;
    c.click("document-create/Create")?;
    c.until("invalid folder name retained", || {
        Ok(c.nodes()?
            .iter()
            .any(|n| n.id.contains("document-create.error") && !n.caption().is_empty()))
    })?;
    check(
        !project.join("Escape").exists(),
        "Invalid folder escaped assets",
    )?;
    c.field("document-create/name", "NativeFolder")?;
    c.click("document-create/Create")?;
    c.until("folder created", || {
        Ok(assets.join("NativeFolder").is_dir() && !c.present("document-create/name")?)
    })?;
    let search = c.find("Browser search", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/search")
    })?;
    c.field(&search.id, "NativeFolder")?;
    c.until("created folder visible", || {
        Ok(c.nodes()?
            .iter()
            .any(|n| n.id.starts_with("browser/") && n.caption() == "NativeFolder"))
    })?;
    c.browser_select("NativeFolder")?;
    c.menu(&browser, "Rename")?;
    c.wait_node("document-create/name")?;
    c.until("rename name populated", || {
        Ok(c.node("document-create/name")?.value() == "NativeFolder")
    })?;
    c.field("document-create/name", "Prefabs")?;
    c.click("document-create/Rename")?;
    c.until("conflicting rename refused", || {
        Ok(!c.present("document-create/name")?)
    })?;
    c.until("rename refusal status", || {
        Ok(c.nodes()?
            .iter()
            .any(|n| n.caption().contains("already exists")))
    })?;
    check(
        assets.join("NativeFolder").is_dir() && assets.join("Prefabs").is_dir(),
        "Conflicting rename changed folders",
    )?;
    c.menu(&browser, "Rename")?;
    c.wait_node("document-create/name")?;
    c.field("document-create/name", "RenamedFolder")?;
    c.click("document-create/Rename")?;
    c.until("folder renamed", || {
        Ok(assets.join("RenamedFolder").is_dir() && !assets.join("NativeFolder").exists())
    })?;
    c.field(&search.id, "RenamedFolder")?;
    c.until("renamed folder visible", || {
        Ok(c.nodes()?
            .iter()
            .any(|n| n.id.starts_with("browser/") && n.caption() == "RenamedFolder"))
    })?;
    c.frame(c.window()?, "renamed asset folder")?;
    c.field(&search.id, "")?;
    let source = c.find("source folder", |n| {
        n.caption() == "RenamedFolder" && n.id.contains("/folder/")
    })?;
    let target = c.find("destination folder", |n| {
        n.caption() == "Prefabs" && n.id.contains("/folder/")
    })?;
    c.move_to(source.center())?;
    c.held(true)?;
    c.move_to([source.center()[0] + 12., source.center()[1]])?;
    c.move_to(target.center())?;
    c.held(false)?;
    c.until("tree folder move", || {
        Ok(assets.join("Prefabs/RenamedFolder").is_dir() && !assets.join("RenamedFolder").exists())
    })?;
    c.frame(c.window()?, "folder moved through tree")?;
    Ok(
        json!({"create":true,"cancel":true,"invalid_name":true,"rename":true,"tree_move":true,"collision_preserves_folders":true,"native_delete_confirmation":"manual"}),
    )
}
