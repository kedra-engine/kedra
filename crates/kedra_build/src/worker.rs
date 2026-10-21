//! Native helpers run in a separate launcher process, never mutating caller state.
use cargo_packager::{Config, PackageFormat};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Serialize, Deserialize)]
pub(crate) struct NativeOutput {
    pub format: PackageFormat,
    pub paths: Vec<PathBuf>,
}

pub(crate) fn package(
    config: &Config,
    directory: &Path,
    runtime: Option<&Path>,
) -> Result<Vec<NativeOutput>, String> {
    let config_path = config.out_dir.join("native-config.json");
    let report_path = config.out_dir.join("native-output.json");
    let bytes = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
    fs::File::create_new(&config_path)
        .and_then(|mut file| file.write_all(&bytes))
        .map_err(|e| e.to_string())?;
    let mut worker = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    if let Some(runtime) = runtime {
        worker.env("LDAI_RUNTIME_FILE", runtime);
    }
    let status = worker
        .args(["bundle-native", "--config"])
        .arg(&config_path)
        .arg("--report")
        .arg(&report_path)
        .current_dir(directory)
        // Upstream's unattended mode skips Finder AppleScript for DMG creation.
        // Set this on the child only; no unsafe process-global env mutation.
        .env("CI", "true")
        .status()
        .map_err(|e| format!("Cannot launch native packaging worker: {e}"))?;
    if !status.success() {
        return Err(format!("Native packaging worker failed: {status}"));
    }
    serde_json::from_slice(
        &fs::read(&report_path).map_err(|e| format!("Native output report: {e}"))?,
    )
    .map_err(|e| format!("Invalid native package result: {e}"))
}

pub(crate) fn execute(config: &Path, report: &Path) -> Result<(), String> {
    if fs::symlink_metadata(report).is_ok() {
        return Err("Native output report already exists".into());
    }
    let config: Config = serde_json::from_slice(&fs::read(config).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Invalid native packager configuration: {e}"))?;
    let outputs = cargo_packager::package(&config).map_err(|e| format!("Native packager: {e}"))?;
    let report_data: Vec<_> = outputs
        .into_iter()
        .map(|output| NativeOutput {
            format: output.format,
            paths: output.paths,
        })
        .collect();
    let bytes = serde_json::to_vec_pretty(&report_data).map_err(|e| e.to_string())?;
    fs::File::create_new(report)
        .and_then(|mut file| file.write_all(&bytes))
        .map_err(|e| e.to_string())?;
    Ok(())
}
