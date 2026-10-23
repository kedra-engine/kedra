//! Exercise clap integration, actual exit status, report publication and escaping.
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, process::Command};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
}

#[test]
fn visual_cli_never_panics_and_rejects_misplaced_or_missing_arguments() -> Result<()> {
    let help = command().arg("--help").output()?;
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("visual"));
    for args in [
        vec!["visual"],
        vec!["fmt", "--config", "unused"],
        vec!["visual", "--actual", "a"],
    ] {
        let output = command().args(args).output()?;
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("error:"), "{error}");
        assert!(!error.contains("panicked"), "{error}");
    }
    Ok(())
}

#[test]
fn complete_cli_reports_mutation_and_preserves_independent_inputs() -> Result<()> {
    let root = tempfile::tempdir()?;
    let mut reference = RgbaImage::from_pixel(640, 360, Rgba([36, 36, 36, 255]));
    reference.put_pixel(5, 5, Rgba([255, 255, 255, 255]));
    reference.save(root.path().join("reference.png"))?;
    let mut actual = reference.clone();
    actual.put_pixel(5, 5, Rgba([0, 0, 0, 255]));
    actual.save(root.path().join("actual.png"))?;
    let reference_bytes = fs::read(root.path().join("reference.png"))?;
    let actual_bytes = fs::read(root.path().join("actual.png"))?;
    // Synthetic comparator contract, not product screenshot coverage.
    let mut config = json!({"schema":1,"scenario":"mutation","fixture":"comparator-contract",
        "profile":{"id":"synthetic","platform":"test","renderer":"synthetic","environment":"unit-test",
            "logical_size":[640,360],"dpr":1,"color_space":"srgb_assumed","motion":"reduced"},
        "reference":{"image":"reference.png","sha256":"0".repeat(64),"source":"Independent synthetic mutation input",
            "kind":"design_reference","approval":"pending","export_size":[640,360],"crop":[0,0,640,360],
            "crop_reason":"Complete synthetic fixture, not a UI baseline"},
        "animation":{"mode":"static"},"comparison":{"normalized_yiq_tolerance":0.05,
            "budget":{"max_changed_pixels":0,"max_changed_ppm":0},"max_excluded_ppm":0},
        "exclusions":[],"critical_regions":[{"name":"mutation-point","rect":[0,0,32,32],
            "budget":{"max_changed_pixels":0,"max_changed_ppm":0}}]});
    // This is an exact mutation fixture, independent of the design audit's budget.
    config["comparison"]["budget"] = json!({"max_changed_pixels":0,"max_changed_ppm":0});
    config["scenario"] = json!("unrelated-fixture <script>alert(1)</script>");
    config["reference"]["image"] = json!("reference.png");
    config["reference"]["export_size"] = json!([640, 360]);
    config["reference"]["crop"] = json!([0, 0, 640, 360]);
    config["reference"]["sha256"] = json!(format!("{:x}", Sha256::digest(&reference_bytes)));
    let evidence = json!({"schema":1,"scenario":config["scenario"],"fixture":config["fixture"],
        "profile":config["profile"],"image":"actual.png",
        "sha256":format!("{:x}",Sha256::digest(&actual_bytes)),
        "producer":"fixture","readiness":"explicit test image, not a native capture","receipt_frame":null});
    let config_path = root.path().join("config.json");
    let evidence_path = root.path().join("evidence.json");
    fs::write(&config_path, serde_json::to_vec(&config)?)?;
    fs::write(&evidence_path, serde_json::to_vec(&evidence)?)?;
    let output = root.path().join("review");
    let result = command()
        .arg("visual")
        .arg("--config")
        .arg(&config_path)
        .arg("--actual")
        .arg(&evidence_path)
        .arg("--output")
        .arg(&output)
        .output()?;
    assert!(!result.status.success());
    let report: Value = serde_json::from_slice(&fs::read(output.join("report.json"))?)?;
    assert_eq!(report["automated_result"], "mismatch");
    assert_eq!(report["domains"][0]["perceptual_changed_pixels"], 1);
    let html = fs::read_to_string(output.join("index.html"))?;
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
    assert_eq!(
        fs::read(root.path().join("reference.png"))?,
        reference_bytes
    );
    assert_eq!(fs::read(output.join("actual-source.png"))?, actual_bytes);
    Ok(())
}
