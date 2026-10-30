//! Native retained workspace, floating headers and docking input contracts.
use super::support::*;
use kedra_test::Result;
use serde_json::{Value, json};

fn placement(c: &Editor<'_>) -> Result<Value> {
    let mut groups = serde_json::Map::new();
    for kind in ["hierarchy", "preview", "inspector", "assets"] {
        let view = c.view(kind)?;
        c.node(&Editor::label(&view))?;
        groups.insert(
            kind.into(),
            json!(c.node(&Editor::group_id(&c.group(&view)?))?.rect),
        );
    }
    let rect = |key: &str| -> Result<[f64; 4]> { Ok(serde_json::from_value(groups[key].clone())?) };
    let (h, p, i, a) = (
        rect("hierarchy")?,
        rect("preview")?,
        rect("inspector")?,
        rect("assets")?,
    );
    check(
        h[2] <= p[0] && p[2] <= i[0] && p[3] <= a[1],
        "Authoring panel placement differs",
    )?;
    for (x, y) in [(p[0], a[0]), (p[2], a[2]), (h[1], i[1]), (h[3], i[3])] {
        near(x, y, 1.)?;
    }
    Ok(Value::Object(groups))
}

pub(super) fn run(c: &Editor<'_>, name: &str) -> Result<Value> {
    c.until("bound workspace", || Ok(c.state()?["bound"] == true))?;
    match name {
        "workspace" => workspace(c),
        "window-chrome" => chrome(c),
        "docking" => super::docking::editor(c),
        "docking-states" => super::docking::states(c),
        _ => Err("Unsupported workspace scenario".into()),
    }
}

fn workspace(c: &Editor<'_>) -> Result<Value> {
    c.resize(c.window()?, 1440, 900, 2)?;
    check(
        object(&c.state()?["layout"]["views"])?.len() == 4,
        "Unexpected authoring view inventory",
    )?;
    let mut retained = Vec::new();
    for kind in ["preview", "inspector", "hierarchy"] {
        let view = c.view(kind)?;
        let id = if kind == "hierarchy" {
            format!("hierarchy/ViewId({view})/entries")
        } else if kind == "inspector" {
            format!("inspector/ViewId({view})/message")
        } else {
            format!("panel/ViewId({view})/empty")
        };
        let root = c.wait_node(&id)?;
        if kind == "preview" {
            check(
                !c.nodes()?.iter().any(|n| n.parent == Some(root.entity)),
                "Placeholder is not empty",
            )?;
        }
        retained.push(root);
    }
    let geometry = placement(c)?;
    c.frame(c.window()?, "startup placement")?;
    let inspector = c.view("inspector")?;
    let root = &retained[1];
    c.menu(&inspector, "Float tab")?;
    c.until("second native window", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 2)
    })?;
    check(
        c.node(&root.id)?.entity == root.entity,
        "Float rebuilt placeholder",
    )?;
    c.frame(c.node(&root.id)?.window()?, "floating Inspector")?;
    let slot = c.node("floating-caption/window.menu")?;
    let tabs = c.node("floating-caption/window.tabs")?;
    let menu = c.node(&format!("dock/node/NodeId({})/menu", c.group(&inspector)?))?;
    check(
        menu.rect[0] >= slot.rect[0]
            && menu.rect[2] <= slot.rect[2]
            && menu.rect[2] <= tabs.rect[0],
        "Inspector header actions overflow their slot and overlap tab input",
    )?;
    c.menu(&inspector, "Redock to main window")?;
    c.until("retired floating window", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 1)
    })?;
    c.menu(&inspector, "Close tab")?;
    c.until("hidden Inspector", || c.hidden(&inspector))?;
    c.main_menu("Windows/Inspector")?;
    check(
        c.wait_node(&root.id)?.entity == root.entity,
        "Reopen rebuilt placeholder",
    )?;
    c.click("workspace/reset")?;
    placement(c)?;
    for n in retained {
        check(
            c.node(&n.id)?.entity == n.entity,
            "Reset rebuilt retained tool",
        )?;
    }
    c.frame(c.window()?, "reset placement")?;
    Ok(json!({"status":"passed","geometry":geometry,"frames":c.captures.borrow().clone()}))
}

fn header(n: &Node, window: u64) -> Result<()> {
    check(n.window()? == window, "Header belongs to another window")?;
    check(
        n.rect.iter().all(|v| v.is_finite()) && n.rect[2] > n.rect[0] && n.rect[3] > n.rect[1],
        "Invalid native header",
    )
}
fn drop_at(c: &Editor<'_>, view: &str, p: [f64; 2]) -> Result<()> {
    let w = c.drag(view, p)?;
    c.preview()?;
    c.release(w)
}

fn chrome(c: &Editor<'_>) -> Result<Value> {
    c.click("workspace/reset")?;
    c.until("four-tool preset", || {
        let s = c.state()?;
        Ok(object(&s["layout"]["hosts"])?.len() == 1
            && object(&s["layout"]["nodes"])?
                .values()
                .filter(|n| !n["Group"].is_null())
                .count()
                >= 4
            && s["navigation_input"]["rendered"] == s["revision"])
    })?;
    let main = c.window()?;
    for dpr in [1, 2] {
        c.resize(main, 1280, 800, dpr)?;
        let row = c.wait_node("window.chrome")?;
        header(&row, main)?;
        near(c.node("editor.canvas")?.rect[1], row.rect[3], 1.)?;
        check(
            c.node("workspace/status")?.rect[1] >= c.node("workspace/host")?.rect[3] - 1.,
            "Status overlaps workspace",
        )?;
        for id in [
            "editor.play",
            "editor.pause",
            "editor.stop",
            "editor.save",
            "editor.build",
            "editor.search",
        ] {
            check(
                !c.node(id)?.enabled,
                "Unsupported header action appears functional",
            )?;
        }
        let before = c.state()?["layout"].clone();
        c.click("workspace/panels")?;
        c.find("header menu", |n| n.id.starts_with("dock-menu/"))?;
        c.key("escape")?;
        check(
            c.state()?["layout"] == before,
            "Header menu started docking gesture",
        )?;
        c.frame(main, "main header")?;
    }
    let a = c.view("hierarchy")?;
    let b = c.view("assets")?;
    drop_at(c, &a, c.node(&Editor::group_id(&c.group(&b)?))?.center())?;
    c.until("merged tools", || Ok(c.group(&a)? == c.group(&b)?))?;
    let labels = [c.node(&Editor::label(&a))?, c.node(&Editor::label(&b))?];
    c.menu(&a, "Float group")?;
    c.until("floating group", || {
        Ok(c.node(&labels[0].id)?.window()? != main)
    })?;
    let floating = c.node(&labels[0].id)?.window()?;
    let row = c.wait_node("floating-caption/window.chrome")?;
    header(&row, floating)?;
    for label in &labels {
        let n = c.node(&label.id)?;
        check(
            n.entity == label.entity && n.rect[1] >= row.rect[1] && n.rect[3] <= row.rect[3] + 1.,
            "Floating tab lost identity/integration",
        )?;
    }
    c.frame(floating, "integrated floating header")?;
    let group = c.node(&Editor::group_id(&c.group(&a)?))?;
    let point = [group.rect[0] + 12., group.center()[1]];
    let before = c.state()?["layout"].clone();
    let w = c.drag(&a, point)?;
    c.preview()?;
    c.input(w, json!({"kind":"key","key":"escape"}))?;
    c.release(w)?;
    check(
        c.state()?["layout"] == before,
        "Cancelled split changed header policy",
    )?;
    drop_at(c, &a, point)?;
    c.until("floating split", || Ok(c.group(&a)? != c.group(&b)?))?;
    for view in [&a, &b] {
        let n = c.node(&Editor::label(view))?;
        check(
            labels.iter().any(|l| l.entity == n.entity) && n.rect[1] >= row.rect[3] - 1.,
            "Split header retained integrated slot",
        )?;
        let group = c.group(view)?;
        let rail = c.node(&format!("dock/node/NodeId({group})/tabs"))?;
        let s = c.state()?;
        let first = text(&s["layout"]["nodes"][&group]["Group"]["tabs"][0])?;
        near(
            c.node(&format!("dock/tab/ViewId({first})"))?.rect[0],
            rail.rect[0],
            1.,
        )?;
    }
    c.frame(floating, "split floating header")?;
    drop_at(c, &a, c.node(&Editor::group_id(&c.group(&b)?))?.center())?;
    c.until("floating remerge", || Ok(c.group(&a)? == c.group(&b)?))?;
    for label in labels {
        let n = c.node(&label.id)?;
        check(
            n.entity == label.entity && n.rect[3] <= row.rect[3] + 1.,
            "Remerge lost integrated header",
        )?;
    }
    c.frame(floating, "reintegrated header")?;
    c.menu(&a, "Redock to main window")?;
    c.until("retired floating header", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 1)
    })?;
    Ok(json!({"status":"passed","frames":c.captures.borrow().clone()}))
}
