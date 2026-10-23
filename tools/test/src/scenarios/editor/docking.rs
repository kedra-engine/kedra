//! Production docking gestures, retention, compass states and cancellation.
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
        near(rail.rect[3] - rail.rect[1], 32., 1.)?;
        near(tab.rect[3] - tab.rect[1], 28., 1.)?;
        c.frame(c.window()?, "docking Figma rail metrics")?;
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
    let divider = c
        .nodes()?
        .into_iter()
        .find(|n| n.id.ends_with("/divider") && n.rect[2] - n.rect[0] < 8.)
        .ok_or("Missing vertical splitter")?;
    let point = divider.center();
    c.move_to(point)?;
    c.held(true)?;
    c.move_to([point[0] + 70., point[1]])?;
    same(c, &before)?;
    c.key("escape")?;
    c.held(false)?;
    same(c, &before)?;
    c.move_to(point)?;
    c.held(true)?;
    c.move_to([point[0] + 50., point[1]])?;
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
    c.menu(&notes, "Float group")?;
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
    c.menu(&notes, "Float group")?;
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
    c.click("workspace/panels")?;
    let show = c.find("Show Document Preview", |n| {
        n.caption() == "Show Document Preview"
    })?;
    c.click(show.id.strip_suffix("/caption").unwrap_or(&show.id))?;
    c.wait_node(&draft.id)?;
    retained()?;
    c.menu(&notes, "Float group")?;
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
    c.key("escape")?;
    c.click("workspace/reset")?;
    c.until("reset native hosts", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 1)
    })?;
    c.resize(c.window()?, width, height, dpr)
}

pub(super) fn states(c: &Editor<'_>) -> Result<Value> {
    for dpr in [1, 2] {
        reset(c, 1440, 1100, dpr)?;
        let project = c.view("hierarchy")?;
        let notes = c.view("preview")?;
        let activity = c.view("inspector")?;
        for id in [
            format!("dock/node/NodeId({})/menu", c.group(&project)?),
            format!("dock/node/NodeId({})/menu", c.group(&notes)?),
            "workspace/panels".into(),
        ] {
            let button = c.node(&id)?;
            c.click(&id)?;
            let menu = c.anchored_popup(&button)?;
            check(
                menu.rect[0] >= 3.
                    && menu.rect[1] >= 3.
                    && menu.rect[2] <= 1437.
                    && menu.rect[3] <= 1097.,
                "Dock popup escaped host",
            )?;
            c.key("escape")?;
        }
        for right in [true, false] {
            let tab = c.node(&format!("dock/tab/ViewId({activity})"))?;
            let x = if right {
                tab.rect[2] - 1.
            } else {
                tab.rect[0] + 1.
            };
            let before = c.state()?["layout"].clone();
            let window = c.drag(&notes, [x, tab.center()[1]])?;
            let marker = c.wait_node("dock.insertion")?;
            near(marker.rect[2] - marker.rect[0], 2., 0.)?;
            near(marker.rect[3] - marker.rect[1], 28., 0.)?;
            same(c, &before)?;
            check(
                !c.present("dock.preview")? && !c.present("dock.guide/tab/guide")?,
                "Rail insertion painted split/compass feedback",
            )?;
            if right {
                c.frame(window, "tab insertion")?;
            }
            c.release(window)?;
            let expected = if right {
                json!([activity, notes])
            } else {
                json!([notes, activity])
            };
            check(
                c.state()?["layout"]["nodes"][c.group(&notes)?]["Group"]["tabs"] == expected,
                "Tab reorder lost requested order",
            )?;
        }
    }
    for direction in ["left", "right", "top", "bottom", "tab"] {
        reset(c, 1440, 1100, 2)?;
        let notes = c.view("preview")?;
        let assets = c.view("assets")?;
        let field = c.node(&format!("panel/ViewId({notes})/empty"))?;
        let target_group = c.group(&assets)?;
        let target = c.node(&Editor::group_id(&target_group))?;
        let before = c.state()?["layout"].clone();
        let window = c.drag(&notes, target.center())?;
        for guide in ["tab", "left", "right", "top", "bottom"] {
            let n = c.wait_node(&format!("dock.guide/{guide}/guide"))?;
            near(n.rect[2] - n.rect[0], 28., 0.)?;
            near(n.rect[3] - n.rect[1], 28., 0.)?;
        }
        let guide = c.node(&format!("dock.guide/{direction}/guide"))?;
        c.input(window, json!({"kind":"move","position":guide.center()}))?;
        let preview = c.preview()?;
        same(c, &before)?;
        c.frame(window, direction)?;
        c.release(window)?;
        c.until("compass commit", || Ok(c.state()?["layout"] != before))?;
        check(
            (c.group(&notes)? == target_group) == (direction == "tab"),
            "Compass committed wrong destination",
        )?;
        let committed = c.node(&Editor::group_id(&c.group(&notes)?))?;
        for (a, b) in preview.rect.into_iter().zip(committed.rect) {
            near(a, b, 1.)?;
        }
        check(
            c.node(&field.id)?.entity == field.entity,
            "Compass reconstructed retained panel",
        )?;
    }
    reset(c, 960, 800, 2)?;
    let notes = c.view("preview")?;
    let hierarchy = c.view("hierarchy")?;
    let assets = c.view("assets")?;
    let before = c.state()?["layout"].clone();
    let target = c.node(&Editor::group_id(&c.group(&hierarchy)?))?;
    let window = c.drag(&notes, target.center())?;
    let guide = c.wait_node("dock.guide/left/guide")?;
    c.input(window, json!({"kind":"move","position":guide.center()}))?;
    c.wait_node("dock.drag/reason")?;
    check(
        !c.present("dock.preview")?,
        "Unavailable split painted destination",
    )?;
    c.frame(window, "unavailable drop")?;
    c.release(window)?;
    same(c, &before)?;
    let target = c.node(&Editor::group_id(&c.group(&assets)?))?;
    let window = c.drag(&notes, target.center())?;
    c.preview()?;
    cancel(c, window)?;
    same(c, &before)?;
    reset(c, 1440, 1100, 2)?;
    c.menu(&notes, "Float group")?;
    c.until("small floating popup host", || {
        Ok(array(&c.client.observe()?["windows"])?.len() == 2)
    })?;
    let button = format!("dock/node/NodeId({})/menu", c.group(&notes)?);
    let floating = c.node(&button)?.window()?;
    c.resize(floating, 320, 320, 2)?;
    c.click(&button)?;
    let menu = c.wait_node("dock-menu")?;
    check(
        menu.window()? == floating
            && menu.rect[0] >= 3.
            && menu.rect[1] >= 3.
            && menu.rect[2] <= 317.
            && menu.rect[3] <= 317.,
        "Small native popup escaped host",
    )?;
    c.frame(floating, "small floating popup")?;
    c.input(floating, json!({"kind":"key","key":"escape"}))?;
    reset(c, 1440, 1100, 2)?;
    Ok(json!({"status":"passed","frames":c.captures.borrow().clone()}))
}
