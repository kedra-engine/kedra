//! Positive native-install identity; portable and unknown layouts stay disabled.
use super::{Config, release::Kind};
use serde::Deserialize;
use std::{ffi::OsString, fs, io::Read, path::Path, process::Command};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    schema: u32,
    application: String,
    version: String,
    kind: Kind,
    target: String,
}

/// Checks a bounded explicit native-install marker against consumer identity.
///
/// Missing markers and AppImages without an outer path return `None`; malformed or
/// mismatched markers and ambiguous executable paths return errors. The host must
/// resolve resources through its installed-layout resolver and establish AppImage
/// mount ownership before calling. This performs no installation or writes.
pub fn detect_at(
    config: &Config,
    executable: &Path,
    resources: &Path,
    kind: Kind,
    appimage: Option<&Path>,
) -> Result<Option<Kind>, String> {
    if kind == Kind::AppImage && appimage.is_none() {
        return Ok(None);
    }
    let path = resources.join(format!("{}-installation.json", config.application));
    let mut file = match fs::File::open(&path) {
        Ok(file) => file.take(4097),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > 4096 {
        return Err("Native installation marker is too large".into());
    }
    let marker: Marker = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Invalid native installation marker: {e}"))?;
    if marker.schema != 1
        || marker.application != config.application
        || marker.version != config.current.to_string()
        || marker.kind != kind
        || marker.target != kind.target()
    {
        return Err("Native installation marker does not match this editor".into());
    }
    if !executable.is_absolute()
        || executable.file_name()
            != Some(std::ffi::OsStr::new(&if kind == Kind::Nsis {
                format!("{}.exe", config.executable)
            } else {
                config.executable.clone()
            }))
        || !fs::symlink_metadata(executable)
            .map_err(|e| e.to_string())?
            .is_file()
    {
        return Err("Native executable identity is ambiguous".into());
    }
    if let Some(image) = appimage
        && (!image.is_absolute()
            || !fs::symlink_metadata(image)
                .map_err(|e| e.to_string())?
                .is_file())
    {
        return Err("AppImage outer-file identity is ambiguous".into());
    }
    if kind == Kind::MacApp
        && (executable.parent().and_then(Path::file_name) != Some(std::ffi::OsStr::new("MacOS"))
            || executable
                .components()
                .any(|c| c.as_os_str() == "Volumes" || c.as_os_str() == "AppTranslocation"))
    {
        return Err("Move Kedra from its disk image into Applications before updating".into());
    }
    if kind == Kind::MacApp {
        let root = executable
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .ok_or("Installed bundle root is missing")?;
        if resources.canonicalize().map_err(|e| e.to_string())?
            != root
                .join("Contents/Resources")
                .canonicalize()
                .map_err(|e| e.to_string())?
        {
            return Err("Installation marker is outside the installed bundle".into());
        }
        super::macos::validate_installed(config, root)?;
    }
    Ok(Some(kind))
}

/// Builds a native file-manager reveal command without shell interpolation or
/// executing the package. The consumer must reverify the package before spawning,
/// handle spawn errors and persist it if the user needs it after application exit.
pub fn reveal_command(path: &Path, kind: Kind) -> Command {
    let mut command = Command::new(match kind {
        Kind::Nsis => "explorer.exe",
        Kind::MacApp => "/usr/bin/open",
        Kind::AppImage => "xdg-open",
    });
    match kind {
        Kind::Nsis => {
            let mut arg = OsString::from("/select,");
            arg.push(path);
            command.arg(arg);
        }
        Kind::MacApp => {
            command.arg("-R").arg(path);
        }
        Kind::AppImage => {
            command.arg(path.parent().unwrap_or(path));
        }
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    #[test]
    fn portable_and_source_are_disabled_but_native_marker_is_recognized() -> TestResult {
        let dir = tempfile::tempdir()?;
        let exe = dir.path().join("kedra.exe");
        fs::write(&exe, b"fixture")?;
        assert!(
            detect_at(
                &crate::config::fixture(),
                &exe,
                dir.path(),
                Kind::Nsis,
                None
            )?
            .is_none()
        );
        fs::write(
            dir.path().join("kedra-installation.json"),
            serde_json::to_vec(&serde_json::json!({
                "schema":1,"application":"kedra","version":"0.1.0","kind":"Nsis","target":"x86_64-pc-windows-msvc"
            }))?,
        )?;
        assert_eq!(
            detect_at(
                &crate::config::fixture(),
                &exe,
                dir.path(),
                Kind::Nsis,
                None
            )?
            .ok_or("Not recognized")?,
            Kind::Nsis
        );
        assert!(
            detect_at(
                &crate::config::fixture(),
                &exe,
                dir.path(),
                Kind::AppImage,
                None
            )?
            .is_none()
        );
        Ok(())
    }
    #[test]
    fn malformed_install_marker_is_not_treated_as_source_or_portable() -> TestResult {
        let dir = tempfile::tempdir()?;
        let exe = dir.path().join("kedra.exe");
        fs::write(&exe, b"fixture")?;
        fs::write(dir.path().join("kedra-installation.json"), b"{}")?;
        assert!(
            detect_at(
                &crate::config::fixture(),
                &exe,
                dir.path(),
                Kind::Nsis,
                None
            )
            .is_err()
        );
        Ok(())
    }
    #[test]
    fn manual_reveal_never_executes_package_or_interpolates_shell_text() {
        let path = Path::new("/absolute/Юникод ' $HOME/package.dmg");
        let command = reveal_command(path, Kind::MacApp);
        assert_eq!(command.get_program(), "/usr/bin/open");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![std::ffi::OsStr::new("-R"), path.as_os_str()]
        );
        let command = reveal_command(path, Kind::AppImage);
        assert_eq!(command.get_program(), "xdg-open");
        assert_eq!(
            command.get_args().next(),
            path.parent().map(Path::as_os_str)
        );
    }
}
