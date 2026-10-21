//! Ordered fresh staging; no output deletion or implicit project initialization.
use crate::{compiler, config::Application, icons, inventory, package};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn execute(
    app: &Application,
    release: bool,
    output: Option<&Path>,
    package: bool,
) -> Result<crate::application::BuiltApplication, String> {
    crate::tools::preflight(app, package)?;
    let output = fresh_output(app, output)?;
    eprintln!("Kedra / {} / {}", app.package.name, app.target.triple());
    eprintln!("Output: {}", output.display());
    let stage = output.join("application");
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    eprintln!("Preparing application icons and verified resource delivery");
    let pngs = icons::generate(&app.distribution.icon, &stage.join("icons"))?;
    let assets = stage.join("assets");
    kedra_import::build(
        &app.distribution.assets,
        &assets,
        &app.distribution.profile,
        &app.distribution.namespace,
        |line| eprintln!("{line}"),
    )?;
    kedra_assets::delivery::read_directory(&assets, &app.distribution.profile)?;
    let notices = stage.join("licenses");
    fs::create_dir(&notices).map_err(|e| e.to_string())?;
    for source in &app.distribution.notices {
        let name = source.file_name().ok_or("Notice filename missing")?;
        let mut destination = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(notices.join(name))
            .map_err(|e| format!("Cannot stage unique license notice: {e}"))?;
        let mut input = fs::File::open(source).map_err(|e| e.to_string())?;
        std::io::copy(&mut input, &mut destination).map_err(|e| e.to_string())?;
    }
    if let Some(config) = &app.distribution.license_config {
        eprintln!("Collecting dependency notices and covered source archives with cargo-about");
        crate::licenses::generate(app, config, &notices)?;
    }
    eprintln!("Compiling application with Cargo");
    let executable = compiler::build(app, release, &stage.join("icons/application.ico"))?;
    fs::copy(
        &executable,
        stage.join(
            executable
                .file_name()
                .ok_or("Executable filename missing")?,
        ),
    )
    .map_err(|e| e.to_string())?;
    eprintln!("Assembling and verifying runnable application");
    let result = crate::application::assemble(app, &stage, &output, pngs.clone())?;
    crate::verify::application(app, &output, &result)?;
    if package {
        eprintln!("Generating native and portable packages");
        let artifacts = package::create(app, &stage, &output, pngs)?;
        record_packages(app, &stage, &output, artifacts)?;
    }
    crate::application::publish(&output, &result)?;
    eprintln!(
        "SUCCESS: {} {} ({})",
        app.package.name,
        app.package.version,
        app.target.triple()
    );
    Ok(crate::application::BuiltApplication { output, result })
}

pub(crate) fn record_packages(
    app: &Application,
    stage: &Path,
    output: &Path,
    artifacts: Vec<(PathBuf, &'static str)>,
) -> Result<(), String> {
    crate::package_verify::verify(app, stage, &artifacts)?;
    eprintln!("Hashing and recording exact package inventory");
    inventory::write(app, output, artifacts)
}

fn fresh_output(app: &Application, requested: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = requested {
        let path = std::path::absolute(path).map_err(|e| e.to_string())?;
        if fs::symlink_metadata(&path).is_ok() {
            return Err(
                "Distribution output must be a new directory; existing files are never overwritten"
                    .into(),
            );
        }
        reject_asset_output(&path, &app.distribution.assets)?;
        fs::create_dir_all(path.parent().ok_or("Output parent missing")?)
            .map_err(|e| e.to_string())?;
        fs::create_dir(&path)
            .map_err(|e| format!("Cannot reserve new distribution output: {e}"))?;
        Ok(path)
    } else {
        let parent = app.target_directory.join("kedra-build");
        reject_asset_output(&parent, &app.distribution.assets)?;
        fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
        Ok(tempfile::Builder::new()
            .prefix("build-")
            .tempdir_in(parent)
            .map_err(|e| e.to_string())?
            .keep())
    }
}

fn reject_asset_output(path: &Path, assets: &Path) -> Result<(), String> {
    let ancestor = path
        .ancestors()
        .find(|p| p.exists())
        .ok_or("Output parent is unavailable")?;
    let ancestor = ancestor.canonicalize().map_err(|e| e.to_string())?;
    if ancestor.starts_with(assets) {
        return Err("Distribution output must not be inside source assets".into());
    }
    Ok(())
}
