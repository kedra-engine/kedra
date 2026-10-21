//! Editor configuration, installed resource resolution and user-facing instructions.
use kedra_update::{Config, Kind, MacInstallation, Version};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn reserve_update_access(
    config: &Config,
    root: PathBuf,
) -> Result<Option<MacInstallation>, String> {
    match MacInstallation::open(config, root) {
        Ok(installation) => Ok(Some(installation)),
        Err(kedra_update::OpenError::Busy) => Err(kedra_update::OpenError::Busy.to_string()),
        Err(kedra_update::OpenError::Unavailable(error)) => {
            bevy::log::warn!("Self-update disabled: {error}");
            Ok(None)
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Installation {
    pub kind: Kind,
    pub macos: Option<MacInstallation>,
}

pub(super) fn config() -> Result<Config, String> {
    Config::new(
        "kedra-engine/kedra",
        "kedra",
        "dev.kedra.editor",
        "kedra",
        "Kedra.app",
        Version::parse(env!("CARGO_PKG_VERSION")).map_err(|e| e.to_string())?,
    )
}

pub(super) fn detect(config: &Config) -> Result<Option<Installation>, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let platform = kedra_assets::location::BundlePlatform::current();
    let resources = kedra_assets::location::bundled_asset_root(&executable, platform)
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Bundled resources have no parent")?
        .to_owned();
    let kind = if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Kind::Nsis
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Kind::MacApp
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Kind::AppImage
    } else {
        return Ok(None);
    };
    if kind == Kind::AppImage {
        let Some(appdir) = std::env::var_os("APPDIR") else {
            return Ok(None);
        };
        if !executable
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(fs::canonicalize(appdir).map_err(|e| e.to_string())?)
        {
            return Err("AppImage mount does not own the running executable".into());
        }
    }
    Ok(kedra_update::detect_installation(
        config,
        &executable,
        &resources,
        kind,
        std::env::var_os("APPIMAGE").as_deref().map(Path::new),
    )?
    .map(|kind| Installation { kind, macos: None }))
}

pub(super) fn instructions(kind: Kind) -> &'static str {
    match kind {
        Kind::Nsis => {
            "Manual installation required. Close all Kedra windows, then run the downloaded installer."
        }
        Kind::AppImage => {
            "Manual installation required. Close all Kedra windows, replace the old AppImage and allow execution of the new file."
        }
        Kind::MacApp => "Kedra will restart after preparation. Unsaved changes prevent restart.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_update_access_does_not_block_editor_startup()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        assert!(reserve_update_access(&config()?, dir.path().join("missing.app"))?.is_none());
        Ok(())
    }
}
