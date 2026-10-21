//! A verified application handoff, separate from source metadata and installers.
use crate::config::{Application, NativeTarget};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BuildResult {
    pub schema_version: u32,
    pub application: String,
    pub version: String,
    pub target: String,
    pub application_root: PathBuf,
    pub executable: PathBuf,
    pub resources: PathBuf,
}

pub(crate) struct BuiltApplication {
    pub output: PathBuf,
    pub result: BuildResult,
}

pub(crate) fn layout(app: &Application) -> BuildResult {
    let macos = matches!(app.target, NativeTarget::MacArm | NativeTarget::MacIntel);
    let root = if macos {
        PathBuf::from("native-application").join(format!("{}.app", app.native.product_name))
    } else {
        PathBuf::from("application")
    };
    let executable = if macos {
        root.join("Contents/MacOS").join(&app.binary)
    } else {
        root.join(format!(
            "{}{}",
            app.binary,
            if app.target == NativeTarget::Windows {
                ".exe"
            } else {
                ""
            }
        ))
    };
    let resources = if macos {
        root.join("Contents/Resources/assets")
    } else {
        root.join("assets")
    };
    BuildResult {
        schema_version: 1,
        application: app.package.name.to_string(),
        version: app.package.version.to_string(),
        target: app.target.triple().into(),
        application_root: root,
        executable,
        resources,
    }
}

pub(crate) fn assemble(
    app: &Application,
    stage: &Path,
    output: &Path,
    icons: Vec<PathBuf>,
) -> Result<BuildResult, String> {
    if matches!(app.target, NativeTarget::MacArm | NativeTarget::MacIntel) {
        let config = crate::package::configuration(
            app,
            stage,
            &output.join("native-application"),
            icons,
            cargo_packager::PackageFormat::App,
        )?;
        fs::create_dir(&config.out_dir).map_err(|e| e.to_string())?;
        let generated = crate::worker::package(
            &config,
            app.package
                .manifest_path
                .parent()
                .ok_or("Application directory missing")?
                .as_std_path(),
            None,
        )?;
        let expected = output.join(layout(app).application_root);
        let paths: Vec<_> = generated
            .into_iter()
            .filter(|item| item.format == cargo_packager::PackageFormat::App)
            .flat_map(|item| item.paths)
            .collect();
        if paths.len() != 1
            || paths[0].canonicalize().map_err(|e| e.to_string())?
                != expected.canonicalize().map_err(|e| e.to_string())?
        {
            return Err("Native application output is missing or ambiguous".into());
        }
    }
    Ok(layout(app))
}

pub(crate) fn publish(output: &Path, result: &BuildResult) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(result).map_err(|e| e.to_string())?;
    fs::File::create_new(output.join("build-result.json"))
        .and_then(|mut file| file.write_all(&bytes))
        .map_err(|e| e.to_string())
}
