//! Real PNG/GLB/UI/mixed preview, retained cameras and source non-mutation.
use super::support::*;
use crate::project::inventory;
use kedra_test::Result;
use serde_json::{Value, json};
use std::path::Path;

fn open_source(c: &Editor<'_>, name: &str) -> Result<String> {
    let search = c.find("browser search", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/search")
    })?;
    c.field(&search.id, name)?;
    c.browser_select(name)?;
    c.menu(&c.view("assets")?, "Open selected source")?;
    let s = c.client.wait(
        "scene source view",
        std::time::Duration::from_secs(40),
        || {
            Ok(object(&c.state()?["layout"]["views"])?
                .iter()
                .find(|(_, v)| v["restore_key"] == name)
                .map(|(id, _)| id.clone()))
        },
    )?;
    ready(c, &format!("panel/ViewId({s})/"))?;
    check(
        !c.nodes()?.iter().any(|n| {
            n.id.starts_with("browser/") && n.caption().contains("Opening read-only asset preview")
        }),
        "Successful Open left browser loading",
    )?;
    Ok(s)
}
pub(super) fn ready(c: &Editor<'_>, p: &str) -> Result<()> {
    c.until("native scene renderer", || {
        if let Some(n) = c.optional(&format!("{p}information"))? {
            check(
                ![
                    "failed",
                    "mismatch",
                    "does not render",
                    "requires",
                    "unavailable",
                ]
                .iter()
                .any(|word| n.caption().contains(word)),
                format!("Scene readiness rejected: {}", n.caption()),
            )?;
            return Ok(false);
        }
        c.present(&format!("{p}render"))
    })
}
pub(super) fn record(c: &Editor<'_>, name: &str, p: &str) -> Result<Value> {
    let image = c.node(&format!("{p}render"))?;
    let window = image.window()?;
    let observation = c.client.observe()?;
    let w = array(&observation["windows"])?
        .iter()
        .find(|w| w["id"] == window)
        .ok_or("Missing preview host")?;
    Ok(json!({"case":name,"frame":c.frame(window,name)?,"rect":image.rect,"dpr":w["dpr"]}))
}

pub(super) fn run(c: &Editor<'_>, name: &str, project: &Path) -> Result<Value> {
    c.until("serving editor", || {
        Ok(c.rpc("editor.state")?["phase"] == "editor" && c.state()?["bound"] == true)
    })?;
    if name == "scene-navigation" {
        let ship = open_source(c, "ship.glb")?;
        let ship_prefix = format!("panel/ViewId({ship})/");
        let ship_frame = record(c, "ship.glb", &ship_prefix)?;
        let view = open_source(c, "Composed.scene.ron")?;
        let prefix = format!("panel/ViewId({view})/");
        let composed_frame = record(c, "Composed.scene.ron", &prefix)?;
        let mut data = super::navigation::run(c, project, &view, &ship)?;
        data["setup_frames"] = json!([ship_frame, composed_frame]);
        return Ok(data);
    }
    let before = inventory(&project.join("assets"))?;
    let mut frames = Vec::new();
    let mut views = std::collections::BTreeMap::new();
    for name in [
        "checker.png",
        "ship.glb",
        "Hud.prefab.ron",
        "Composed.scene.ron",
    ] {
        let view = open_source(c, name)?;
        let prefix = format!("panel/ViewId({view})/");
        frames.push(record(c, name, &prefix)?);
        check(
            !c.nodes()?
                .iter()
                .any(|n| n.id.starts_with("preview/") || n.caption() == "SCENE / SYSTEM CHECK"),
            "Projected UI leaked serving observation identities",
        )?;
        if name.ends_with(".png") || name.ends_with(".glb") {
            check(
                c.state()?["documents"]["active"].is_null(),
                "Raw preview invented source authority",
            )?;
        }
        views.insert(name.to_string(), view);
    }
    let view = &views["Composed.scene.ron"];
    let prefix = format!("panel/ViewId({view})/");
    let documents = c.state()?["documents"].clone();
    c.menu(view, "Float tab")?;
    c.until("floating preview", || {
        Ok(c.node(&format!("{prefix}render"))?.window()? != c.window()?)
    })?;
    let window = c.node(&format!("{prefix}render"))?.window()?;
    c.resize(window, 640, 480, 2)?;
    ready(c, &prefix)?;
    frames.push(record(c, "mixed-float", &prefix)?);
    c.menu(view, "Redock to main window")?;
    c.until("redocked preview", || {
        Ok(c.node(&format!("{prefix}render"))?.window()? == c.window()?)
    })?;
    check(
        c.state()?["documents"] == documents,
        "Host transfer changed source documents",
    )?;
    c.click(&Editor::label(&views["Hud.prefab.ron"]))?;
    c.click(&Editor::label(&c.view("assets")?))?;
    let search = c.find("serving browser search", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/search")
    })?;
    c.field(&search.id, "ship")?;
    check(
        inventory(&project.join("assets"))? == before,
        "Read-only preview changed source bytes/sidecars",
    )?;
    c.click(&search.id)?;
    c.key("select_all")?;
    c.key("backspace")?;
    c.click(&Editor::label(view))?;
    ready(c, &prefix)?;
    frames.push(record(c, "mixed-restored", &prefix)?);
    Ok(
        json!({"status":"passed","project":project,"frames":frames,"limitations":["Injected Bevy input, not physical OS input"]}),
    )
}
