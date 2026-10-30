//! Read-only composed source hierarchy, virtual navigation and responsive usability.
use super::support::*;
use crate::project::inventory;
use kedra_test::Result;
use serde_json::{Value, json};
use std::fs;
use uuid::Uuid;

fn labels(c: &Editor<'_>, prefix: &str, name: &str) -> Result<Vec<Node>> {
    Ok(c.nodes()?
        .into_iter()
        .filter(|n| n.id.starts_with(prefix) && n.caption() == name)
        .collect())
}
fn rows(c: &Editor<'_>, prefix: &str, name: &str) -> Result<Vec<Node>> {
    let labels = labels(c, prefix, name)?;
    Ok(c.nodes()?
        .into_iter()
        .filter(|n| labels.iter().any(|l| l.parent == Some(n.entity)))
        .collect())
}
fn search(c: &Editor<'_>, view: &str, prefix: &str, value: &str) -> Result<()> {
    let id = format!("{prefix}search");
    if !c.present(&id)? {
        c.menu(view, "Search nodes")?;
    }
    c.field(&id, value)
}
fn open(c: &Editor<'_>, name: &str, identity: Uuid) -> Result<()> {
    c.open_asset(name)?;
    c.until("source activation", || {
        Ok(c.state()?["documents"]["active"] == identity.to_string())
    })
}

pub(super) fn run(c: &Editor<'_>, phases: &mut Value) -> Result<Value> {
    *phases = json!({"behavior":{"status":"not_run"},"usability":{"status":"not_run"}});
    for (name, phase) in [
        ("behavior", behavior as fn(&Editor<'_>) -> Result<Value>),
        ("usability", usability as fn(&Editor<'_>) -> Result<Value>),
    ] {
        eprintln!("Native hierarchy phase: {name}");
        let first_frame = c.captures.borrow().len();
        let result = phase(c);
        let frames = c.captures.borrow()[first_frame..].to_vec();
        match result {
            Ok(mut data) => {
                data["frames"] = json!(frames);
                phases[name] = data;
            }
            Err(error) => {
                phases[name] = json!({"status":"failed","error":error.to_string(),"frames":frames});
                return Err(error);
            }
        }
    }
    Ok(json!({"status":"passed","phases":phases,"frames":c.captures.borrow().clone()}))
}

fn behavior(c: &Editor<'_>) -> Result<Value> {
    let project = c.open_project()?;
    let assets = project.join("assets");
    let directory = assets.join("Hierarchy");
    let before = inventory(&assets)?;
    let reference = c.reference("hierarchy")?;
    let identity = |name: &str| -> Result<Uuid> { Ok(Uuid::parse_str(text(&reference[name])?)?) };
    let (main, main_root, other, other_root) = (
        identity("main")?,
        identity("main_root")?,
        identity("other")?,
        identity("other_root")?,
    );
    c.browser_folder("Hierarchy")?;
    let view = c.view("hierarchy")?;
    let prefix = format!("hierarchy/ViewId({view})/");
    c.wait_node(&format!("{prefix}message"))?;
    open(c, "Main.scene.ron", main)?;
    c.until("composed root", || {
        Ok(!labels(c, &prefix, "Main scene")?.is_empty())
    })?;
    check(
        c.selection()? == json!({"instances":[],"node":main_root}),
        "Wrong admitted root selection",
    )?;
    check(
        c.nodes()?
            .iter()
            .filter(|n| n.id.starts_with(&format!("{prefix}node/")))
            .count()
            < 100,
        "Hierarchy is not virtualized",
    )?;
    search(c, &view, &prefix, "STRASSE")?;
    c.until("both scoped leaves", || {
        Ok(rows(c, &prefix, "Straße leaf")?.len() == 2)
    })?;
    let leaves_rows = rows(c, &prefix, "Straße leaf")?;
    c.click(&leaves_rows[0].id)?;
    let one = c.selection()?;
    c.click(&leaves_rows[1].id)?;
    let two = c.selection()?;
    check(
        one["node"] == reference["first_leaf"]
            && two["node"] == one["node"]
            && one["instances"] != two["instances"],
        "Nested source instances lost address identity",
    )?;
    c.modifier("control", true)?;
    let clicked = c.click(&leaves_rows[0].id);
    c.modifier("control", false)?;
    clicked?;
    c.until("shared additive source selection", || {
        let state = c.state()?;
        let items = array(&state["selection"]["items"])?;
        Ok(items.len() == 2
            && items.iter().any(|item| item["Node"]["node"] == one)
            && items.iter().any(|item| item["Node"]["node"] == two))
    })?;
    c.frame(c.window()?, "shared hierarchy multi-selection")?;
    c.modifier("shift", true)?;
    let clicked = c.click(&leaves_rows[1].id);
    c.modifier("shift", false)?;
    clicked?;
    c.until("shared range active endpoint", || {
        let state = c.state()?;
        let items = array(&state["selection"]["items"])?;
        Ok(items.len() >= 2 && items.last().is_some_and(|item| item["Node"]["node"] == two))
    })?;
    c.click(&leaves_rows[1].id)?;
    c.until("plain click replaces shared selection", || {
        Ok(array(&c.state()?["selection"]["items"])?.len() == 1)
    })?;
    search(c, &view, &prefix, "nothing matches")?;
    c.find("empty hierarchy filter", |n| {
        n.caption() == "No nodes match this search."
    })?;
    check(c.selection()? == two, "Filter changed hidden selection")?;
    c.menu(&view, "Reveal selected node")?;
    c.until("Reveal clears filter", || {
        Ok(c.node(&format!("{prefix}search"))?.value().is_empty())
    })?;
    c.until("Reveal scroll/focus", || {
        Ok(rows(c, &prefix, "Straße leaf")?.iter().any(|n| n.focused))
    })?;
    c.key("end")?;
    c.until("virtual tail", || {
        Ok(c.selection()?["node"] == reference["last_leaf"])
    })?;
    let end = c.selection()?;
    c.frame(c.window()?, "virtualized hierarchy tail")?;
    c.key("home")?;
    c.until("root Home", || {
        Ok(c.selection()?["node"] == main_root.to_string())
    })?;
    c.key("left")?;
    c.until("collapsed root", || {
        Ok(c.nodes()?
            .iter()
            .filter(|n| n.id.starts_with(&format!("{prefix}node/")))
            .count()
            == 1)
    })?;
    c.key("right")?;
    c.until("expanded root", || {
        Ok(c.nodes()?
            .iter()
            .filter(|n| n.id.starts_with(&format!("{prefix}node/")))
            .count()
            > 1)
    })?;
    search(c, &view, &prefix, "STRASSE")?;
    c.until("restored filter", || {
        Ok(rows(c, &prefix, "Straße leaf")?.len() == 2)
    })?;
    c.click(&rows(c, &prefix, "Straße leaf")?[0].id)?;
    let saved = c.selection()?;
    let field = c.node(&format!("{prefix}search"))?;
    let other_source = fs::read(directory.join("Other.prefab.ron"))?;
    fs::write(directory.join("Other.prefab.ron"), "changed")?;
    c.open_asset("Other.prefab.ron")?;
    c.until("failed Open preserves hierarchy", || {
        Ok(c.optional(&format!("{prefix}message"))?.is_some_and(|n| {
            n.caption()
                .contains("Showing the previously admitted document")
        }))
    })?;
    check(c.selection()? == saved, "Failed Open lost selection")?;
    fs::write(directory.join("Other.prefab.ron"), other_source)?;
    open(c, "Other.prefab.ron", other)?;
    c.until("other source root", || {
        Ok(!labels(c, &prefix, "Other root")?.is_empty())
    })?;
    check(
        c.selection()?["node"] == other_root.to_string(),
        "Document switch selected wrong root",
    )?;
    let s = c.state()?;
    let main_doc = array(&s["documents"]["documents"])?
        .iter()
        .find(|d| d["document"] == main.to_string())
        .ok_or("Missing main document")?;
    c.click(&Editor::label(text(&main_doc["view"])?))?;
    c.until("per-document search retained", || {
        Ok(c.node(&field.id)?.value() == "STRASSE")
    })?;
    check(
        c.selection()? == saved,
        "Document switch lost scoped selection",
    )?;
    c.menu(&view, "Float tab")?;
    c.until("floating hierarchy", || {
        Ok(c.node(&field.id)?.window()? != c.window()?)
    })?;
    let window = c.node(&field.id)?.window()?;
    c.resize(window, 320, 640, 2)?;
    c.frame(window, "narrow floating hierarchy")?;
    check(
        c.node(&field.id)?.entity == field.entity && c.selection()? == saved,
        "Float reconstructed hierarchy",
    )?;
    c.menu(&view, "Redock to main window")?;
    // Redock may merge into any eligible main group. Restore the authored preset
    // before closing the Hierarchy tab, keeping its source views visible.
    c.click("workspace/reset")?;
    c.until("isolated Hierarchy group", || {
        Ok(c.state()?["layout"]["nodes"][c.group(&view)?]["Group"]["tabs"] == json!([view]))
    })?;
    c.menu(&view, "Close tab")?;
    c.until("hidden hierarchy", || c.hidden(&view))?;
    check(c.selection()? == saved, "Hide lost source selection")?;
    c.main_menu("Windows/Hierarchy")?;
    check(
        c.wait_node(&field.id)?.entity == field.entity && c.node(&field.id)?.value() == "STRASSE",
        "Reopen lost search identity/value",
    )?;
    c.click("workspace/reset")?;
    check(
        c.wait_node(&field.id)?.entity == field.entity && c.selection()? == saved,
        "Reset rebuilt hierarchy",
    )?;
    check(
        inventory(&assets)? == before,
        "Read-only hierarchy changed sources or sidecars",
    )?;
    Ok(
        json!({"status":"passed","project":project,"frames":c.captures.borrow().clone(),"selection_examples":[one,two,end]}),
    )
}

fn usability(c: &Editor<'_>) -> Result<Value> {
    let view = c.view("hierarchy")?;
    let p = format!("hierarchy/ViewId({view})/");
    let close = || -> Result<()> {
        if c.present(&format!("{p}close-search"))? {
            c.click(&format!("{p}close-search"))?;
        }
        c.until("no default search toolbar", || {
            Ok(!c.present(&format!("{p}toolbar"))?)
        })
    };
    close()?;
    check(
        !c.present(&format!("{p}status"))?,
        "Default design has persistent UUID footer",
    )?;
    c.click(&Editor::label(&view))?;
    c.menu(&view, "Float tab")?;
    c.until("floating hierarchy", || {
        Ok(c.node(&Editor::label(&view))?.window()? != c.window()?)
    })?;
    let window = c.node(&Editor::label(&view))?.window()?;
    c.resize(window, 320, 674, 2)?;
    c.until("compact floating hierarchy", || {
        let n = c.node(&Editor::group_id(&c.group(&view)?))?;
        Ok(n.rect[2] > n.rect[0] && n.rect[2] - n.rect[0] < 360.)
    })?;
    c.menu(&view, "Reveal selected node")?;
    c.input(window, json!({"kind":"key","key":"home"}))?;
    c.wait_node(&format!("{p}row/0"))?;
    close()?;
    let root = c.node(&format!("{p}row/0"))?;
    let viewport = c.node(&format!("{p}entries"))?;
    check(
        root.rect[0] >= viewport.rect[0]
            && root.rect[1] >= viewport.rect[1]
            && root.rect[2] <= viewport.rect[2]
            && root.rect[3] <= viewport.rect[3],
        "Hierarchy root is not contained in its scroll viewport",
    )?;
    c.click(&format!("{p}node/0"))?;
    for key in ["left", "right"] {
        c.input(window, json!({"kind":"key","key":key}))?;
    }
    let row = c.wait_node(&format!("{p}row/1"))?;
    check(
        row.rect[0] > root.rect[0] && row.rect[1] >= root.rect[3],
        "Child row lost hierarchy indentation or overlaps its parent",
    )?;
    c.input(window, json!({"kind":"key","key":"down"}))?;
    c.until("keyboard-selected child row", || {
        Ok(c.node(&format!("{p}node/1"))?.focused)
    })?;
    let selected = c.state()?["documents"].clone();
    c.input(window, json!({"kind":"move","position":[310,620]}))?;
    c.frame(window, "hierarchy default")?;
    c.menu(&view, "Search nodes")?;
    c.wait_node(&format!("{p}search"))?;
    c.node(&format!("{p}toolbar"))?;
    for key in ["search-icon", "filter", "close-search"] {
        c.node(&format!("{p}{key}"))?;
    }
    check(
        !c.node(&format!("{p}filter"))?.enabled,
        "Unsupported filters appear functional",
    )?;
    c.field(&format!("{p}search"), "STRASSE")?;
    c.menu(&view, "Redock to main window")?;
    c.click("workspace/reset")?;
    let group = c.node(&Editor::group_id(&c.group(&view)?))?;
    let divider = c
        .nodes()?
        .into_iter()
        .filter(|n| n.id.ends_with("/divider") && n.rect[2] - n.rect[0] <= 8.)
        .min_by(|a, b| {
            (a.rect[0] - group.rect[2])
                .abs()
                .total_cmp(&(b.rect[0] - group.rect[2]).abs())
        })
        .ok_or("No hierarchy divider")?;
    let point = divider.center();
    c.move_to(point)?;
    c.held(true)?;
    c.move_to([
        group.rect[0] + 232. + (divider.rect[2] - divider.rect[0]) / 2.,
        point[1],
    ])?;
    c.held(false)?;
    c.until("narrowed docked hierarchy", || {
        let n = c.node(&Editor::group_id(&c.group(&view)?))?;
        Ok(n.rect[2] > n.rect[0] && n.rect[2] - n.rect[0] < group.rect[2] - group.rect[0])
    })?;
    c.click(&format!("{p}search"))?;
    c.frame(c.window()?, "hierarchy narrow search")?;
    let doc = c.current_document()?;
    c.click(&Editor::label(text(&doc["view"])?))?;
    c.frame(c.window()?, "inactive hierarchy selection")?;
    check(
        c.state()?["documents"] == selected,
        "Moving tool focus changed source identity",
    )?;
    c.key("escape")?;
    close()?;
    c.menu(&view, "Reveal selected node")?;
    c.key("home")?;
    c.key("left")?;
    c.until("collapsed hierarchy", || {
        Ok(!c.present(&format!("{p}row/1"))?)
    })?;
    c.frame(c.window()?, "collapsed hierarchy")?;
    c.menu(&view, "Toggle source details")?;
    c.wait_node(&format!("{p}status"))?;
    c.menu(&view, "Toggle source details")?;
    c.until("dismissed source details", || {
        Ok(!c.present(&format!("{p}status"))?)
    })?;
    c.key("right")?;
    c.click("workspace/reset")?;
    Ok(json!({"status":"passed","frames":c.captures.borrow().clone()}))
}
