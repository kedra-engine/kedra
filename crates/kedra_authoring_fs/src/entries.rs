//! Explicit source-entry operations share the source writer lease and admission reader.
use crate::{directories, group::Session, lease, source};
use kedra_authoring::persistence::SourceSnapshot;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// One immediate filesystem operation, separate from document Save and Undo.
/// Rename retains all bytes and sidecars. Delete detaches the complete entry
/// into a retained sibling recovery directory outside the asset inventory.
#[derive(Clone, Debug)]
pub enum Change {
    /// Create exactly this new project-relative folder; parents must exist.
    Create(PathBuf),
    /// Rename or move an existing file or directory to a vacant project-relative path.
    Rename {
        /// Existing project-relative file or directory.
        from: PathBuf,
        /// Vacant destination; its parent must already exist.
        to: PathBuf,
    },
    /// Remove a file or directory and its metadata from the source inventory.
    Delete(PathBuf),
}

/// Validate a single portable entry name, preserving Unicode spelling.
pub fn name(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value != value.trim()
        || value.ends_with('.')
        || matches!(value, "." | "..")
        || value.ends_with(".meta")
        || value
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
        || value.len() > 255
    {
        return Err("Enter a name without reserved characters or surrounding spaces.".into());
    }
    let stem = value
        .split('.')
        .next()
        .unwrap_or(value)
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
        })
    {
        return Err("This folder name is reserved by the filesystem.".into());
    }
    Ok(())
}

fn relative(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("The asset root cannot be changed.".into());
    }
    for part in path.components() {
        let std::path::Component::Normal(part) = part else {
            return Err("Folder path must be project-relative.".into());
        };
        name(part.to_str().ok_or("Folder paths must be Unicode")?)?;
    }
    Ok(())
}
fn vacant(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err("A file or folder with that name already exists.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Apply an entry change under the exclusive source lease. `validate` receives
/// the actual current inventory and proposed inventory before any mutation; its
/// error preserves the entire entry. Hosts use it for source/dependency/history
/// admission. Returned state is published only after the filesystem operation succeeds.
/// Existing names and the root cannot be replaced. Pending Save recovery refuses.
/// This is advisory coordination, not a sandbox against concurrent hostile writers.
/// Delete keeps a recovery copy beside the source root; callers own recovery/cleanup.
/// Interrupted file/sidecar journals block admission and require explicit inspection;
/// they cannot be replayed by the document Save recovery API.
/// This function blocks and belongs on a worker thread in interactive hosts.
pub fn apply<T>(
    root: &Path,
    namespace: &str,
    change: &Change,
    validate: impl FnOnce(&SourceSnapshot, SourceSnapshot) -> Result<T, String>,
) -> Result<T, String> {
    let session = Session::new(root).map_err(|e| e.to_string())?;
    let root = &session.root;
    let _lease = lease(root, false).map_err(|e| e.to_string())?;
    if session.pending().map_err(|e| e.to_string())?.is_some()
        || session.journal_present().map_err(|e| e.to_string())?
    {
        return Err("Recover pending source Save before changing folders.".into());
    }
    let (from, to) = match change {
        Change::Create(to) => (None, Some(to)),
        Change::Rename { from, to } => (Some(from), Some(to)),
        Change::Delete(from) => (Some(from), None),
    };
    for path in [from, to].into_iter().flatten() {
        relative(path)?;
    }
    let mut metadata = false;
    if let Some(from) = from {
        directories(&root.join(from.parent().ok_or("Source parent missing")?))
            .map_err(|e| e.to_string())?;
        let info = fs::symlink_metadata(root.join(from)).map_err(|e| e.to_string())?;
        if info.is_symlink() || !(info.is_file() || info.is_dir()) {
            return Err("Only regular files and folders can be changed.".into());
        }
        if let Some(to) = to {
            if to.starts_with(from) {
                return Err("An entry cannot be moved into itself or a descendant.".into());
            }
            if info.is_file() && suffix(from) != suffix(to) {
                return Err("Rename must preserve the asset file type.".into());
            }
        }
        metadata = fs::symlink_metadata(sidecar(&root.join(from)))
            .map(|meta| meta.is_file() && !meta.is_symlink())
            .unwrap_or(false);
        if info.is_dir() && metadata {
            return Err("Folder sidecars are not supported.".into());
        }
    }
    if let Some(to) = to {
        directories(&root.join(to.parent().ok_or("Folder parent missing")?))
            .map_err(|e| e.to_string())?;
        vacant(&root.join(to))?;
        let mut sidecar = root.join(to).into_os_string();
        sidecar.push(".meta");
        vacant(Path::new(&sidecar))?;
    }
    let before = source::read_unlocked(root, namespace)?;
    let mut after = before.clone();
    if let Some(from) = from {
        let remap = |path: &mut String| -> bool {
            let Ok(suffix) = Path::new(path).strip_prefix(from) else {
                return true;
            };
            if let Some(to) = to {
                *path = if suffix.as_os_str().is_empty() {
                    to.clone()
                } else {
                    to.join(suffix)
                }
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
                true
            } else {
                false
            }
        };
        after.documents.retain_mut(|file| remap(&mut file.path));
        after.assets.retain_mut(|file| remap(&mut file.path));
        after.environments = std::mem::take(&mut after.environments)
            .into_iter()
            .filter_map(|(mut path, source)| {
                (remap(&mut path) && path.ends_with(".environment.ron")).then_some((path, source))
            })
            .collect();
        if let Some(index) = &after.index
            && Path::new(&index.path).starts_with(from)
        {
            return Err("The source catalog cannot be moved or deleted here.".into());
        }
    }
    let accepted = validate(&before, after)?;
    match change {
        Change::Create(to) => fs::create_dir(root.join(to)).map_err(|e| e.to_string())?,
        Change::Rename { from, to } => {
            vacant(&root.join(to))?;
            move_entry(&session, from, &root.join(to), metadata)?;
        }
        Change::Delete(from) => {
            let recovery = tempfile::Builder::new()
                .prefix(".kedra-deleted-")
                .tempdir_in(root.parent().ok_or("Asset root parent missing")?)
                .map_err(|e| e.to_string())?;
            let destination = recovery
                .path()
                .join(from.file_name().ok_or("Entry name missing")?);
            // Retain recovery bytes even if the transaction reports an uncertain failure.
            let _ = recovery.keep();
            move_entry(&session, from, &destination, metadata)?;
        }
    }
    Ok(accepted)
}
fn sidecar(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".meta");
    name.into()
}
fn suffix(path: &Path) -> &str {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if name.ends_with(".scene.ron") {
        "scene.ron"
    } else if name.ends_with(".prefab.ron") {
        "prefab.ron"
    } else {
        path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
    }
}
fn move_entry(session: &Session, from: &Path, to: &Path, metadata: bool) -> Result<(), String> {
    use fs_transaction::{ChangeSet, StdFs, exec::block_on};
    // One native directory rename already carries its whole subtree atomically.
    if !metadata {
        return fs::rename(session.root.join(from), to).map_err(|e| e.to_string());
    }
    // A file and its sidecar are a coupled operation: reuse the existing maintained
    // journal/rollback engine. An interrupted journal blocks all ordinary admission.
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    match builder.create(&session.staging) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.to_string()),
    }
    directories(&session.staging).map_err(|e| e.to_string())?;
    let parent = session.root.parent().ok_or("Asset parent missing")?;
    let source = session.root.join(from);
    let mut changes = ChangeSet::new();
    for (from, to) in [
        (source.clone(), to.to_owned()),
        (sidecar(&source), sidecar(to)),
    ] {
        vacant(&to)?;
        let from = from.strip_prefix(parent).map_err(|e| e.to_string())?;
        let to = to.strip_prefix(parent).map_err(|e| e.to_string())?;
        changes.expect_absent(to);
        changes.rename(from, to);
    }
    block_on(session.journal.apply(&changes, &StdFs, parent)).map_err(|e| e.to_string())
}
