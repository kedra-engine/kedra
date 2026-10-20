//! Executable-relative default locations for installed asset deliveries.
use std::{
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
};

/// Native bundle layout, explicit so all layouts can be tested on every host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundlePlatform {
    /// Windows portable and installed applications use adjacent assets.
    Windows,
    /// Linux AppDir/system installs use `usr/lib/<executable>`; portable uses adjacency.
    Linux,
    /// macOS app bundles keep assets under Contents/Resources.
    Macos,
}

impl BundlePlatform {
    /// Return this build's native layout; non-macOS/Windows hosts use adjacency.
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// Locate an application's default delivery without accessing the filesystem.
///
/// `executable` must be an absolute file location, normally from `current_exe`.
/// Relative or parentless paths return `InvalidInput` rather than depend on the
/// launch directory. Standard macOS bundles use Contents/Resources/assets;
/// Linux usr/bin executables use `usr/lib/<executable>/assets`, matching native
/// packaging tools. Development and portable executables use adjacent assets.
/// The caller owns CLI/environment overrides and must verify the returned
/// delivery before use. This function does not search, create or load resources.
pub fn bundled_asset_root(executable: &Path, platform: BundlePlatform) -> io::Result<PathBuf> {
    if !executable.is_absolute() || executable.file_name().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Expected an absolute executable file path",
        ));
    }
    let directory = executable.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "Executable directory missing")
    })?;
    if platform == BundlePlatform::Macos
        && directory.file_name() == Some(OsStr::new("MacOS"))
        && let Some(contents) = directory.parent()
        && contents.file_name() == Some(OsStr::new("Contents"))
        && contents.parent().and_then(Path::extension) == Some(OsStr::new("app"))
    {
        return Ok(contents.join("Resources/assets"));
    }
    if platform == BundlePlatform::Linux
        && directory.file_name() == Some(OsStr::new("bin"))
        && let Some(prefix) = directory.parent()
        && prefix.file_name() == Some(OsStr::new("usr"))
        && let Some(name) = executable.file_name()
    {
        return Ok(prefix.join("lib").join(name).join("assets"));
    }
    Ok(directory.join("assets"))
}
