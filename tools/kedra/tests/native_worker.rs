//! Exercise the real launcher, not a test harness pretending to be the launcher.
use std::{fs, process::Command};

#[test]
fn native_worker_reports_outputs_and_rejects_existing_or_invalid_reports()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    fs::write(directory.path().join("probe"), "fixture executable")?;
    let config = directory.path().join("config.json");
    let report = directory.path().join("report.json");
    fs::write(
        &config,
        serde_json::to_vec(&serde_json::json!({
            "productName": "Worker Probe", "version": "1.2.3",
            "identifier": "dev.kedra.worker-probe", "formats": ["app"],
            "targetTriple": "aarch64-apple-darwin",
            "binariesDir": directory.path(), "outDir": directory.path().join("packages"),
            "binaries": [{"path": "probe", "main": true}]
        }))?,
    )?;
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_kedra-build-tool"))
            .args(["bundle-native", "--config"])
            .arg(&config)
            .arg("--report")
            .arg(&report)
            .output()
    };
    let output = run()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let before = fs::read(&report)?;
    let outputs: serde_json::Value = serde_json::from_slice(&before)?;
    assert_eq!(outputs[0]["format"], "app");
    assert!(
        std::path::Path::new(
            outputs[0]["paths"][0]
                .as_str()
                .ok_or("Missing output path")?
        )
        .is_dir()
    );
    assert!(!run()?.status.success());
    assert_eq!(fs::read(&report)?, before);
    fs::write(&config, "invalid configuration")?;
    fs::remove_file(&report)?;
    assert!(!run()?.status.success());
    assert!(!report.exists());
    Ok(())
}
