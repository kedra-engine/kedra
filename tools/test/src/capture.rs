//! Editor-owned readiness, specimen input and source-state calibration.
use crate::catalog::{FIELD, Scenario};
use kedra_test::{Client, Result, evidence};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

fn setup(client: &Client, scenario: &Scenario) -> Result<u64> {
    client.wait(
        "production template dependencies",
        Duration::from_secs(40),
        || Ok((client.rpc("catalog.state", Value::Null)?["ready"] == true).then_some(())),
    )?;
    let window = client.window()?;
    client.input(window, json!({"kind":"button","pressed":false}))?;
    client.rpc(
        "catalog.show",
        json!({"source":scenario.source,"disabled":scenario.state=="disabled"}),
    )?;
    let serialized_window = window.to_string();
    let delivered = client.scenario(
        "catalog.prepare",
        json!({
            "window":serialized_window.as_str(), "state":scenario.state, "field":scenario.source == FIELD,
        }),
    )?;
    if delivered["delivered"] != true
        || delivered["window"].as_str() != Some(serialized_window.as_str())
    {
        return Err(
            "Component input sequence returned a foreign window or incomplete delivery".into(),
        );
    }
    client.wait("explicit renderer DPR", Duration::from_secs(12), || {
        Ok(client.observe()?["windows"]
            .as_array()
            .and_then(|windows| {
                windows
                    .iter()
                    .find(|w| w["id"] == window && w["dpr"].as_f64() == Some(1.))
            })
            .cloned())
    })?;
    Ok(window)
}

fn calibrated(client: &Client, scenario: &Scenario) -> Result<bool> {
    let observation = client.observe()?;
    let nodes = observation["elements"]
        .as_array()
        .ok_or("Missing native inventory")?;
    let visible = |id| {
        nodes
            .iter()
            .find(|node| node["id"] == id && node["visible"] == true)
    };
    let Some(viewport) = visible("specimen.viewport") else {
        return Ok(false);
    };
    let rect = &viewport["rect"];
    if rect[0].as_f64() != Some(32.)
        || rect[1].as_f64() != Some(32.)
        || rect[2].as_f64().zip(rect[0].as_f64()).map(|(a, b)| a - b)
            != Some(f64::from(scenario.width))
    {
        return Ok(false);
    }
    let text = if let Some(field) = visible("specimen.input") {
        field["value"].as_str().unwrap_or("")
    } else {
        visible("specimen.caption")
            .and_then(|node| node["text"].as_str())
            .unwrap_or("")
    };
    if text != scenario.text() {
        return Ok(false);
    }
    let native = client.rpc("catalog.state", Value::Null)?;
    if native["selected"] != scenario.source || native["document"] != scenario.document {
        return Ok(false);
    }
    let proof = &native["rendered_state"];
    if ["hovered", "pressed", "focused", "focus_visible", "disabled"]
        .iter()
        .any(|key| !proof[key].is_boolean())
    {
        return Ok(false);
    }
    if proof["disabled"] != (scenario.state == "disabled")
        || proof["pressed"] != (scenario.state == "pressed")
        || proof["hovered"] != matches!(scenario.state, "hover" | "pressed")
    {
        return Ok(false);
    }
    if scenario.state == "focus" {
        if proof["focused"] != true || proof["focus_visible"] != true {
            return Ok(false);
        }
        if scenario.source == FIELD && proof["text_selected"] != false {
            return Ok(false);
        }
    } else if scenario.state != "pressed" && proof["focused"] == true {
        return Ok(false);
    }
    Ok(true)
}

pub fn actual_profile(client: &Client, policy: &Value) -> Result<Value> {
    let renderer = client.rpc("catalog.state", Value::Null)?["renderer"].clone();
    if renderer["backend"].as_str().is_none_or(|s| s.is_empty())
        || renderer["name"].as_str().is_none_or(|s| s.is_empty())
    {
        return Err("Native renderer identity is missing".into());
    }
    let reviewed: Value = serde_json::from_str(
        policy["profile"]["renderer"]
            .as_str()
            .ok_or("Missing policy renderer")?,
    )?;
    if renderer != reviewed {
        return Err("Unreviewed native renderer profile".into());
    }
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    let environment = if cfg!(target_os = "macos") {
        let output = Command::new("sw_vers").arg("-productVersion").output()?;
        if !output.status.success() {
            return Err("Cannot establish macOS profile".into());
        }
        let version = String::from_utf8(output.stdout)?.trim().to_string();
        let machine = if std::env::consts::ARCH == "aarch64" {
            "arm64-arm"
        } else {
            "x86_64-i386"
        };
        format!(
            "macOS-{version}-{machine}-64bit-Mach-O/{}/{}",
            std::env::var("ImageVersion").unwrap_or_else(|_| "local".into()),
            std::env::var("KEDRA_TEST_ENVIRONMENT")
                .unwrap_or_else(|_| "uncontrolled-packages".into())
        )
    } else {
        return Err("No verified Rust environment identity adapter for this platform".into());
    };
    // Renderer fields were independently compared above. Retain the reviewed
    // string encoding, but never inherit unmeasured extra profile attributes.
    Ok(
        json!({"id":policy["profile"]["id"],"renderer":policy["profile"]["renderer"],
        "platform":platform,"environment":environment,"logical_size":[640,360],
        "dpr":1,"color_space":"srgb_assumed","motion":"reduced"}),
    )
}

pub fn scenario(
    client: &Client,
    artifacts: &Path,
    public: &Path,
    repeat: usize,
    scenario: &Scenario,
    policy: &Value,
) -> Result<Value> {
    let result = (|| -> Result<Value> {
        let window = setup(client, scenario)?;
        client.wait(
            "fixed source/state/slot calibration",
            Duration::from_secs(12),
            || Ok(calibrated(client, scenario)?.then_some(())),
        )?;
        let profile = actual_profile(client, policy)?;
        if profile != policy["profile"] {
            return Err("Native environment differs from reviewed profile".into());
        }
        let mut run = json!({});
        for layer in ["full", "shape", "restored"] {
            let paint = if layer == "shape" { "shape" } else { "full" };
            eprintln!("{} process {repeat}/2 · {layer}", scenario.id);
            client.wait(
                "measured scoped native text",
                Duration::from_secs(12),
                || {
                    let snapshot = client.rpc("catalog.text_capture", json!({"layer":paint}))?;
                    Ok((snapshot["ready"] == true).then_some(snapshot))
                },
            )?;
            let reason=(layer=="shape").then_some("Glyph-free diagnostic; nonuniform full frames, retained layout and exact restoration independently required");
            let (frame, hash) = evidence::stable(client, window, artifacts, [640, 360], reason)?;
            let snapshot = client.rpc("catalog.text_capture", json!({"layer":paint}))?;
            if !calibrated(client, scenario)? {
                return Err("Source/state changed during capture".into());
            }
            let relative = PathBuf::from(format!("{}/run-{repeat}/{layer}.png", scenario.id));
            let mut record =
                serde_json::to_value(evidence::publish(&frame, &hash, public, &relative)?)?;
            record["profile"] = profile.clone();
            record["snapshot"] = snapshot;
            std::fs::write(
                public.join(relative.with_extension("json")),
                serde_json::to_vec_pretty(&record)?,
            )?;
            run[layer] = record;
        }
        kedra_test::text::layers(&run["full"]["snapshot"], &run["shape"]["snapshot"])?;
        if run["full"]["sha256"] != run["restored"]["sha256"]
            || run["full"]["snapshot"] != run["restored"]["snapshot"]
        {
            return Err("Full-paint restoration changed original pixels or typography".into());
        }
        Ok(run)
    })();
    // Best effort reset even on a failed case. Next setup remounts the source.
    if let Ok(window) = client.window() {
        let _ = client.input(window, json!({"kind":"button","pressed":false}));
    }
    let _ = client.rpc("catalog.text_capture", json!({"layer":"full"}));
    result
}
