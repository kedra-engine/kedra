//! Selecting a source asset exposes its reflected root without opening a scene tab.
use super::support::*;
use kedra_test::Result;
use serde_json::{Value, json};
pub(super) fn run(c: &Editor<'_>) -> Result<Value> {
    let project = c.open_project()?;
    let path = project.join("assets/day.environment.ron");
    let before = std::fs::read_to_string(&path)?;
    let metadata = std::fs::read(path.with_extension("ron.meta"))?;
    let search = c.find("Assets search", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/search")
    })?;
    c.field(&search.id, "day.environment.ron")?;
    c.browser_select("day.environment.ron")?;
    let brightness = c.find("Environment brightness in Inspector", |n| {
        n.id.starts_with("inspector/") && n.id.contains("/environment/") && n.value() == "20"
    })?;
    c.frame(c.window()?, "Asset root in ordinary Inspector")?;
    c.field(&brightness.id, "-1")?;
    c.key("enter")?;
    c.until("invalid environment rejected", || {
        Ok(c.nodes()?
            .iter()
            .any(|n| n.visible && n.caption().contains("ambient brightness")))
    })?;
    check(
        std::fs::read_to_string(&path)? == before,
        "Invalid asset field changed source bytes",
    )?;
    c.key("escape")?;
    c.field(&brightness.id, "450")?;
    c.key("enter")?;
    c.until("environment source draft accepted", || {
        Ok(c.state()?["documents"]["save_available"] == true)
    })?;
    for (old, new) in [("#242B34", "#112233"), ("#FFFFFF", "#CCDDEE")] {
        let color = c.find("Environment color", |n| {
            n.id.starts_with("inspector/") && n.id.contains("/environment/") && n.value() == old
        })?;
        c.field(&color.id, new)?;
        c.key("enter")?;
    }
    check(
        std::fs::read_to_string(&path)? == before,
        "Environment edited the filesystem before Save",
    )?;
    c.key("save")?;
    c.until("environment saved", || {
        Ok(c.state()?["documents"]["save_available"] == false
            && std::fs::read_to_string(&path)?.contains("450"))
    })?;
    check(
        std::fs::read(path.with_extension("ron.meta"))? == metadata,
        "Environment Save changed persistent identity",
    )?;
    c.frame(c.window()?, "Saved environment root")?;
    c.field(&search.id, "Composed.scene.ron")?;
    c.open_asset("Composed.scene.ron")?;
    c.until("scene admitted", || Ok(c.current_document().is_ok()))?;
    let view = c.current_document()?["view"]
        .as_str()
        .ok_or("Scene view unavailable")?
        .to_owned();
    let prefix = format!("panel/ViewId({view})/");
    super::scene::ready(c, &prefix)?;
    c.click(&format!("{prefix}settings"))?;
    let light = c.find("Scene lighting command", |n| {
        n.id.starts_with("scene-menu/") && n.caption() == "Scene lighting"
    })?;
    c.click(
        light
            .id
            .strip_suffix("/caption")
            .ok_or("Lighting caption")?,
    )?;
    c.until("saved source environment reaches camera", || {
        Ok(c.state()?["scene_views"][&view]["ambient_brightness"] == 450.0)
    })?;
    c.field(&search.id, "day.environment.ron")?;
    c.browser_select("day.environment.ron")?;
    let brightness = c.find("Environment brightness after scene selection", |n| {
        n.id.starts_with("inspector/") && n.id.contains("/environment/") && n.value() == "450"
    })?;
    c.field(&brightness.id, "300")?;
    c.key("enter")?;
    c.until("working environment reaches existing camera", || {
        Ok(c.state()?["scene_views"][&view]["ambient_brightness"] == 300.0)
    })?;
    c.frame(
        c.window()?,
        "Environment working value refreshes Scene View",
    )?;
    c.main_menu("Undo")?;
    c.until("environment Undo refreshes camera", || {
        Ok(c.state()?["scene_views"][&view]["ambient_brightness"] == 450.0)
    })?;
    c.field(&search.id, "")?;
    create(c, "Environment", "Evening")?;
    let created = project.join("assets/Evening.environment.ron");
    let brightness = c.find("New environment selected in Inspector", |n| {
        n.id.starts_with("inspector/") && n.id.contains("/environment/") && n.value() == "100"
    })?;
    check(
        created.is_file() && created.with_extension("ron.meta").is_file(),
        "Environment selection preceded file and identity publication",
    )?;
    c.field(&brightness.id, "275")?;
    c.key("enter")?;
    c.until("new environment dirty", || {
        Ok(c.state()?["documents"]["save_available"] == true)
    })?;
    c.key("save")?;
    c.until("new environment saved", || {
        Ok(c.state()?["documents"]["save_available"] == false
            && std::fs::read_to_string(&created)?.contains("275"))
    })?;
    create(c, "Scene", "LightingScene")?;
    c.click(&Editor::label(&view))?;
    c.field(&search.id, "LightingScene.scene.ron")?;
    c.browser_select("LightingScene.scene.ron")?;
    let picker = c.find("Empty scene Environment picker", |n| {
        n.id.starts_with("inspector/")
            && n.id.contains("/environment-assignment/")
            && n.id.ends_with("/picker")
    })?;
    let scene_path = project.join("assets/LightingScene.scene.ron");
    let scene_before = std::fs::read_to_string(&scene_path)?;
    c.click(&picker.id)?;
    let choice = c.find("New environment choice without rebuilding delivery", |n| {
        n.caption() == "game://Evening.environment.ron" && n.id.contains("/menu/")
    })?;
    c.click(choice.id.strip_suffix("/caption").unwrap_or(&choice.id))?;
    c.until("scene environment assigned", || {
        Ok(c.nodes()?.iter().any(|n| {
            n.id.contains("/scene.environment/") && n.value() == "game://Evening.environment.ron"
        }))
    })?;
    check(
        std::fs::read_to_string(&scene_path)? == scene_before,
        "Assignment wrote scene before Save",
    )?;
    c.key("save")?;
    c.until("scene profile reference saved", || {
        Ok(std::fs::read_to_string(&scene_path)?.contains("game://Evening.environment.ron"))
    })?;
    check(
        c.current_document()?["view"] == view,
        "Inspecting the scene asset unexpectedly activated its tab",
    )?;
    c.open_asset("LightingScene.scene.ron")?;
    c.until("assigned scene activated", || {
        Ok(c.current_document()
            .is_ok_and(|document| document["view"] != view))
    })?;
    let assigned_view = c.current_document()?["view"]
        .as_str()
        .ok_or("New scene view missing")?
        .to_owned();
    let prefix = format!("panel/ViewId({assigned_view})/");
    super::scene::ready(c, &prefix)?;
    c.click(&format!("{prefix}settings"))?;
    c.click("scene-menu/4")?;
    c.until("new profile renders in scene", || {
        Ok(c.state()?["scene_views"][&assigned_view]["ambient_brightness"] == 275.0)
    })?;
    c.frame(
        c.window()?,
        "Created environment assigned through scene Inspector",
    )?;
    Ok(json!({"status":"passed", "frames":c.captures.borrow().clone()}))
}

fn create(c: &Editor<'_>, kind: &str, name: &str) -> Result<()> {
    let viewport = c.find("Assets viewport", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/entries")
    })?;
    c.input(viewport.window()?, json!({"kind":"click", "position":[viewport.rect[2]-8.,viewport.rect[3]-8.],"button":"right"}))?;
    c.click("folder-menu/0")?;
    let command = c.find("Create asset command", |n| {
        n.id.starts_with("folder-menu/") && n.id.ends_with("/caption") && n.caption() == kind
    })?;
    c.click(
        command
            .id
            .strip_suffix("/caption")
            .ok_or("Creation command missing")?,
    )?;
    c.wait_node("document-create/name")?;
    c.field("document-create/name", name)?;
    c.click("document-create/Create")?;
    c.until("asset creation completed", || {
        Ok(!c.present("document-create/name")?)
    })
}
