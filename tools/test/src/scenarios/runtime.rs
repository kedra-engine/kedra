//! Concrete shared-project switch/projection verifiers, not arbitrary-manifest runners.
//! Reusable Cargo/process/transport/readback mechanisms remain in kedra_test.
use kedra_test::{Client, Result, evidence, process::NativeProcess};
use serde_json::{Value, json};
use std::{path::Path, process::Command, time::Duration};

fn exercise(client: &Client, artifacts: &Path) -> Result<Value> {
    client.wait("independent window", Duration::from_secs(30), || {
        Ok(client.observe()?["windows"]
            .as_array()
            .is_some_and(|w| w.len() == 1)
            .then_some(()))
    })?;
    let result = client.scenario("scene.switch", Value::Null)?;
    let before = &result["before"];
    let after = &result["after"];
    // Luau has one numeric type: integral coordinates may return as JSON
    // integers while native observation serializes floats. Compare geometry,
    // not the JSON number representation; decoding still rejects malformed data.
    let before_rect: [f64; 4] = serde_json::from_value(before["rect"].clone())?;
    if before["background"] != json!([180, 40, 30, 255])
        || before_rect != [40.0, 50.0, 160.0, 114.0]
        || before["visible"] != true
    {
        return Err(format!("Unexpected initial native switch state: {before}").into());
    }
    if after["background"] != json!([30, 100, 190, 255]) || after["visible"] != true {
        return Err("Native scene script did not activate the control".into());
    }
    if before["id"] != "game.switch"
        || after["id"] != "game.switch"
        || !before["entity"].is_string()
        || after["entity"] != before["entity"]
    {
        return Err("Native click rebuilt its retained control".into());
    }
    let observation = client.observe()?;
    let live = observation["elements"]
        .as_array()
        .ok_or("Missing live UI inventory")?
        .iter()
        .find(|node| node["id"] == "game.switch" && node["visible"] == true)
        .ok_or("Script result has no matching live control")?;
    let entity = after["entity"]
        .as_str()
        .ok_or("Missing script entity identity")?
        .parse::<u64>()?;
    let live_rect: [f64; 4] = serde_json::from_value(live["rect"].clone())?;
    let after_rect: [f64; 4] = serde_json::from_value(after["rect"].clone())?;
    if live["entity"].as_u64() != Some(entity)
        || live["background"] != after["background"]
        || live_rect != after_rect
    {
        return Err("Script result differs from independent native observation".into());
    }
    let before_frame = evidence::consume_fence(
        serde_json::from_value(result["before_frame"].clone())?,
        artifacts,
        [320, 320],
    )?;
    let after_frame = evidence::consume_fence(
        serde_json::from_value(result["after_frame"].clone())?,
        artifacts,
        [320, 320],
    )?;
    let renderer = client.rpc("game.state", Value::Null)?["renderer"].clone();
    if renderer["backend"].as_str().is_none_or(str::is_empty)
        || renderer["name"].as_str().is_none_or(str::is_empty)
    {
        return Err("Missing native renderer identity".into());
    }
    Ok(
        json!({"verification":"behavior_only","input_execution":"in_application_luau","before":before_frame,"after":after_frame,
        "renderer":renderer,"input":{"kind":"click","element":"game.switch","accepted":true,"retained":true}}),
    )
}

pub(crate) fn run_switch(
    manifest: &Path,
    assets: &Path,
    project_assets: &Path,
    output: &Path,
) -> Result<()> {
    run(manifest, assets, project_assets, output, false)
}

fn group<'a>(state: &'a Value, name: &str) -> Result<&'a Value> {
    let id = state["views"][name]["group"]
        .as_str()
        .ok_or("View has no live group")?;
    state["snapshot"]["nodes"][id]
        .get("Group")
        .ok_or_else(|| "Missing native group".into())
}

fn maximized(state: &Value) -> Result<&Value> {
    let hosts = state["snapshot"]["hosts"]
        .as_object()
        .ok_or("Missing hosts")?;
    if hosts.len() != 1 {
        return Err("Independent projection requires exactly one host".into());
    }
    Ok(&hosts.values().next().ok_or("Missing host")?["maximized"])
}

fn exercise_projection(client: &Client, artifacts: &Path) -> Result<Value> {
    let result = client.scenario("projection.exercise", Value::Null)?;
    let before = &result["before"];
    let steps = result["steps"]
        .as_array()
        .ok_or("Missing projection steps")?;
    let expected = [
        "reorder",
        "split",
        "activate",
        "maximize",
        "maximize",
        "visibility",
        "visibility",
        "restore",
    ];
    if steps.len() != expected.len() {
        return Err("Incomplete native projection sequence".into());
    }
    let mut fences = Vec::new();
    let mut verified_steps = Vec::new();
    for (step, action) in steps.iter().zip(expected) {
        let state = &step["state"];
        if step["action"] != action || state["projected"] != true {
            return Err("Missing projected native action result".into());
        }
        for name in ["A", "B", "C", "D"] {
            let content = before["views"][name]["content"]
                .as_str()
                .ok_or("Missing initial retained identity")?;
            if state["views"][name]["content"] != content {
                return Err("Projection action rebuilt retained content".into());
            }
        }
        fences.push(evidence::consume_fence(
            serde_json::from_value(step["frame"].clone())?,
            artifacts,
            [1000, 800],
        )?);
        verified_steps.push(json!({"action":action,"state":state}));
    }
    let reorder = &steps[0]["state"];
    let split = &steps[1]["state"];
    let activated = &steps[2]["state"];
    let hidden = &steps[5]["state"];
    let shown = &steps[6]["state"];
    if group(before, "C")?["tabs"]
        != json!([before["views"]["C"]["id"], before["views"]["D"]["id"]])
        || group(reorder, "D")?["tabs"]
            != json!([before["views"]["D"]["id"], before["views"]["C"]["id"]])
        || split["views"]["D"]["group"] == split["views"]["A"]["group"]
        || split["views"]["D"]["group"] == split["views"]["C"]["group"]
        || group(activated, "B")?["active"] != before["views"]["B"]["id"]
        || maximized(&steps[3]["state"])? != &steps[3]["state"]["views"]["D"]["group"]
        || !maximized(&steps[4]["state"])?.is_null()
        || hidden["views"]["D"]["hidden"] != true
        || !hidden["views"]["D"]["group"].is_null()
        || shown["views"]["D"]["hidden"] != false
        || shown["views"]["D"]["group"] != shown["views"]["A"]["group"]
        || group(shown, "D")?["tabs"][0] != before["views"]["D"]["id"]
        || steps[7]["state"]["snapshot"] != before["snapshot"]
    {
        return Err("Independent native projection topology assertions failed".into());
    }
    // Independently inspect the newly created right-hand split, not just group separation.
    let split_nodes = split["snapshot"]["nodes"]
        .as_object()
        .ok_or("Missing split topology")?;
    if !split_nodes.values().any(|node| {
        let node = &node["Split"];
        node["axis"] == "Horizontal"
            && node["fraction"] == 0.5
            && node["first"] == split["views"]["A"]["group"]
            && node["second"] == split["views"]["D"]["group"]
    }) {
        return Err("D was not split to the right of A at half width".into());
    }
    let live = client.rpc("projection.state", Value::Null)?;
    if live != result["after"] {
        return Err("Projection result differs from independent native observation".into());
    }
    let observation = client.observe()?;
    let draft = observation["elements"]
        .as_array()
        .ok_or("Missing UI inventory")?
        .iter()
        .find(|node| node["id"] == "lab.D/draft" && node["visible"] == true)
        .ok_or("Restored draft is not visible")?;
    let entity = result["draft_before"]["entity"]
        .as_str()
        .ok_or("Missing draft identity")?
        .parse::<u64>()?;
    if draft["entity"].as_u64() != Some(entity)
        || result["draft"]["entity"] != result["draft_before"]["entity"]
        || draft["value"] != "Retained 世界 draft"
        || result["draft"]["value"] != draft["value"]
    {
        return Err("Native projection lost its retained Unicode draft".into());
    }
    fences.push(evidence::consume_fence(
        serde_json::from_value(result["frame"].clone())?,
        artifacts,
        [1000, 800],
    )?);
    let renderer = client.rpc("game.state", Value::Null)?["renderer"].clone();
    if renderer["backend"].as_str().is_none_or(str::is_empty)
        || renderer["name"].as_str().is_none_or(str::is_empty)
    {
        return Err("Missing independent renderer identity".into());
    }
    Ok(
        json!({"verification":"behavior_only", "input_execution":"in_application_luau",
        "input":{"accepted":true,"retained":true}, "renderer":renderer,
        "steps":verified_steps, "fences":fences, "draft":draft, "state":live}),
    )
}

pub(crate) fn run_projection(
    manifest: &Path,
    assets: &Path,
    project_assets: &Path,
    output: &Path,
) -> Result<()> {
    run(manifest, assets, project_assets, output, true)
}

fn run(
    manifest: &Path,
    assets: &Path,
    project_assets: &Path,
    output: &Path,
    projection: bool,
) -> Result<()> {
    let manifest = manifest.canonicalize()?;
    let assets = assets.canonicalize()?;
    let project_assets = project_assets.canonicalize()?;
    // This independent workspace is outside the root Cargo test inventory.
    // Run its host-adapter contracts on every acceptance route, not an optional
    // undocumented command. The caller supplies a complete locked working copy.
    eprintln!("Independent runtime host: checking projection readiness contracts…");
    let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(manifest.parent().ok_or("Manifest parent missing")?)
        .args(["test", "--locked", "--manifest-path"])
        .arg(&manifest)
        .args(["--bin", "kedra-test-project", "docking::scripting::tests"])
        .status()?;
    if !status.success() {
        return Err(
            "Independent runtime host contracts failed; native acceptance not started".into(),
        );
    }
    let host = kedra_test::cargo::build(
        &manifest,
        "kedra-test-project",
        "kedra-test-project",
        false,
        &[],
    )?;
    kedra_test::process::ensure_desktop()?;
    let case = if projection { "projection" } else { "switch" };
    let mut report = kedra_test::suite::capture_twice(
        output,
        &[case.into()],
        |_, private| {
            let artifacts = private.join("frames");
            let mut command = Command::new(&host);
            command.arg("--assets").arg(&assets);
            command.arg("--project-assets").arg(&project_assets);
            if projection {
                command.args(["--scene", "editor://fixtures/projection.scene.ron"]);
            }
            command
                .current_dir(output)
                .args(["--remote-port", "0", "--remote-credentials"])
                .arg(private.join("session.json"))
                .arg("--remote-artifacts")
                .arg(&artifacts);
            let mut native = NativeProcess::spawn(&mut command, private)?;
            let client = native.connect()?;
            Ok((native, client, artifacts))
        },
        |client, artifacts, _, _| {
            if projection {
                exercise_projection(client, artifacts)
            } else {
                exercise(client, artifacts)
            }
        },
        |client| {
            // Standard native close policy; retirement can precede its receipt.
            // capture_twice independently requires successful child termination.
            let _ = client.input(client.window()?, json!({"kind":"close_window"}));
            Ok(())
        },
    )?;
    report["verification"] = json!("behavior_only");
    let validation = (|| -> Result<()> {
        kedra_test::suite::verify_case(&report, case, &[])?;
        let runs = report["cases"][case]["runs"]
            .as_array()
            .ok_or("Missing native runs")?;
        if runs
            .iter()
            .any(|run| run["input"]["accepted"] != true || run["input"]["retained"] != true)
        {
            return Err("Incomplete native input evidence".into());
        }
        report["cases"][case]["status"] = json!("passed");
        report["result"] = json!("passed");
        Ok(())
    })();
    if let Err(error) = &validation {
        report["error"] = json!(error.to_string());
    }
    kedra_test::report::write(&output.join("public"), &report)?;
    eprintln!("Independent native behavior: {}", report["result"]);
    validation
}
