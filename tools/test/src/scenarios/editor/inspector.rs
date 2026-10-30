//! Read-only Inspector specialization, effective source values and retained identity.
use super::support::*;
use crate::project::inventory;
use kedra_test::Result;
use serde_json::{Value, json};
mod caption;
mod collections;
mod compound_collections;
mod layout;
mod resources;
mod saving;
mod spatial;
mod transform;

fn choose(c: &Editor<'_>, control: &Node, caption: &str) -> Result<()> {
    c.click(&control.id)?;
    let item = c.find("Native variant menu item", |node| {
        node.id.starts_with(&format!("{}/menu/", control.id)) && node.caption() == caption
    })?;
    c.click(&item.id)
}

fn inputs(c: &Editor<'_>, prefix: &str) -> Result<Vec<Node>> {
    Ok(c.nodes()?
        .into_iter()
        .filter(|n| n.id.starts_with(prefix) && n.value.is_some())
        .collect())
}
fn readonly(c: &Editor<'_>, node: &Node) -> Result<()> {
    c.click(&node.id)?;
    c.input(node.window()?, json!({"kind":"key","key":"select_all"}))?;
    let receipt = c.client.receipt(c.client.submit(
        "app.input",
        json!({"window":node.window()?,"input":{"kind":"text","text":"Must not mutate"}}),
    )?)?;
    if receipt["state"] == "failed" {
        check(
            receipt["detail"]
                .to_string()
                .to_lowercase()
                .contains("read-only"),
            "Unexpected native read-only failure",
        )?;
    }
    let current = c.node(&node.id)?;
    check(
        current.entity == node.entity && current.value == node.value,
        "Read-only native input mutated/rebuilt field",
    )
}

pub(super) fn fixture(c: &Editor<'_>) -> Result<Value> {
    text_acceptance(c)?;
    let p = "fixture/settings/";
    c.until("field specialization", || {
        Ok(inputs(c, p)?.iter().any(|n| n.value() == "25%"))
    })?;
    let values = inputs(c, p)?;
    c.frame(c.window()?, "native Transform fields")?;
    let math_axes = inline_transform(c, p)?;
    let transform_toggle = c.find("base-owned Transform disclosure", |node| {
        node.id.starts_with(p) && node.id.ends_with("/chrome/toggle")
    })?;
    for toggle in [&transform_toggle.id, &format!("{p}toggle")] {
        section_order(c, toggle, true)?;
        c.click(toggle)?;
        c.until("collapsed Inspector section", || {
            Ok(!c.present(&math_axes[0].id)?)
        })?;
        c.frame(c.window()?, "collapsed Inspector section")?;
        section_order(c, toggle, false)?;
        for axis in &math_axes {
            let retained = c.retained(&axis.id)?;
            check(
                retained.entity == axis.entity && retained.value == axis.value,
                "Section collapse retired or changed a native field",
            )?;
        }
        c.click(toggle)?;
        c.until("expanded Inspector section", || c.present(&math_axes[0].id))?;
        for axis in &math_axes {
            check(
                c.wait_node(&axis.id)?.entity == axis.entity,
                "Section expansion rebuilt a native field",
            )?;
        }
    }
    check(
        values.iter().any(|n| n.value() == "3 / 9")
            && values.iter().any(|n| n.value() == "18446744073709551615")
            && values.iter().any(|n| n.value() == "0.33333334")
            && values.iter().any(|n| n.value() == "Calibration 名称"),
        "Missing exact specialization/integer precision",
    )?;
    readonly(
        c,
        values
            .iter()
            .find(|n| n.value() == "25%")
            .ok_or("Missing specialized field")?,
    )?;
    c.frame(c.window()?, "specialized fields")?;
    let boolean = c
        .nodes()?
        .into_iter()
        .find(|n| n.id.starts_with(p) && n.checked == Some(true))
        .ok_or("Missing boolean field")?;
    let gain = values
        .iter()
        .find(|n| n.value() == "25%")
        .ok_or("Missing gain field")?;
    check(
        c.client
            .scenario(
                "inspector.presentation",
                json!({"boolean":"outside-scope", "gain":gain.id}),
            )
            .is_err(),
        "Luau accepted an unrelated field scope",
    )?;
    check(
        c.node(&boolean.id)?.checked == Some(true),
        "Rejected Luau program changed host values",
    )?;
    c.client.scenario(
        "inspector.presentation",
        json!({"boolean":boolean.id,"gain":gain.id}),
    )?;
    c.until("host field refresh", || {
        Ok(c.node(&boolean.id)?.checked == Some(false))
    })?;
    check(
        c.node(&boolean.id)?.entity == boolean.entity,
        "Refresh reconstructed field",
    )?;
    c.until("default view", || {
        Ok(inputs(c, p)?.iter().any(|n| n.value() == "0.25"))
    })?;
    check(
        !inputs(c, p)?.iter().any(|n| n.value() == "25%"),
        "Explicit default view retained specialization",
    )?;
    let defaults = inputs(c, p)?;
    check(
        defaults.iter().any(|n| n.value() == "3")
            && defaults.iter().any(|n| n.value() == "9")
            && defaults.iter().any(|n| n.value() == "18446744073709551615")
            && defaults.iter().any(|n| n.value() == "0.33333334"),
        "Default fallback hid native nested values or lost numeric precision",
    )?;
    c.frame(c.window()?, "default fields")?;
    c.resize(c.window()?, 320, 740, 1)?;
    c.wait_node("fixture/toggle")?;
    c.frame(c.window()?, "narrow field layout")?;
    check(
        c.node(&boolean.id)?.entity == boolean.entity,
        "Narrow resize rebuilt retained native fields",
    )?;
    // Return to specialized controls, then inspect actual renderer-derived geometry
    // at both widths. Default mode intentionally retires specialized subtrees.
    c.click("fixture/toggle")?;
    c.until("native Transform controls", || {
        Ok(inputs(c, p)?
            .iter()
            .any(|n| n.id.ends_with("/translation/x/value")))
    })?;
    c.frame(c.window()?, "narrow Transform fields")?;
    let axes = inline_transform(c, p)?;
    c.resize(c.window()?, 480, 740, 1)?;
    c.frame(c.window()?, "wide Transform fields")?;
    inline_transform(c, p)?;
    for axis in axes {
        check(
            c.node(&axis.id)?.entity == axis.entity,
            "Resize rebuilt a retained native vector input",
        )?;
    }
    Ok(json!({"status":"passed","frames":c.captures.borrow().clone()}))
}

fn text_acceptance(c: &Editor<'_>) -> Result<()> {
    let id = "fixture/text/value";
    let original = c.wait_node(id)?;
    check(
        original.value() == "Draft value",
        "Missing independent text value",
    )?;
    c.field(id, "  名称🙂  ")?;
    let window = c.node(id)?.window()?;
    c.input(window, json!({"kind":"key","key":"enter"}))?;
    c.until("native text acceptance", || {
        Ok(c.node("fixture/text/diagnostic")?.caption() == "Accepted:   名称🙂  ")
    })?;
    check(c.node(id)?.focused, "Acceptance lost native focus")?;
    // The new local epoch cannot undo an acknowledged consumer value.
    c.input(window, json!({"kind":"key","key":"undo"}))?;
    check(
        c.node(id)?.value() == "  名称🙂  ",
        "Local undo crossed acceptance",
    )?;
    c.input(window, json!({"kind":"key","key":"select_all"}))?;
    c.input(window, json!({"kind":"key","key":"backspace"}))?;
    c.input(window, json!({"kind":"key","key":"enter"}))?;
    c.until("rejected empty text draft", || {
        Ok(c.node("fixture/text/diagnostic")?
            .caption()
            .contains("cannot be empty"))
    })?;
    check(
        c.node(id)?.value().is_empty() && c.node(id)?.focused,
        "Rejection lost native draft/focus",
    )?;
    c.frame(window, "text acceptance rejection")?;
    c.input(window, json!({"kind":"key","key":"escape"}))?;
    c.until("native text cancellation", || {
        Ok(c.node(id)?.value() == "  名称🙂  ")
    })?;
    check(
        c.node(id)?.focused && c.node(id)?.entity == original.entity,
        "Cancellation blurred/rebuilt the field",
    )?;
    c.frame(window, "text acceptance cancellation")?;
    // Restore the initial independent value for the rest of the presentation run.
    c.field(id, "Draft value")?;
    c.input(window, json!({"kind":"key","key":"enter"}))?;
    c.until("independent text reset", || {
        Ok(c.node("fixture/text/diagnostic")?.caption() == "Accepted: Draft value")
    })?;
    Ok(())
}

fn section_order(c: &Editor<'_>, toggle: &str, expanded: bool) -> Result<()> {
    let prefix = toggle
        .strip_suffix("toggle")
        .ok_or("Section toggle has no local scope")?;
    let icon = c.node(&format!(
        "{prefix}{}",
        if expanded { "down" } else { "right" }
    ))?;
    let title = c.node(&format!("{prefix}title"))?;
    check(
        icon.rect[2] <= title.rect[0] + 1.,
        "Section disclosure must precede its title, matching the canonical kit",
    )
}

fn inline_transform(c: &Editor<'_>, prefix: &str) -> Result<Vec<Node>> {
    let layout = c.nodes()?;
    let nodes = inputs(c, prefix)?;
    let mut result = Vec::new();
    for group in ["translation", "rotation", "scale"] {
        let axes = ["x", "y", "z"]
            .into_iter()
            .map(|axis| {
                nodes
                    .iter()
                    .find(|node| node.id.ends_with(&format!("/{group}/{axis}/value")))
                    .cloned()
                    .ok_or_else(|| format!("Missing {group} {axis} input").into())
            })
            .collect::<Result<Vec<Node>>>()?;
        let label = layout
            .iter()
            .find(|node| {
                node.id.starts_with(prefix) && node.id.ends_with(&format!("/{group}/label"))
            })
            .ok_or("Missing Transform property label")?;
        check(
            label.rect[1] <= axes[0].center()[1]
                && axes[0].center()[1] <= label.rect[3]
                && label.rect[2] <= axes[0].rect[0] + 1.,
            "Property label must stay left of its input on the same line at every panel width",
        )?;
        for (index, axis) in axes.iter().enumerate() {
            let row = layout
                .iter()
                .find(|node| {
                    node.id.starts_with(prefix)
                        && node
                            .id
                            .ends_with(&format!("/{group}/axis-{}", ["x", "y", "z"][index]))
                })
                .ok_or("Missing native vector axis row")?;
            check(
                row.rect[1] <= axis.rect[1] && axis.rect[3] <= row.rect[3],
                "Vector input escaped its owning axis control",
            )?;
            check(
                (axis.rect[1] - axes[0].rect[1]).abs() <= 1.,
                "Vector axes wrapped instead of staying inline",
            )?;
            check(
                axis.rect[2] - axis.rect[0] > 10.,
                "Vector axis input has no usable width",
            )?;
            if index > 0 {
                check(
                    axes[index - 1].rect[2] <= axis.rect[0] + 1.,
                    "Vector axes overlap or are reordered",
                )?;
            }
        }
        if group == "translation" {
            check(
                axes.iter().map(Node::value).collect::<Vec<_>>() == ["12.5", "23.75", "-0.125"],
                "Vector specialization changed native axis values",
            )?;
        }
        if group == "rotation" {
            check(
                axes.iter().all(|node| node.value() == "0"),
                "Identity quaternion is not zero degrees",
            )?;
        }
        result.extend(axes);
    }
    Ok(result)
}

pub(super) fn ui(c: &Editor<'_>, compound: bool) -> Result<Value> {
    let project = c.open_project()?;
    let assets = project.join("assets");
    let inspector = c.view("inspector")?;
    let hierarchy = c.view("hierarchy")?;
    let prefix = format!("inspector/ViewId({inspector})/");
    let search = c.find("Assets search", |node| {
        node.id.starts_with("browser/") && node.id.ends_with("/search")
    })?;
    c.field(&search.id, "Hud.prefab.ron")?;
    c.open_asset("Hud.prefab.ron")?;
    c.find("source-owned Save status", |node| {
        node.caption()
            .contains("Source open: assets/Hud.prefab.ron")
            && node.caption().contains("in memory until Save")
    })?;
    if compound {
        compound_collections::run(c, &assets, &inspector, &hierarchy, &prefix)?;
    } else {
        caption::select_title(c, &hierarchy)?;
        layout::run(c, &inspector, &prefix)?;
        collections::run(c, &assets, &inspector, &prefix)?;
    }
    Ok(json!({"status":"passed","project":project,"frames":c.captures.borrow().clone()}))
}

pub(super) fn run(c: &Editor<'_>) -> Result<Value> {
    let project = c.open_project()?;
    let assets = project.join("assets");
    let reference = c.reference("inspector")?;
    let (main, other) = (text(&reference["main"])?, text(&reference["other"])?);
    let before = inventory(&assets)?;
    c.browser_folder("Inspector")?;
    let inspector = c.view("inspector")?;
    let hierarchy = c.view("hierarchy")?;
    let p = format!("inspector/ViewId({inspector})/");
    c.until("selected folder context without document Open", || {
        let state = c.state()?;
        Ok(c.optional(&format!("{p}name"))?
            .is_some_and(|node| node.caption() == "Inspector")
            && state["documents"]["active"].is_null()
            && array(&state["documents"]["documents"])?.is_empty()
            && array(&state["selection"]["items"])?.len() == 1
            && state["selection"]["items"][0]["Asset"]["path"] == "game://Inspector")
    })?;
    c.open_asset("Main.prefab.ron")?;
    c.menu(&inspector, "Use default reflected view")?;
    c.until("main source activation", || {
        Ok(c.state()?["documents"]["active"] == main)
    })?;
    let label = c.find("nested source node", |n| {
        n.id.starts_with(&format!("hierarchy/ViewId({hierarchy})/"))
            && n.caption() == "Repeated name"
    })?;
    let row = c
        .nodes()?
        .into_iter()
        .find(|n| Some(n.entity) == label.parent)
        .ok_or("Missing nested hierarchy row")?;
    c.click(&row.id)?;
    c.until("effective overridden field", || {
        Ok(inputs(c, &p)?
            .iter()
            .any(|n| n.value() == "Override 日本語"))
    })?;
    let field = inputs(c, &p)?
        .into_iter()
        .find(|n| n.value() == "Override 日本語")
        .ok_or("Missing override field")?;
    readonly(c, &field)?;
    c.menu(&inspector, "Lock Inspector")?;
    c.open_asset("Other.prefab.ron")?;
    c.until("other source activation", || {
        Ok(c.state()?["documents"]["active"] == other)
    })?;
    check(
        c.wait_node(&field.id)?.entity == field.entity,
        "Locked Inspector lost source identity",
    )?;
    c.menu(&inspector, "Search properties")?;
    c.field(&format!("{p}search"), "text")?;
    let search_id = format!("{p}search");
    let search_entity = c.node(&search_id)?.entity;
    c.input(
        c.node(&search_id)?.window()?,
        json!({"kind":"key","key":"undo"}),
    )?;
    c.until("Inspector search local undo", || {
        Ok(c.node(&search_id)?.value() == "")
    })?;
    c.frame(
        c.node(&search_id)?.window()?,
        "Inspector search after local undo",
    )?;
    c.input(
        c.node(&search_id)?.window()?,
        json!({"kind":"key","key":"redo"}),
    )?;
    c.until("Inspector search local redo", || {
        Ok(c.node(&search_id)?.value() == "text")
    })?;
    check(
        c.node(&search_id)?.entity == search_entity,
        "Search history reconstructed the native input",
    )?;
    check(
        c.node(&format!("{p}name"))?.caption() == "Repeated name",
        "Property search lost source context",
    )?;
    let search = c.node(&format!("{p}search"))?;
    c.frame(c.window()?, "locked property search")?;
    c.menu(&inspector, "Float group")?;
    c.until("floating Inspector", || {
        Ok(c.node(&search.id)?.window()? != c.window()?)
    })?;
    let window = c.node(&search.id)?.window()?;
    c.resize(window, 320, 640, 1)?;
    c.click(&search.id)?;
    c.input(window, json!({"kind":"key","key":"undo"}))?;
    c.until("floating search local undo", || {
        Ok(c.node(&search.id)?.value() == "")
    })?;
    c.input(window, json!({"kind":"key","key":"redo"}))?;
    c.until("floating search local redo", || {
        Ok(c.node(&search.id)?.value() == "text")
    })?;
    c.frame(window, "narrow Inspector")?;
    check(
        c.node(&field.id)?.entity == field.entity && c.node(&search.id)?.entity == search.entity,
        "Float rebuilt Inspector",
    )?;
    c.menu(&inspector, "Redock to main window")?;
    c.click("workspace/reset")?;
    c.until("isolated Inspector group", || {
        Ok(
            c.state()?["layout"]["nodes"][c.group(&inspector)?]["Group"]["tabs"]
                == json!([inspector]),
        )
    })?;
    c.menu(&inspector, "Hide group")?;
    c.until("hidden Inspector", || c.hidden(&inspector))?;
    c.click("workspace/panels")?;
    let show = c.find("Show Inspector", |n| n.caption() == "Show Inspector")?;
    c.click(show.id.strip_suffix("/caption").unwrap_or(&show.id))?;
    check(
        c.wait_node(&search.id)?.entity == search.entity
            && c.node(&field.id)?.entity == field.entity,
        "Reopen rebuilt Inspector",
    )?;
    c.click(&format!("{p}close-search"))?;
    c.click("workspace/reset")?;
    let s = c.state()?;
    let doc = array(&s["documents"]["documents"])?
        .iter()
        .find(|d| d["document"] == other)
        .ok_or("Missing other document")?;
    c.click(&Editor::label(text(&doc["view"])?))?;
    c.until("other document reactivation", || {
        Ok(c.state()?["documents"]["active"] == other)
    })?;
    c.menu(&inspector, "Unlock Inspector")?;
    c.until("unlocked source following", || {
        Ok(inputs(c, &p)?.iter().any(|n| n.value() == "Other source"))
    })?;
    c.menu(&inspector, "Use custom presentations")?;
    transform::run(c, &inspector, &hierarchy, &p)?;
    check(
        inventory(&assets)? == before,
        "Read-only Inspector changed source bytes/sidecars",
    )?;
    spatial::run(c, &assets, &inspector, &hierarchy, &p)?;
    resources::run(c, &assets, &inspector, &hierarchy, &p)?;
    let saved_spatial = inventory(&assets)?;
    caption::run(c, &inspector, &hierarchy, &p)?;
    check(
        inventory(&assets)? == saved_spatial,
        "Caption editing changed files before Save",
    )?;
    saving::run(c, &assets, &inspector, &p)?;
    Ok(json!({"status":"passed","project":project,"frames":c.captures.borrow().clone()}))
}
