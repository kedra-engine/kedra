//! Offline visual regression. Capture producers remain separate from comparison policy.
mod compare;
mod config;
mod images;
mod report;
#[cfg(test)]
mod tests;

use config::{Animation, Approval, Config, Profile};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(clap::Args, Default)]
#[group(id = "visual_options")]
pub(crate) struct Options {
    /// Versioned visual scenario JSON (only for `visual`).
    #[arg(long)]
    config: Option<PathBuf>,
    /// Actual capture evidence JSON, not just an unlabelled PNG.
    #[arg(long)]
    actual: Option<PathBuf>,
    /// New artifact directory. Never overwrites an existing review or baseline.
    #[arg(long)]
    output: Option<PathBuf>,
}

impl Options {
    pub fn is_present(&self) -> bool {
        self.config.is_some() || self.actual.is_some() || self.output.is_some()
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    schema: u32,
    scenario: String,
    fixture: String,
    profile: Profile,
    image: PathBuf,
    sha256: String,
    producer: String,
    readiness: String,
    /// Producer's receipt frame, NOT a presentation timestamp or animation sample.
    receipt_frame: Option<u64>,
}

pub(crate) fn run(options: &Options, interactive: bool) -> crate::Result<()> {
    let config = options
        .config
        .as_deref()
        .ok_or("visual requires --config")?;
    let actual = options
        .actual
        .as_deref()
        .ok_or("visual requires --actual evidence.json")?;
    let output = options
        .output
        .as_deref()
        .ok_or("visual requires --output NEW_DIRECTORY")?;
    let mut session = crate::output::Session::new(interactive, "visual")?;
    let result = session.stage(
        1,
        1,
        "Visual regression (not design acceptance)",
        |session| {
            session.message("Validating policy, capture evidence and independent reference…")?;
            let result = verify(config, actual, output);
            session.message(&format!("Artifacts: {}", output.display()))?;
            result
        },
    );
    session.finish(1)?;
    result
}

fn parse<T: serde::de::DeserializeOwned>(path: &Path) -> crate::Result<T> {
    Ok(serde_json::from_slice(&images::read_bounded(
        path,
        1024 * 1024,
    )?)?)
}

/// Verification never writes inputs. All errors after directory creation retain a
/// machine-readable outcome; successful publication does not mean design approval.
fn verify(config_path: &Path, evidence_path: &Path, output: &Path) -> crate::Result<()> {
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    let mut status = "invalid_config";
    let result = process(config_path, evidence_path, output, &mut status);
    match result {
        Ok(matched) => {
            if matched {
                Ok(())
            } else {
                Err("visual mismatch; inspect report.json and index.html".into())
            }
        }
        Err(error) => {
            report::write_json(
                output.join("report.json"),
                &serde_json::json!({
                    "schema": 1, "automated_result": status, "design_acceptance": "not_granted",
                    "error": error.to_string(),
                }),
            )?;
            Err(error)
        }
    }
}

fn process(
    config_path: &Path,
    evidence_path: &Path,
    output: &Path,
    status: &mut &str,
) -> crate::Result<bool> {
    let config_bytes = images::read_bounded(config_path, 1024 * 1024)?;
    let config: Config = serde_json::from_slice(&config_bytes)?;
    config.validate()?;
    report::write_json(output.join("policy.json"), &config)?;
    *status = "unsupported";
    if matches!(config.animation, Animation::Keyframes | Animation::Live) {
        return Err("animation mode requires a verified host clock/source-frame contract; no static fallback".into());
    }
    *status = "invalid_capture";
    let evidence: Evidence = parse(evidence_path)?;
    if evidence.schema != 1
        || evidence.scenario != config.scenario
        || evidence.fixture != config.fixture
        || evidence.profile != config.profile
    {
        return Err(
            "capture schema/scenario/fixture/environment profile differs from configured reference"
                .into(),
        );
    }
    config::nonempty(&evidence.producer)?;
    config::nonempty(&evidence.readiness)?;
    let actual_path = images::resolve_image(evidence_path, &evidence.image)?;
    let actual_bytes = images::read_bounded(&actual_path, images::MAX_FILE_BYTES)?;
    fs::write(output.join("actual-source.png"), &actual_bytes)?;
    let actual_hash = images::digest(&actual_bytes);
    if actual_hash != evidence.sha256 {
        return Err("actual image digest differs from capture evidence".into());
    }
    let size = config.profile.physical_size()?;
    let actual = images::decode(&actual_bytes, size)?;
    images::validate_content(&actual, config.comparison.allow_uniform_reason.is_some())?;
    // Do not copy an unrestricted receipt or credentials into public artifacts.
    report::write_json(
        output.join("capture.json"),
        &Evidence {
            image: PathBuf::from("actual-source.png"),
            ..evidence
        },
    )?;

    *status = "missing_reference";
    let reference_path = images::resolve_image(config_path, &config.reference.image)?;
    let reference_bytes = images::read_bounded(&reference_path, images::MAX_FILE_BYTES)?;
    fs::write(output.join("reference-source.png"), &reference_bytes)?;
    *status = "invalid_reference";
    if images::digest(&reference_bytes) != config.reference.sha256 {
        return Err(
            "reference digest changed; review the independent source, never auto-update".into(),
        );
    }
    let reference_export = images::decode(&reference_bytes, config.reference.export_size)?;
    let [x, y, width, height] = config.reference.crop;
    let reference = image::imageops::crop_imm(&reference_export, x, y, width, height).to_image();
    images::validate_content(&reference, config.comparison.allow_uniform_reason.is_some())?;

    *status = "invalid_config";
    let compared = compare::compare(&config, &reference, &actual)?;
    let matched = compared.domains.iter().all(|d| d.matched);
    *status = "infrastructure_failure";
    report::artifacts(output, &config, &reference, &actual, &compared)?;
    let exclusions = config
        .exclusions
        .iter()
        .map(|e| {
            Ok(
                serde_json::json!({"name": e.name, "reason": e.reason, "kind": e.kind,
            "reference_and_actual_bounds_physical": config.profile.resolve(e.rect)?}),
            )
        })
        .collect::<crate::Result<Vec<_>>>()?;
    report::write_json(
        output.join("report.json"),
        &serde_json::json!({
            "schema": 1,
            "automated_result": if matched { "matched" } else { "mismatch" },
            "design_acceptance": "awaiting_human_review",
            "baseline_approval": config.reference.approval,
            "policy_review_required": matches!(config.reference.approval, Approval::Pending),
            "comparator": {"name": "dify", "version": "0.8.0", "aa_suppression": false,
                "normalized_yiq_tolerance": config.comparison.normalized_yiq_tolerance,
                "squared_yiq_threshold": compare::threshold(config.comparison.normalized_yiq_tolerance)},
            "config_sha256": images::digest(&config_bytes),
            "reference_sha256": config.reference.sha256,
            "actual_sha256": actual_hash,
            "profile": config.profile,
            "domains": compared.domains,
            "resolved_exclusions": exclusions,
            "processing": "reference crop only; no actual resize, crop, alignment, AA suppression or baseline update",
            "temporal_coverage": "none; a single frame does not verify animation or native window operations",
        }),
    )?;
    Ok(matched)
}
