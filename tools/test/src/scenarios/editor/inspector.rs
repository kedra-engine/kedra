//! Read-only Inspector specialization, effective source values and retained identity.
use super::support::*;
use crate::project::inventory;
use kedra_test::Result;
use serde_json::{Value, json};

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
    let p = "fixture/settings/";
    c.until("field specialization", || {
        Ok(inputs(c, p)?.iter().any(|n| n.value() == "25%"))
    })?;
    let values = inputs(c, p)?;
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
    let boolean = values
        .iter()
        .find(|n| n.value() == "true")
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
        c.node(&boolean.id)?.value() == "true",
        "Rejected Luau program changed host values",
    )?;
    c.client.scenario(
        "inspector.presentation",
        json!({"boolean":boolean.id,"gain":gain.id}),
    )?;
    c.until("host field refresh", || {
        Ok(c.node(&boolean.id)?.value() == "false")
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
    Ok(json!({"status":"passed","frames":c.captures.borrow().clone()}))
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
    c.wait_node(&format!("{p}message"))?;
    c.open_asset("Main.prefab.ron")?;
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
    check(
        inventory(&assets)? == before,
        "Read-only Inspector changed source bytes/sidecars",
    )?;
    Ok(json!({"status":"passed","project":project,"frames":c.captures.borrow().clone()}))
}
