//! Native project-manager modal input and source lifecycle.
use super::support::*;
use kedra_test::Result;
use serde_json::{Value, json};
use std::fs;

pub(super) fn run(c: &Editor<'_>) -> Result<Value> {
    c.until("project manager", || {
        Ok(c.rpc("editor.state")?["phase"] == "projects")
    })?;
    check(
        c.rpc("editor.state")?["entry"].is_null(),
        "Start with closed startup forms",
    )?;
    let mut runs = Vec::new();
    for dpr in [1, 2] {
        c.resize(c.window()?, 1120, 720, dpr)?;
        let mut run = json!({"dpr":dpr,"frames":{}});
        run["frames"]["manager"] = c.frame(c.window()?, "manager")?;
        c.click("projects.new")?;
        c.until("create form", || {
            Ok(!c.rpc("editor.state")?["entry"].is_null())
        })?;
        c.until("modal autofocus", || Ok(c.node("entry.name")?.focused))?;
        c.key("tab")?;
        check(c.node("entry.directory")?.focused, "Tab escaped modal")?;
        c.key("shift_tab")?;
        check(c.node("entry.name")?.focused, "Reverse Tab escaped modal")?;
        run["frames"]["create"] = c.frame(c.window()?, "create")?;
        c.field("entry.name", "QA Кедра")?;
        check(
            c.rpc("editor.state")?["entry"]["name"] == "QA Кедра",
            "Unicode native entry lost",
        )?;
        c.field("entry.name", "invalid/name")?;
        c.key("enter")?;
        c.until("validation failure", || {
            Ok(c.rpc("editor.state")?["entry"]["error"]
                .as_str()
                .is_some_and(|error| !error.is_empty()))
        })?;
        check(
            c.rpc("editor.state")?["entry"]["name"] == "invalid/name",
            "Validation discarded invalid draft",
        )?;
        run["validation"] = c.rpc("editor.state")?["entry"]["error"].clone();
        run["frames"]["invalid"] = c.frame(c.window()?, "invalid create")?;
        c.key("escape")?;
        c.until("cancel form", || {
            Ok(c.rpc("editor.state")?["entry"].is_null())
        })?;
        c.click("projects.open")?;
        c.until("open form", || {
            Ok(!c.rpc("editor.state")?["entry"].is_null())
        })?;
        run["frames"]["open"] = c.frame(c.window()?, "open")?;
        c.click("entry.cancel")?;
        c.until("open form cancellation", || {
            Ok(c.rpc("editor.state")?["entry"].is_null())
        })?;
        runs.push(run);
    }
    let parent = c.state_dir.join("lifecycle");
    fs::create_dir_all(&parent)?;
    let project = c.project.ok_or("Missing shared project copy")?;
    let created = parent.join("Кедра QA");
    let mut steps = Vec::new();
    for command in [
        json!({"kind":"open","root":project}),
        // Creation is the feature under test, not a scenario-data generator.
        json!({"kind":"create","parent":parent,"name":"Кедра QA"}),
        json!({"kind":"open","root":project}),
    ] {
        let old_window = c.window()?;
        c.client
            .operation("editor.project", json!({"command":command}))?;
        let expected = if command["kind"] == "create" {
            &created
        } else {
            project
        };
        check(
            c.rpc("editor.state")?["project"]["root"] == json!(expected),
            "Project root differs",
        )?;
        check(
            expected.join("kedra.toml").is_file() && expected.join("assets").is_dir(),
            "Project source creation incomplete",
        )?;
        c.until("editor host", || {
            Ok(c.rpc("editor.state")?["phase"] == "editor")
        })?;
        check(
            c.window()? != old_window,
            "Native manager window was retained",
        )?;
        let frame = c.frame(c.window()?, "lifecycle editor")?;
        check(
            c.client
                .input(old_window, json!({"kind":"move","position":[10,10]}))
                .is_err(),
            "Retired native window admitted input",
        )?;
        // Prove the transport remains live, rather than accepting a connection failure.
        c.event(json!({"kind":"move","position":[10,10]}))?;
        steps.push(
            json!({"command":command["kind"],"frame":frame,"intermediate_loading_required":false}),
        );
        c.client
            .operation("editor.project", json!({"command":{"kind":"projects"}}))?;
        c.until("manager restored", || {
            Ok(c.rpc("editor.state")?["phase"] == "projects")
        })?;
    }
    for command in [
        json!({"kind":"create","parent":parent,"name":"Кедра QA"}),
        json!({"kind":"open","root":parent.join("missing")}),
    ] {
        let receipt = c.client.receipt(
            c.client
                .submit("editor.project", json!({"command":command}))?,
        )?;
        check(
            receipt["state"] == "failed",
            "Invalid project request did not fail",
        )?;
        check(
            c.rpc("editor.state")?["phase"] == "projects",
            "Rejected source request changed phase",
        )?;
        steps.push(json!({"command":command["kind"],"expected_failure":true}));
    }
    Ok(json!({"status":"passed","runs":runs,"lifecycle":{"project":project,"steps":steps}}))
}
