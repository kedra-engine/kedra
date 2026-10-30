//! Editor panel retention/persistence and real workspace menus.
use super::support::*;
use kedra_test::Result;
use serde_json::{Value, json};

fn same(c: &Editor<'_>, before: &Value) -> Result<()> {
    check(
        c.state()?["layout"] == *before,
        "Uncommitted/cancelled docking gesture changed layout",
    )
}
fn cancel(c: &Editor<'_>, window: u64) -> Result<()> {
    c.input(window, json!({"kind":"key","key":"escape"}))?;
    c.release(window)?;
    for id in [
        "dock.drag/ghost",
        "dock.insertion",
        "dock.guide/tab/guide",
        "dock.preview",
    ] {
        check(!c.present(id)?, "Cancelled gesture retained feedback")?;
    }
    Ok(())
}
fn hidden(c: &Editor<'_>, view: &str) -> Result<bool> {
    c.hidden(view)
}

pub(super) fn editor(c: &Editor<'_>) -> Result<Value> {
    let notes = c.view("preview")?;
    let assets = c.view("assets")?;
    c.click(&Editor::label(&notes))?;
    let draft = c.node(&format!("panel/ViewId({notes})/empty"))?;
    let retained = || -> Result<()> {
        check(
            c.retained(&draft.id)?.entity == draft.entity,
            "Docking reconstructed retained panel",
        )
    };
    for dpr in [1, 2] {
        c.resize(c.window()?, 1200, 800, dpr)?;
        c.wait_node(&Editor::label(&notes))?;
        let group = c.group(&notes)?;
        let rail = c.node(&format!("dock/node/NodeId({group})/tabs"))?;
        let tab = c.node(&format!("dock/tab/ViewId({notes})"))?;
        check(
            rail.rect[1] <= tab.rect[1] && tab.rect[3] <= rail.rect[3],
            "Dock tab escaped its input rail",
        )?;
        c.frame(c.window()?, "usable docking rail")?;
    }
    retained()?;
    let assets_group = c.group(&assets)?;
    let bounds = c.node(&Editor::group_id(&assets_group))?;
    let target = [bounds.rect[0] + 12., bounds.center()[1]];
    let before_split = c.state()?["layout"].clone();
    let window = c.drag(&notes, target)?;
    let preview = c.preview()?;
    c.release(window)?;
    c.until("split commit", || {
        // Moving the only tab deliberately preserves its group identity (detach's
        // payload carries that group). Topology and measured bounds prove the split.
        Ok(c.group(&notes)? != assets_group && c.state()?["layout"] != before_split)
    })?;
    let split_state = c.state()?;
    let moved = c.group(&notes)?;
    check(
        object(&split_state["layout"]["nodes"])?
            .values()
            .any(|node| {
                let split = &node["Split"];
                split["axis"] == "Horizontal"
                    && split["first"] == moved
                    && split["second"] == assets_group
            }),
        "Split did not place the retained panel to the left of Assets",
    )?;
    let committed = c.wait_node(&Editor::group_id(&c.group(&notes)?))?;
    for (a, b) in preview.rect.into_iter().zip(committed.rect) {
        near(a, b, 1.)?;
    }
    retained()?;
    let before = c.state()?["layout"].clone();
    let group = c.node(&Editor::group_id(&c.group(&assets)?))?;
    let window = c.drag(&notes, group.center())?;
    c.preview()?;
    cancel(c, window)?;
    same(c, &before)?;
    retained()?;
    // Resize the split just created, whose two children have spare width. An
    // arbitrary outer divider may already be clamped by a panel's minimum size.
    let split = object(&before["nodes"])?
        .iter()
        .find(|(_, node)| {
            node["Split"]["first"] == moved && node["Split"]["second"] == assets_group
        })
        .map(|(id, _)| id)
        .ok_or("Missing created split")?;
    let divider = c.node(&format!("dock/node/NodeId({split})/divider"))?;
    let point = divider.center();
    c.move_to(point)?;
    c.held(true)?;
    c.until("divider resize acquired", || {
        Ok(c.state()?["navigation_input"]["gesture"] == true)
    })?;
    c.move_to([point[0] + 70., point[1]])?;
    same(c, &before)?;
    c.key("escape")?;
    c.held(false)?;
    same(c, &before)?;
    c.until("cancelled divider geometry restored", || {
        Ok((c.node(&divider.id)?.center()[0] - point[0]).abs() <= 1.)
    })?;
    c.move_to(point)?;
    c.held(true)?;
    c.until("second divider resize acquired", || {
        Ok(c.state()?["navigation_input"]["gesture"] == true)
    })?;
    c.move_to([point[0] + 50., point[1]])?;
    c.until("divider resize projected", || {
        Ok((c.node(&divider.id)?.center()[0] - point[0]).abs() > 1.)
    })?;
    c.held(false)?;
    c.until("committed resize", || Ok(c.state()?["layout"] != before))?;
    let before = c.state()?["layout"].clone();
    retained()?;
    let group = c.group(&notes)?;
    c.menu(&notes, "Maximize group")?;
    let host = c.node("workspace/host")?;
    c.until("maximized fills workspace", || {
        Ok(c.node(&Editor::group_id(&group))?
            .rect
            .into_iter()
            .zip(host.rect)
            .all(|(a, b)| (a - b).abs() <= 1.))
    })?;
    c.menu(&notes, "Restore group")?;
    same(c, &before)?;
    retained()?;
    c.menu(&notes, "Float tab")?;
    c.until("second native window", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 2)
    })?;
    retained()?;
    let floating = c.node(&draft.id)?.window()?;
    check(
        floating != c.window()?,
        "Float kept content in primary host",
    )?;
    c.frame(floating, "floating retained panel")?;
    c.menu(&notes, "Redock to main window")?;
    c.until("retired float", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 1)
    })?;
    retained()?;
    c.menu(&notes, "Float tab")?;
    c.until("second float", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 2)
    })?;
    c.close_window(c.node(&draft.id)?.window()?)?;
    c.until("floating close", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 1)
    })?;
    check(
        hidden(c, &notes)?,
        "Floating close did not hide source panel",
    )?;
    retained()?;
    c.main_menu("Windows/Document Preview")?;
    c.wait_node(&draft.id)?;
    retained()?;
    c.menu(&notes, "Float tab")?;
    c.until("persisted floating host", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 2)
    })?;
    retained()?;
    let layout = c.state()?["layout"].clone();
    let o = c.client.observe()?;
    for w in array(&o["windows"])? {
        c.frame(
            w["id"].as_u64().ok_or("Invalid window")?,
            "persisted docking hosts",
        )?;
    }
    Ok(
        json!({"status":"passed","layout":layout,"frames":c.captures.borrow().clone(),"physical_cross_monitor_input":"not verified"}),
    )
}

fn reset(c: &Editor<'_>, width: u32, height: u32, dpr: u32) -> Result<()> {
    c.activate(c.window()?)?;
    c.key("escape")?;
    c.click("workspace/reset")?;
    c.until("reset native hosts", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 1)
    })?;
    c.resize(c.window()?, width, height, dpr)
}

pub(super) fn states(c: &Editor<'_>) -> Result<Value> {
    // The shared fixture owns compass directions, insertion order, refusal and
    // cancellation. Here only real editor menu/host integration is exercised.
    reset(c, 1440, 1100, 2)?;
    let notes = c.view("preview")?;
    let hierarchy = c.view("hierarchy")?;
    for id in [
        format!("dock/node/NodeId({})/menu", c.group(&hierarchy)?),
        format!("dock/node/NodeId({})/menu", c.group(&notes)?),
        "workspace/panels".into(),
    ] {
        let button = c.node(&id)?;
        c.click(&id)?;
        c.anchored_popup(&button)?;
        c.key("escape")?;
    }
    c.frame(c.window()?, "editor docking menus")?;
    c.menu(&notes, "Float tab")?;
    c.until("small floating popup host", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 2)
    })?;
    let button = format!("dock/node/NodeId({})/menu", c.group(&notes)?);
    let floating = c.node(&button)?.window()?;
    c.resize(floating, 320, 320, 2)?;
    c.activate(floating)?;
    c.click(&button)?;
    c.anchored_popup(&c.node(&button)?)?;
    c.frame(floating, "small floating popup")?;
    c.input(floating, json!({"kind":"key","key":"escape"}))?;
    reset(c, 1440, 1100, 2)?;
    Ok(json!({"status":"passed","frames":c.captures.borrow().clone()}))
}
