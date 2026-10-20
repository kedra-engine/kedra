//! Project sources, independent of Bevy and the desktop editor.
//!
//! Opening is read-only. Creating reserves a new directory and publishes the
//! manifest last; it never overwrites an existing directory or project.

pub mod recent;
mod storage;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

/// Current source format. Editor release numbers do not determine compatibility.
pub const FORMAT_VERSION: u32 = 1;
/// Filename identifying a project root.
pub const MANIFEST_FILE: &str = "kedra.toml";
/// Editor release which creates these sources.
pub const EDITOR_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Durable project identity. Paths are relative to the root, not the process cwd.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectManifest {
    /// Persistent identity, unchanged when a project moves.
    pub id: Uuid,
    /// Version of the on-disk source schema.
    pub format: u32,
    /// Human-readable project name, not an executable or Cargo package name.
    pub name: String,
    /// Release that last wrote the manifest; opening does not change it.
    pub editor_version: String,
    /// Relative source asset directory.
    pub assets: PathBuf,
}

/// Validated project, with canonical root and asset directory owned by the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    /// Canonical project root.
    pub root: PathBuf,
    /// Canonical source asset directory.
    pub assets: PathBuf,
    /// Validated manifest.
    pub manifest: ProjectManifest,
}

/// Actionable failures; rejected opens never modify project sources.
#[derive(Debug, Error)]
pub enum ProjectError {
    /// A user-facing validation failure.
    #[error("{0}")]
    Invalid(String),
    /// No migration is available for this source version.
    #[error(
        "Project format {found} is not supported by this editor (format {supported}). No migration is available; the project was not changed."
    )]
    UnsupportedFormat {
        /// Format read from disk.
        found: u32,
        /// Format understood by this release.
        supported: u32,
    },
    /// Filesystem operation with context and original diagnostic.
    #[error("{action}: {source}")]
    Io {
        /// Operation that failed.
        action: String,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },
}

pub(crate) fn io(action: impl Into<String>, source: std::io::Error) -> ProjectError {
    ProjectError::Io {
        action: action.into(),
        source,
    }
}

/// Validate a project name for a portable directory component.
///
/// Unicode is retained unchanged. Separators, control characters and Windows
/// device names are rejected on every platform to keep projects portable.
pub fn validate_name(name: &str) -> Result<(), ProjectError> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|n| n.len() == 1 && matches!(n.as_bytes()[0], b'1'..=b'9'))
        });
    if name.is_empty()
        || name.trim() != name
        || name.ends_with('.')
        || name == "."
        || name == ".."
        || reserved
        || name.len() > 160
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return Err(ProjectError::Invalid("Use a project name of at most 160 UTF-8 bytes, without reserved names, surrounding spaces or filename separators.".into()));
    }
    Ok(())
}

/// Create `parent/name`, without overwriting any existing entry.
///
/// The parent must already exist. The manifest is published only after the
/// initial directories exist. On failure only this operation's empty files and
/// directories are cleaned up; unrelated files are never removed.
pub fn create(parent: &Path, name: &str) -> Result<Project, ProjectError> {
    validate_name(name)?;
    storage::create(parent, name)
}

/// Resolve a proposed creation target without writing any files.
/// This is a UI preflight, not a reservation: `create` checks again atomically.
pub fn validate_creation(parent: &Path, name: &str) -> Result<PathBuf, ProjectError> {
    validate_name(name)?;
    let parent = parent
        .canonicalize()
        .map_err(|e| io("Choose an existing parent directory", e))?;
    if !parent.is_dir() {
        return Err(ProjectError::Invalid(
            "Project location must be a directory.".into(),
        ));
    }
    let root = parent.join(name);
    match std::fs::symlink_metadata(&root) {
        Ok(_)=>Err(ProjectError::Invalid("The target folder already exists. Choose another project name or location; existing content will not be changed.".into())),
        Err(error) if error.kind()==std::io::ErrorKind::NotFound=>Ok(root),
        Err(error)=>Err(io("Cannot check project destination",error)),
    }
}

/// Open and validate a project root without executing code or writing files.
///
/// Unknown formats, oversized manifests and asset roots escaping the project
/// are rejected. Different editor releases may open the same source format.
/// Static nonregular manifest files fail before opening. Regular-file symlinks
/// remain supported; concurrent filesystem replacement is not sandboxed.
pub fn open(root: &Path) -> Result<Project, ProjectError> {
    storage::open(root)
}

#[cfg(test)]
mod tests;
