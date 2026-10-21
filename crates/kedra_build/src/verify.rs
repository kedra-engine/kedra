//! Consumer-neutral executable, bundle and resource checks; never runs the app.
use crate::{
    application::BuildResult,
    config::{Application, NativeTarget},
};
use object::Object;
use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

pub(crate) fn application(
    app: &Application,
    output: &Path,
    result: &BuildResult,
) -> Result<(), String> {
    if result.schema_version != 1
        || result.application != app.package.name.as_str()
        || result.version != app.package.version.to_string()
        || result.target != app.target.triple()
    {
        return Err("Build result identity differs from the selected application".into());
    }
    let root = member(output, &result.application_root)?;
    let binary = member(output, &result.executable)?;
    let assets = member(output, &result.resources)?;
    if matches!(app.target, NativeTarget::MacArm | NativeTarget::MacIntel) {
        macos(app, &root)?;
    }
    crate::package_verify::installed(app, &output.join("application"), &root, &binary, &assets)
}

pub(crate) fn member(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Application location must be output-relative without traversal".into());
    }
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| format!("Application member {}: {e}", relative.display()))?;
    if !path.starts_with(root.canonicalize().map_err(|e| e.to_string())?) {
        return Err("Application member escaped the reserved output".into());
    }
    Ok(path)
}

pub(crate) fn executable(path: &Path, target: NativeTarget) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("Application executable: {e}"))?;
    let object = object::File::parse(bytes.as_slice())
        .map_err(|e| format!("Application executable format: {e}"))?;
    let (format, architecture) = match target {
        NativeTarget::Windows => (object::BinaryFormat::Pe, object::Architecture::X86_64),
        NativeTarget::Linux => (object::BinaryFormat::Elf, object::Architecture::X86_64),
        NativeTarget::MacArm => (object::BinaryFormat::MachO, object::Architecture::Aarch64),
        NativeTarget::MacIntel => (object::BinaryFormat::MachO, object::Architecture::X86_64),
    };
    let runnable = object.kind() == object::ObjectKind::Executable
        || (target == NativeTarget::Linux
            && object.kind() == object::ObjectKind::Dynamic
            && object.entry() != 0);
    if object.format() != format || object.architecture() != architecture || !runnable {
        return Err(format!(
            "Application executable does not match {}",
            target.triple()
        ));
    }
    if target == NativeTarget::Linux
        && object
            .imports()
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        let result = Command::new("ldd")
            .arg(path)
            .output()
            .map_err(|e| format!("Cannot inspect native libraries: {e}"))?;
        if !result.status.success() || String::from_utf8_lossy(&result.stdout).contains("not found")
        {
            return Err(format!(
                "Application has unresolved native libraries: {}{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            ));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(path)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o111
            == 0
        {
            return Err("Application binary lacks executable permission".into());
        }
    }
    Ok(())
}

pub(crate) fn resources(app: &Application, assets: &Path, notices: &Path) -> Result<(), String> {
    kedra_assets::delivery::read_directory(assets, &app.distribution.profile)?;
    for source in &app.distribution.notices {
        let installed = notices.join(source.file_name().ok_or("Notice filename missing")?);
        if fs::read(source).map_err(|e| e.to_string())?
            != fs::read(installed).map_err(|e| format!("Installed notice missing: {e}"))?
        {
            return Err("Installed license notice differs from its source".into());
        }
    }
    if app.distribution.license_config.is_some() {
        let html = notices.join("dependency-notices.html");
        if fs::metadata(&html)
            .map_err(|e| format!("Dependency notices missing: {e}"))?
            .len()
            == 0
        {
            return Err("Configured dependency license materials are incomplete".into());
        }
    }
    Ok(())
}

pub(crate) fn macos(app: &Application, root: &Path) -> Result<(), String> {
    let plist = root.join("Contents/Info.plist");
    for (key, expected) in [
        (
            "CFBundleIdentifier",
            app.native
                .identifier
                .as_deref()
                .ok_or("Bundle identifier missing")?
                .to_owned(),
        ),
        (
            "CFBundleShortVersionString",
            app.package.version.to_string(),
        ),
        ("CFBundleExecutable", app.binary.clone()),
    ] {
        let result = Command::new("plutil")
            .args(["-extract", key, "raw", "-o", "-"])
            .arg(&plist)
            .output()
            .map_err(|e| format!("Cannot inspect bundle identity: {e}"))?;
        if !result.status.success() || String::from_utf8_lossy(&result.stdout).trim() != expected {
            return Err(format!(
                "Installed bundle {key} differs from application metadata"
            ));
        }
    }
    let result = Command::new("codesign")
        .args(["--verify", "--deep", "--strict", "--verbose=2"])
        .arg(root)
        .output()
        .map_err(|e| format!("Cannot verify bundle signature: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "Bundle signature verification failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(())
}
