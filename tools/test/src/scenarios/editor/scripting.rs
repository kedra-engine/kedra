//! Actual editor-owned Luau program; Rust supervises and validates GPU evidence.
use super::support::*;
use kedra_test::{Result, evidence};
use serde_json::{Value, json};

pub(super) fn run(c: &Editor<'_>) -> Result<Value> {
    let arguments = json!({"preferences":c.state_dir.join("recent-projects.toml")});
    // No source strings are admitted; a wrong scope precondition must also fail
    // before UI input and leave the manager usable for the real sequence.
    let mut refused = arguments.clone();
    refused["preferences"] = json!("not-the-isolated-preferences-file");
    check(
        c.client.scenario("editor.startup.forms", refused).is_err(),
        "Luau isolation precondition was ignored",
    )?;
    check(
        c.rpc("editor.state")?["entry"].is_null(),
        "Rejected script opened a form",
    )?;
    check(
        c.client.scenario("not-registered", Value::Null).is_err(),
        "Unknown Luau program was admitted",
    )?;
    let result = c.client.scenario("editor.startup.forms", arguments)?;
    let mut frames = Vec::new();
    for frame in array(&result["frames"])? {
        let dpr = frame["dpr"].as_u64().ok_or("Missing independent DPR")? as u32;
        check(matches!(dpr, 1 | 2), "Unexpected script DPR")?;
        let receipt: evidence::Frame = serde_json::from_value(frame["receipt"].clone())?;
        let size: [u32; 2] = serde_json::from_value(frame["size"].clone())?;
        check(
            size.iter().all(|side| *side > 0),
            "Missing measured viewport",
        )?;
        let evidence = evidence::consume_fence(receipt, c.artifacts, size)?;
        let mut value = serde_json::to_value(evidence)?;
        value["label"] = frame["label"].clone();
        frames.push(value);
    }
    check(
        frames.len() == 8,
        "Luau sequence omitted native frame evidence",
    )?;
    let project = c.open_project()?;
    Ok(
        json!({"verification":"behavior_only","language":"Luau","program":"editor.startup.forms","project":project,"frames":frames,"negative_cases":["scope-precondition","unknown-program"]}),
    )
}
