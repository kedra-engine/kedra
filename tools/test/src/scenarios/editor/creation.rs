//! Small assembled boundary: editor name entry, shared history and explicit Save.
use super::support::*;
use kedra_test::Result;
use serde_json::{Value, json};
use std::fs;

fn begin(c: &Editor<'_>, index: usize) -> Result<()> {
    let viewport = c.find("Assets viewport", |n| {
        n.id.starts_with("browser/") && n.id.ends_with("/entries")
    })?;
    c.input(viewport.window()?, json!({"kind":"click", "position":[viewport.rect[2] - 8., viewport.rect[3] - 8.], "button":"right"}))?;
    c.click("folder-menu/0")?;
    c.click(&format!("folder-menu/0/submenu/{index}"))?;
    c.wait_node("document-create/name")?;
    Ok(())
}
pub(super) fn run(c: &Editor<'_>) -> Result<Value> {
    let project = c.open_project()?;
    let assets = project.join("assets");
    let before = crate::project::inventory(&assets)?;
    super::thumbnails::run(c, &project)?;
    begin(c, 0)?;
    c.field("document-create/name", "Cancelled")?;
    c.click("document-create/Cancel")?;
    c.until("name form cancelled", || {
        Ok(!c.present("document-create/name")?)
    })?;
    check(
        crate::project::inventory(&assets)? == before,
        "Cancel changed sources",
    )?;
    for (index, name, suffix, kind) in [
        (0, "NativeArena", "scene", "Scene"),
        (1, "NativeBomb", "prefab", "Prefab"),
    ] {
        begin(c, index)?;
        c.field("document-create/name", "../Escape")?;
        c.click("document-create/Create")?;
        c.until("invalid name retained", || {
            Ok(c.nodes()?
                .iter()
                .any(|n| n.id.contains("document-create.error") && !n.caption().is_empty()))
        })?;
        c.field("document-create/name", name)?;
        c.click("document-create/Create")?;
        c.until("created source tab", || {
            Ok(!c.present("document-create/name")?
                && c.current_document().is_ok_and(|d| d["dirty"] == false))
        })?;
        let path = assets.join(format!("{name}.{suffix}.ron"));
        check(
            path.is_file(),
            "Create opened a tab before publishing the source file",
        )?;
        let created = fs::read(&path)?;
        let identity = c.current_document()?["document"].clone();
        check(identity.is_string(), "Missing document identity")?;
        let empty_view = c.current_document()?["view"]
            .as_str()
            .ok_or("Created view missing")?
            .to_owned();
        let prefix = format!("panel/ViewId({empty_view})/");
        super::scene::ready(c, &prefix)?;
        let lighting = c.node(&format!("{prefix}settings"))?;
        check(
            lighting.enabled,
            "Editor Light unavailable on an empty created document",
        )?;
        let initial = c.state()?["scene_views"][&empty_view]["editor_lighting"].clone();
        c.click(&lighting.id)?;
        c.wait_node("scene-menu/0")?;
        c.click(if initial == true {
            "scene-menu/4"
        } else {
            "scene-menu/5"
        })?;
        c.until("empty document editor lighting changed", || {
            Ok(c.state()?["scene_views"][&empty_view]["editor_lighting"] != initial)
        })?;
        super::scene::ready(c, &prefix)?;
        c.click(&lighting.id)?;
        c.wait_node("scene-menu/0")?;
        c.click(if initial == true {
            "scene-menu/5"
        } else {
            "scene-menu/4"
        })?;
        c.until("empty document editor lighting restored", || {
            Ok(c.state()?["scene_views"][&empty_view]["editor_lighting"] == initial)
        })?;
        super::scene::ready(c, &prefix)?;
        let inspector = c.view("inspector")?;
        c.main_menu("Undo")?;
        c.until("creation undone", || {
            Ok(c.state()?["documents"]["can_redo"] == true)
        })?;
        check(
            fs::read(&path)? == created,
            "Undo changed the published file without Save",
        )?;
        c.main_menu("Redo")?;
        c.until("creation restored", || {
            Ok(array(&c.state()?["documents"]["documents"])?
                .iter()
                .any(|d| d["document"] == identity && d["dirty"] == false))
        })?;
        c.click(&format!("dock/view/ViewId({empty_view})/title"))?;
        c.until("restored document activated", || {
            Ok(c.current_document()
                .is_ok_and(|d| d["document"] == identity))
        })?;
        check(
            fs::read(&path)? == created,
            "Redo changed the already published source",
        )?;
        c.until("created document saved", || {
            Ok(path.exists() && c.current_document()?["dirty"] == false)
        })?;
        let saved = fs::read_to_string(&path)?;
        check(saved.contains(kind), "Save lost document kind")?;
        c.frame(c.window()?, &format!("{kind} created and saved"))?;
        c.hierarchy_menu(None, "Create/3D Object/Cube")?;
        c.until("cube selected in source hierarchy", || {
            Ok(c.current_document()?["dirty"] == true
                && c.nodes()?
                    .iter()
                    .any(|n| n.id.starts_with("hierarchy/") && n.caption() == "Cube"))
        })?;
        let view = c.current_document()?["view"]
            .as_str()
            .ok_or("Document view missing")?
            .to_owned();
        super::scene::ready(c, &format!("panel/ViewId({view})/"))?;
        c.main_menu("Undo")?;
        c.until("cube creation undone", || {
            Ok(c.current_document()?["dirty"] == false)
        })?;
        check(
            fs::read_to_string(&path)? == saved,
            "Object Undo changed saved bytes",
        )?;
        c.main_menu("Redo")?;
        c.until("cube restored", || {
            Ok(c.current_document()?["dirty"] == true)
        })?;
        super::scene::ready(c, &format!("panel/ViewId({view})/"))?;
        c.key("save")?;
        c.until("cube saved", || Ok(c.current_document()?["dirty"] == false))?;
        check(
            fs::read_to_string(&path)?.contains("Cube"),
            "Save omitted created object",
        )?;
        c.frame(c.window()?, &format!("{kind} cube after Redo and Save"))?;
        for (menu, key) in [
            ("Remove component", "scene.surface"),
            ("Add component", "scene.surface"),
        ] {
            if menu.starts_with("Add") {
                c.click(&format!("inspector/ViewId({inspector})/add"))?;
            } else {
                let action = c.find("Component section menu", |node| {
                    node.id.starts_with("inspector/") && node.id.ends_with(&format!("/{key}/menu"))
                })?;
                c.click(&action.id)?;
            }
            let choice = c.find("Component schema", |node| {
                node.id.starts_with("component-menu/")
                    && node.caption()
                        == if menu.starts_with("Add") {
                            key
                        } else {
                            "Remove component"
                        }
            })?;
            c.click(choice.id.strip_suffix("/caption").unwrap_or(&choice.id))?;
            c.until("component source edit", || {
                Ok(c.current_document()?["dirty"] == true)
            })?;
            c.key("save")?;
            c.until("component saved", || {
                Ok(c.current_document()?["dirty"] == false)
            })?;
            c.until("effective component presentation", || {
                Ok(c.nodes()?.iter().any(|node| {
                    node.id.starts_with("inspector/") && node.id.contains(&format!("/{key}/"))
                }) == menu.starts_with("Add"))
            })?;
            if !menu.starts_with("Add") {
                c.until("missing Surface projection diagnostic", || {
                    Ok(c.optional(&format!("panel/ViewId({view})/information"))?
                        .is_some_and(|node| node.caption().contains("Primitive has no surface")))
                })?;
                check(
                    c.state()?["scene_views"][&view]["source_transforms"].is_null(),
                    "Failed projection retained source entities",
                )?;
            }
        }
        if index == 0 {
            super::scene::ready(c, &format!("panel/ViewId({view})/"))?;
            let search = c.find("Browser search", |node| {
                node.id.starts_with("browser/") && node.id.ends_with("/search")
            })?;
            c.field(&search.id, "marker.glb")?;
            let model = c.find("Model asset", |node| {
                node.id.starts_with("browser/")
                    && node.id.ends_with("/name")
                    && node.caption() == "marker.glb"
            })?;
            let parent = c.find("Model parent", |node| {
                node.id.starts_with("hierarchy/") && node.caption() == "Cube"
            })?;
            let window = model.window()?;
            c.input(window, json!({"kind":"move", "position":model.center()}))?;
            c.input(window, json!({"kind":"button", "pressed":true}))?;
            c.input(
                window,
                json!({"kind":"move", "position":[model.center()[0]+8., model.center()[1]]}),
            )?;
            c.input(window, json!({"kind":"move", "position":parent.center()}))?;
            c.input(window, json!({"kind":"move", "position":parent.center()}))?;
            c.release(window)?;
            c.until("model placed", || {
                Ok(c.current_document()
                    .is_ok_and(|document| document["dirty"] == true))
            })?;
            super::scene::ready(c, &format!("panel/ViewId({view})/"))?;
            c.key("save")?;
            c.until(
                "model saved",
                || Ok(c.current_document()?["dirty"] == false),
            )?;
            check(
                fs::read_to_string(&path)?.contains("game://marker.glb#"),
                "Placed model lost logical source reference",
            )?;
            let viewport = c.wait_node(&format!("panel/ViewId({view})/viewport"))?;
            c.input(
                window,
                json!({"kind":"drag", "from":model.center(), "to":viewport.center()}),
            )?;
            c.until("model placed through Scene View", || {
                Ok(c.current_document()?["dirty"] == true)
            })?;
            super::scene::ready(c, &format!("panel/ViewId({view})/"))?;
            c.key("save")?;
            c.until("Scene View placement saved", || {
                Ok(c.current_document()?["dirty"] == false)
            })?;
            check(
                fs::read_to_string(&path)?
                    .matches("game://marker.glb#")
                    .count()
                    == 2,
                "Scene View drop did not retain both placed model references",
            )?;
            c.field(&search.id, "")?;
            c.frame(window, "Placed native model")?;
        }
    }
    // Saved authoring documents may differ from the launch delivery; unchanged
    // resource thumbnails must still be admitted after a fresh project lifetime.
    super::thumbnails::run(c, &project)?;
    // Empty sources must restore without a document member in the launch delivery.
    begin(c, 0)?;
    c.field("document-create/name", "RestoredEmpty")?;
    c.click("document-create/Create")?;
    c.until("empty source published", || {
        Ok(!c.present("document-create/name")?
            && c.current_document()
                .is_ok_and(|d| d["path"] == "RestoredEmpty.scene.ron" && d["dirty"] == false))
    })?;
    super::thumbnails::run(c, &project)?;
    // An inactive persisted descriptor admits its source on tab selection too.
    let state = c.state()?;
    let arena = object(&state["layout"]["views"])?
        .iter()
        .find(|(_, view)| view["restore_key"] == "NativeArena.scene.ron")
        .map(|(id, _)| id)
        .ok_or("Persisted arena tab missing")?;
    c.click(&Editor::label(arena))?;
    c.until("inactive restored source admitted", || {
        Ok(c.current_document()
            .is_ok_and(|d| d["path"] == "NativeArena.scene.ron"))
    })?;
    let restored = c.current_document()?;
    super::scene::ready(c, &format!("panel/ViewId({})/", text(&restored["view"])?))?;
    for (path, hash) in before {
        check(
            kedra_test::evidence::digest(&fs::read(assets.join(path))?) == hash,
            "Creation changed an existing source",
        )?;
    }
    Ok(json!({"status":"passed", "frames":c.captures.borrow().clone()}))
}
