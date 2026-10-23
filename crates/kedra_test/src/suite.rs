//! Two complete process sequences, not a process per screenshot or test registry.
use crate::{Client, Result, process::NativeProcess};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Run a selected sequence in exactly two fresh processes. Project functions own
/// launch arguments, feature readiness, actual input and graceful authorization.
/// The capture callback returns portable evidence only. Failed cases are retained
/// and the sequence continues; lifecycle failure invalidates the complete session.
/// Writes incremental reports so native errors never erase completed evidence.
pub fn capture_twice(
    output: &Path,
    cases: &[String],
    mut launch: impl FnMut(usize, &Path) -> Result<(NativeProcess, Client, PathBuf)>,
    mut capture: impl FnMut(&Client, &Path, &str, usize) -> Result<Value>,
    mut shutdown: impl FnMut(&Client) -> Result<()>,
) -> Result<Value> {
    if cases.is_empty()
        || cases
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != cases.len()
    {
        return Err("Native sequence requires nonempty unique case identities".into());
    }
    let mut report = json!({"schema":1,"result":"failed","design_acceptance":"not_granted",
        "processes":[],"cases":{}});
    for case in cases {
        report["cases"][case] = json!({"status":"not_run","runs":[{},{}],"comparisons":{}});
    }
    for repeat in 1..=2 {
        let started = std::time::Instant::now();
        let private = output.join("private").join(format!("run-{repeat}"));
        let mut process_report = json!({"repeat":repeat,"status":"starting","cases":[]});
        let result = (|| -> Result<()> {
            let (mut native, client, artifacts) = launch(repeat, &private)?;
            for (index, case) in cases.iter().enumerate() {
                eprintln!(
                    "[process {repeat}/2 case {}/{}] {case}",
                    index + 1,
                    cases.len()
                );
                native.require_running()?;
                match capture(&client, &artifacts, case, repeat) {
                    Ok(evidence) => {
                        report["cases"][case]["runs"][repeat - 1] = evidence;
                        report["cases"][case]["status"] = json!("captured");
                        process_report["cases"]
                            .as_array_mut()
                            .ok_or("Invalid process report")?
                            .push(json!(case));
                    }
                    Err(error) => {
                        eprintln!("FAILED {case}: {error}");
                        let result = &mut report["cases"][case];
                        if result.get("capture_errors").is_none() {
                            result["capture_errors"] = json!([]);
                        }
                        result["capture_errors"].as_array_mut().ok_or("Invalid capture report")?.push(json!({
                            "process":repeat,"error":error.to_string(),"diagnostics":format!("private/run-{repeat}/native.log")}));
                        result["status"] = json!("capture_failed");
                    }
                }
                crate::report::write(&output.join("public"), &report)?;
            }
            native.finish(|| shutdown(&client))
        })();
        process_report["duration_seconds"] = json!(started.elapsed().as_secs_f64());
        match result {
            Ok(()) => process_report["status"] = json!("completed"),
            Err(error) => {
                eprintln!("FAILED native process {repeat}: {error}");
                process_report["status"] = json!("failed");
                process_report["error"] = json!(error.to_string());
            }
        }
        report["processes"]
            .as_array_mut()
            .ok_or("Invalid suite report")?
            .push(process_report);
        crate::report::write(&output.join("public"), &report)?;
    }
    Ok(report)
}

/// Check full/shape cross-session identity and exact restoration. Receipt frame
/// numbers intentionally need not match. A lifecycle/capture failure cannot pass.
pub fn verify_case(report: &Value, case: &str, layers: &[&str]) -> Result<()> {
    let processes = report["processes"]
        .as_array()
        .ok_or("Missing process evidence")?;
    if processes.len() != 2 || processes.iter().any(|p| p["status"] != "completed") {
        return Err("Exactly two successful process lifecycles are required".into());
    }
    let result = &report["cases"][case];
    if result.get("capture_errors").is_some() {
        return Err("Case has capture failures".into());
    }
    let runs = result["runs"]
        .as_array()
        .ok_or("Missing session evidence")?;
    if runs.len() != 2 {
        return Err("Expected two capture runs".into());
    }
    if layers.contains(&"restored") {
        if !layers.contains(&"full") {
            return Err("Restoration requires original full-frame evidence".into());
        }
        for run in runs {
            for key in ["sha256", "profile", "snapshot", "size"] {
                if run["full"][key] != run["restored"][key] {
                    return Err(format!("Restoration changed original full/{key}").into());
                }
            }
        }
    }
    for layer in layers {
        for run in runs {
            if run[layer]["sha256"].as_str().is_none_or(|s| s.len() != 64) {
                return Err("Incomplete native layer evidence".into());
            }
        }
        for key in ["sha256", "profile", "snapshot"] {
            if runs[0][layer][key] != runs[1][layer][key] {
                return Err(format!("Cross-process {layer}/{key} differs").into());
            }
        }
    }
    Ok(())
}
