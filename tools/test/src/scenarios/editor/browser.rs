//! Read-only Asset Browser virtualization, source admission and retained tools.
use super::support::*;
use kedra_test::Result;
use serde_json::{Value, json};
use std::fs;

fn control(c: &Editor<'_>, suffix: &str) -> Result<Node> {
    c.find(suffix, |n| {
        n.id.starts_with("browser/") && n.id.ends_with(&format!("/{suffix}"))
    })
}
fn has_text(c: &Editor<'_>, suffix: &str, value: &str) -> Result<bool> {
    Ok(c.nodes()?
        .iter()
        .any(|n| n.id.ends_with(&format!("/{suffix}")) && n.caption().contains(value)))
}
fn refresh(c: &Editor<'_>) -> Result<()> {
    c.menu(&c.view("assets")?, "Refresh sources")
}
fn open(c: &Editor<'_>) -> Result<()> {
    c.menu(&c.view("assets")?, "Open selected source")
}
fn search(c: &Editor<'_>, value: &str) -> Result<()> {
    c.field(&control(c, "search")?.id, value)
}
fn documents(c: &Editor<'_>) -> Result<Value> {
    Ok(c.state()?["documents"].clone())
}

fn focus_file_list(c: &Editor<'_>) -> Result<()> {
    // The shared project has more folders than the compact list viewport can
    // show. Mounted overscan rows are not necessarily pointer targets. Reveal
    // the first file with real scrolling before acquiring keyboard focus.
    let label = c.find("first list file", |n| {
        n.id.starts_with("browser/")
            && n.id.ends_with("/name")
            && n.caption() == "Composed.scene.ron"
    })?;
    let viewport = control(c, "entries")?;
    let delta = viewport.center()[1] - label.center()[1];
    c.move_to(viewport.center())?;
    c.event(json!({"kind":"wheel","delta":[0.,delta]}))?;
    c.until("first list file revealed", || {
        let file = c.node(&label.id)?;
        let viewport = control(c, "entries")?;
        Ok(file.rect[1] >= viewport.rect[1] && file.rect[3] <= viewport.rect[3])
    })?;
    c.browser_select("Composed.scene.ron")
}

pub(super) fn run(c: &Editor<'_>) -> Result<Value> {
    let project = c.open_project()?;
    c.resize(c.window()?, 1120, 640, 1)?;
    let assets = project.join("assets");
    let folder = assets.join("Prefabs");
    let source = folder.join("Example.prefab.ron");
    let source_bytes = fs::read(&source)?;
    let source_meta = fs::read(folder.join("Example.prefab.ron.meta"))?;
    fs::write(&source, "invalid source")?;
    let other = folder.join("Main.scene.ron");
    let other_bytes = fs::read(&other)?;
    let entries = fs::read_dir(&assets)?.collect::<std::io::Result<Vec<_>>>()?;
    let last = entries.iter().filter(|e| e.path().is_dir()).count();
    let folders: Vec<_> = entries
        .iter()
        .filter(|e| {
            e.path().is_dir()
                && e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("tree-"))
        })
        .map(|e| e.path())
        .collect();
    check(!folders.is_empty(), "Missing Browser overflow folders")?;
    let count = entries
        .iter()
        .filter(|e| !e.file_name().to_str().is_some_and(|n| n.ends_with(".meta")))
        .count()
        - folders.len();
    let last_file = entries
        .iter()
        .filter(|e| {
            e.path().is_file() && !e.file_name().to_str().is_some_and(|n| n.ends_with(".meta"))
        })
        .map(|e| e.file_name())
        .max()
        .ok_or("Missing Browser source files")?;
    let last_file = last_file
        .to_str()
        .ok_or("Non-Unicode Browser fixture name")?;
    refresh(c)?;
    c.until("root entries", || has_text(c, "name", "Prefabs"))?;
    let tree = control(c, "folder/0")?;
    let prefix = tree.id.strip_suffix('0').ok_or("Invalid root folder ID")?;
    c.frame(c.window()?, "bounded Browser viewport")?;
    c.until("complete folder inventory", || {
        let observation = c.client.observe()?;
        Ok(array(&observation["elements"])?
            .iter()
            .any(|n| n["id"] == format!("{prefix}{last}")))
    })?;
    let tail = c.retained(&format!("{prefix}{last}"))?;
    check(
        tail.rect[3] > control(c, "folders")?.rect[3] + 1.,
        "Folder fixture does not overflow its viewport",
    )?;
    c.click(&tree.id)?;
    for (key, index) in [
        ("end", last),
        ("home", 0),
        ("down", 1),
        ("end", last),
        ("up", last - 1),
        ("down", last),
        ("left", 0),
    ] {
        c.key(key)?;
        c.until("folder keyboard focus/reveal", || {
            let Some(n) = c.optional(&format!("{prefix}{index}"))? else {
                return Ok(false);
            };
            let viewport = control(c, "folders")?;
            Ok(n.focused
                && viewport.rect[1] - 1. <= n.rect[1]
                && n.rect[3] <= viewport.rect[3] + 1.)
        })?;
    }
    c.frame(c.window()?, "folder keyboard virtualization")?;
    let retired = project.join("retired-browser-folders");
    fs::create_dir(&retired)?;
    for path in folders {
        fs::rename(
            &path,
            retired.join(path.file_name().ok_or("Missing tree folder name")?),
        )?;
    }
    refresh(c)?;
    c.until("tree fixture retired", || {
        has_text(c, "count", &format!("{count}/{count} items"))
    })?;
    let disabled = control(c, "new")?;
    check(!disabled.enabled, "Unsupported New command is enabled")?;
    c.move_to(disabled.center())?;
    c.until("disabled command reason", || {
        has_text(c, "tooltip", "New is unavailable")
    })?;
    check(
        !has_text(c, "name", "hidden.meta")?,
        "Sidecar exposed as independent source",
    )?;
    let browser_prefix = tree
        .id
        .strip_suffix("folder/0")
        .ok_or("Invalid Browser ID")?;
    let last_item = format!("{browser_prefix}item/{}", count - 1);
    c.until("virtual grid omits offscreen sources", || {
        let observation = c.client.observe()?;
        let nodes = array(&observation["elements"])?;
        let mounted = nodes
            .iter()
            .filter(|n| {
                n["id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with(browser_prefix) && id.contains("/item/"))
            })
            .count();
        Ok(mounted > 0 && mounted < count && !nodes.iter().any(|n| n["id"] == last_item))
    })?;
    search(c, "STRASSE")?;
    c.until("Unicode case folding", || has_text(c, "name", "Straße.txt"))?;
    c.type_text("bad\nquery")?;
    check(
        control(c, "search")?.value() == "STRASSE",
        "Search admitted control characters",
    )?;
    c.browser_select("Straße.txt")?;
    c.until("relative source status", || {
        has_text(
            c,
            "status",
            &std::path::Path::new("assets")
                .join("Straße.txt")
                .display()
                .to_string(),
        )
    })?;
    let absolute = assets.display().to_string();
    check(
        !c.nodes()?.iter().any(|n| n.caption().contains(&absolute)),
        "Absolute source root leaked into UI",
    )?;
    c.find("filename tooltip", |n| {
        n.id.ends_with("/tooltip") && n.caption() == "Straße.txt"
    })?;
    let label = c.find("selected file label", |n| {
        n.id.ends_with("/name") && n.caption() == "Straße.txt"
    })?;
    let tile = c
        .nodes()?
        .into_iter()
        .find(|n| Some(n.entity) == label.parent)
        .ok_or("Missing file tile")?;
    let pointer = tile.center();
    c.input(tile.window()?, json!({"kind":"move","position":pointer}))?;
    c.until("compact cursor tooltip", || {
        Ok(c.nodes()?.iter().any(|n| {
            n.id.ends_with("/tooltip") && {
                let [l, t, r, b] = n.rect;
                20. < r - l
                    && r - l <= 160.
                    && 16. <= b - t
                    && b - t <= 40.
                    && (l - pointer[0]).abs() <= 170.
                    && (t - pointer[1]).abs() <= 52.
            }
        }))
    })?;
    c.move_to([8., 40.])?;
    c.until("tooltip departure", || {
        Ok(!c.nodes()?.iter().any(|n| n.id.ends_with("/tooltip")))
    })?;
    check(
        documents(c)?["active"].is_null(),
        "File selection invented source authority",
    )?;
    search(c, "no matches")?;
    c.until("empty filter", || {
        has_text(c, "message", "No sources match")
    })?;
    c.until("retained hidden selection", || {
        has_text(c, "status", "filtered selection")
    })?;
    c.click(&control(c, "clear")?.id)?;
    c.click(&control(c, "list")?.id)?;
    // Acquire file-list focus through a visible file, not a folder (which
    // navigates), before testing End against the shared project's full listing.
    focus_file_list(c)?;
    c.until("list tail starts unmounted", || {
        let observation = c.client.observe()?;
        Ok(!array(&observation["elements"])?
            .iter()
            .any(|n| n["id"] == last_item))
    })?;
    c.key("end")?;
    c.until("virtual list End reveals the last source", || {
        let Some(tail) = c.optional(&last_item)? else {
            return Ok(false);
        };
        let viewport = control(c, "entries")?;
        Ok(tail.focused
            && tail.rect[1] >= viewport.rect[1] - 1.
            && tail.rect[3] <= viewport.rect[3] + 1.
            && has_text(c, "status", last_file)?)
    })?;
    c.frame(c.window()?, "virtual list tail")?;
    let thumb = control(c, "content-thumb")?;
    let viewport = control(c, "entries")?;
    // Bevy starts the scrollbar drag on the first displacement; the native
    // drag sequence supplies subsequent moves to update the captured thumb.
    // Move beyond the track start so the minimum-size thumb clamps to Home.
    c.event(json!({"kind":"drag","from":thumb.center(),
        "to":[thumb.center()[0],viewport.rect[1]-(thumb.rect[3]-thumb.rect[1])/2.]}))?;
    c.until("scroll thumb reveals list top", || {
        Ok(c.nodes()?.iter().any(|n| {
            n.id.starts_with(browser_prefix)
                && n.id.contains("/row/")
                && n.id.ends_with("/name")
                && n.caption() == "Empty"
                && n.rect[1] >= viewport.rect[1]
                && n.rect[3] <= viewport.rect[3]
        }))
    })?;
    focus_file_list(c)?;
    c.key("home")?;
    c.until("virtual list Home focuses the first source", || {
        let Some(first) = c.optional(&format!("{browser_prefix}item/0"))? else {
            return Ok(false);
        };
        let viewport = control(c, "entries")?;
        Ok(first.focused
            && first.rect[1] >= viewport.rect[1] - 1.
            && first.rect[3] <= viewport.rect[3] + 1.)
    })?;
    c.click(&control(c, "grid")?.id)?;
    search(c, "Prefabs")?;
    c.browser_select("Prefabs")?;
    open(c)?;
    c.until("folder navigation", || {
        has_text(c, "name", "Example.prefab.ron")
    })?;
    c.click(&control(c, "back")?.id)?;
    c.until("folder history filter", || {
        Ok(control(c, "search")?.value() == "Prefabs")
    })?;
    c.click(&control(c, "forward")?.id)?;
    c.browser_select("Example.prefab.ron")?;
    open(c)?;
    c.until("malformed source refusal", || {
        has_text(c, "message", "Cannot open source")
    })?;
    check(
        documents(c)?["active"].is_null(),
        "Invalid source acquired document authority",
    )?;
    fs::write(&source, source_bytes)?;
    c.browser_select("Example.prefab.ron")?;
    c.browser_select("Example.prefab.ron")?;
    c.until("native double click admission", || {
        Ok(array(&documents(c)?["documents"])?.len() == 1)
    })?;
    check(
        !c.nodes()?.iter().any(|n| n.caption().contains(&absolute)),
        "Absolute path leaked after admission",
    )?;
    check(
        !c.nodes()?
            .iter()
            .any(|n| n.id.starts_with("panel/") && n.value.is_some()),
        "Read-only document mounted editable field",
    )?;
    let active = documents(c)?["active"].clone();
    open(c)?;
    check(
        array(&documents(c)?["documents"])?.len() == 1,
        "Duplicate Open created document",
    )?;
    fs::write(&other, "changed after admission")?;
    c.browser_select("Main.scene.ron")?;
    c.key("enter")?;
    c.until("changed-source refusal", || {
        has_text(c, "message", "changed since admission")
    })?;
    check(
        documents(c)?["active"] == active,
        "Failed Open replaced active document",
    )?;
    fs::write(&other, other_bytes)?;
    open(c)?;
    c.until("shared source scope", || {
        Ok(array(&documents(c)?["documents"])?.len() == 2)
    })?;
    let doc = c.current_document()?;
    let view = text(&doc["view"])?;
    let group = c.group(view)?;
    c.click(&Editor::label(view))?;
    c.move_to(c.node(&Editor::label(view))?.center())?;
    c.click(&format!("dock/view/ViewId({view})/close"))?;
    c.until("closed source view", || c.hidden(view))?;
    check(
        array(&documents(c)?["documents"])?.len() == 2,
        "View close retired source document",
    )?;
    open(c)?;
    c.until("reopened source view", || Ok(!c.hidden(view)?))?;
    check(c.group(view)? == group, "Reopen lost source group")?;
    check(
        control(c, "status-clip")?.rect[2] < control(c, "count")?.rect[0],
        "Source path overlaps count",
    )?;
    let icon = control(c, "selected-folder")?;
    let label = c.find("selected folder label", |n| {
        n.parent == icon.parent && n.caption() == "Prefabs"
    })?;
    check(
        icon.rect[2] < label.rect[0],
        "Folder icon moved after label",
    )?;
    c.frame(c.window()?, "source admission")?;
    let admitted = documents(c)?;
    let browser = c.view("assets")?;
    let search_node = control(c, "search")?;
    c.menu(&browser, "Float group")?;
    c.until("floating browser", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 2)
    })?;
    check(
        c.node(&search_node.id)?.entity == search_node.entity,
        "Float reconstructed search field",
    )?;
    let window = c.node(&search_node.id)?.window()?;
    c.resize(window, 480, 560, 2)?;
    c.frame(window, "narrow floating browser")?;
    c.menu(&browser, "Redock to main window")?;
    c.until("redocked browser", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 1)
    })?;
    c.click("workspace/reset")?;
    c.until("isolated Assets group", || {
        Ok(c.state()?["layout"]["nodes"][c.group(&browser)?]["Group"]["tabs"] == json!([browser]))
    })?;
    c.menu(&browser, "Hide group")?;
    c.until("hidden browser", || c.hidden(&browser))?;
    c.click("workspace/panels")?;
    let show = c.find("Show Assets", |n| n.caption() == "Show Assets")?;
    c.click(show.id.strip_suffix("/caption").unwrap_or(&show.id))?;
    check(
        c.wait_node(&search_node.id)?.entity == search_node.entity,
        "Reopen reconstructed search field",
    )?;
    check(
        has_text(
            c,
            "status",
            &std::path::Path::new("assets")
                .join("Prefabs/Main.scene.ron")
                .display()
                .to_string(),
        )?,
        "Selected source lost after reopen",
    )?;
    check(
        documents(c)?["documents"] == admitted["documents"],
        "Hide/reopen changed source documents",
    )?;
    open(c)?;
    c.until("selected admitted source visible", || {
        Ok(documents(c)?["active"] == doc["document"] && !c.hidden(view)?)
    })?;
    check(
        documents(c)?["documents"] == admitted["documents"],
        "Reopen duplicated documents or selections",
    )?;
    check(
        c.state()?["layout"]["nodes"][c.group(view)?]["Group"]["active"] == view,
        "Reopened source tab inactive",
    )?;
    let opened = documents(c)?;
    c.click("workspace/reset")?;
    check(
        c.wait_node(&search_node.id)?.entity == search_node.entity,
        "Reset reconstructed search",
    )?;
    check(
        documents(c)? == opened,
        "Reset lost source scope or active selection",
    )?;
    check(
        c.state()?["layout"]["nodes"][c.group(view)?]["Group"]["active"] == view
            && c.group(view)? != c.group(&browser)?,
        "Reset merged source into browser group",
    )?;
    check(
        fs::read_to_string(assets.join("Straße.txt"))? == "not executed"
            && fs::read(folder.join("Example.prefab.ron.meta"))? == source_meta,
        "Read-only browser changed sources or sidecars",
    )?;
    Ok(json!({"status":"passed","project":project,"frames":c.captures.borrow().clone()}))
}
