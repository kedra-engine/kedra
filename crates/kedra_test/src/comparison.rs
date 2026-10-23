//! Adapt native evidence to the single maintained offline visual comparator.
use crate::{Result, evidence};
use serde_json::{Value, json};
use std::{fs, path::Path};

/// Compare original full-canvas PNGs using an explicit independent local region.
/// `config` is the existing versioned visual policy, not executable scenario data.
/// Reserves a new directory, preserves both originals and returns the raw engine
/// report for matches and mismatches. Invalid evidence/policy returns an error.
/// This never grants known-difference acceptance or rewrites reference hashes.
pub fn compare(
    root: &Path,
    relative: &Path,
    reference: &Path,
    actual: &Path,
    mut config: Value,
    capture: &Value,
) -> Result<Value> {
    if relative
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("Comparison directory must be relative".into());
    }
    let directory = root.join(relative);
    if let Some(parent) = directory.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&directory)?;
    let reference_bytes = evidence::read(reference, 64 * 1024 * 1024)?;
    let actual_bytes = evidence::read(actual, 64 * 1024 * 1024)?;
    if config["reference"]["sha256"] != evidence::digest(&reference_bytes)
        || capture["sha256"] != evidence::digest(&actual_bytes)
    {
        return Err("Comparison input changed before publication".into());
    }
    fs::write(directory.join("reference.png"), reference_bytes)?;
    fs::write(directory.join("actual.png"), actual_bytes)?;
    config["reference"]["image"] = json!("reference.png");
    let evidence = json!({"schema":1,"scenario":config["scenario"],"fixture":config["fixture"],
        "profile":capture["profile"],"image":"actual.png","sha256":capture["sha256"],
        "receipt_frame":capture["receipt_frame"],"producer":"Native Rust scenario; original renderer frame",
        "readiness":"Three stable frames; project-owned input and readiness"});
    fs::write(
        directory.join("config.json"),
        serde_json::to_vec_pretty(&config)?,
    )?;
    fs::write(
        directory.join("actual.json"),
        serde_json::to_vec_pretty(&evidence)?,
    )?;
    let result = crate::visual::verify(
        &directory.join("config.json"),
        &directory.join("actual.json"),
        &directory.join("diff"),
    );
    let engine: Value = serde_json::from_slice(&evidence::read(
        &directory.join("diff/report.json"),
        1024 * 1024,
    )?)?;
    if !matches!(
        engine["automated_result"].as_str(),
        Some("matched" | "mismatch")
    ) {
        result?;
        return Err("Comparator returned no pixel verdict".into());
    }
    let domain = engine["domains"]
        .as_array()
        .ok_or("Missing comparison domains")?
        .iter()
        .find(|domain| domain["name"] == "region-0")
        .ok_or("Missing independently fixed local region")?;
    let prefix = relative
        .to_str()
        .ok_or("Public comparison path must be Unicode")?
        .replace('\\', "/");
    Ok(
        json!({"pixels":domain["perceptual_changed_pixels"],"eligible":domain["eligible_pixels"],
        "changed_percent":domain["changed_percent"],"automated_result":engine["automated_result"],
        "engine_exit":if result.is_ok(){0}else{1},"actual":format!("{prefix}/actual.png"),
        "reference":format!("{prefix}/reference.png"),"difference":format!("{prefix}/diff/difference.png"),
        "report":format!("{prefix}/diff/index.html")}),
    )
}
