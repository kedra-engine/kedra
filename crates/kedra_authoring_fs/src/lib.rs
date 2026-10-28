//! Guarded existing-source Save groups with durable interruption recovery.
//! Hash/inventory preconditions and a stable advisory lease guard cooperative writers.
//! Noncooperating hostile filesystem replacement races are not a sandbox guarantee.
use kedra_authoring::persistence::{CommitRequest, Receipt, SavedFile, content_hash};
use kedra_scene::document::PrefabDocument;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
mod group;
pub mod source;
mod storage;
pub use group::{commit_existing, read_guard, recover_existing};

/// Save failure classification: only a definite refusal permits releasing pinned bytes.
#[derive(Debug)]
pub struct SaveError {
    /// An atomic replacement may have happened. Retain/retry this exact request.
    pub uncertain: bool,
    /// Bounded filesystem/policy diagnostic; native paths are not UI labels.
    pub message: String,
}
impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for SaveError {}
fn refused(message: impl ToString) -> SaveError {
    SaveError {
        uncertain: false,
        message: message.to_string(),
    }
}
fn uncertain(message: impl ToString) -> SaveError {
    SaveError {
        uncertain: true,
        message: message.to_string(),
    }
}

fn portable(path: &str) -> Result<(), SaveError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains(['\\', ':', '#', '\0'])
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(refused(
            "Save requires an exact portable source-relative path",
        ));
    }
    Ok(())
}
fn directories(path: &Path) -> Result<(), SaveError> {
    for ancestor in path.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        let metadata = std::fs::symlink_metadata(ancestor).map_err(refused)?;
        if !metadata.is_dir() || metadata.is_symlink() {
            return Err(refused("Save ancestor is not a regular directory"));
        }
    }
    Ok(())
}
fn hash(path: &Path) -> Result<String, SaveError> {
    let metadata = std::fs::symlink_metadata(path).map_err(refused)?;
    if !metadata.is_file() || metadata.is_symlink() || metadata.len() > 8 * 1024 * 1024 {
        return Err(refused("Save precondition requires a bounded regular file"));
    }
    let file = File::open(path).map_err(refused)?;
    if !file.metadata().map_err(refused)?.is_file() {
        return Err(refused("Save precondition changed to a nonregular file"));
    }
    let mut data = Vec::new();
    file.take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut data)
        .map_err(refused)?;
    if data.len() > 8 * 1024 * 1024 {
        return Err(refused("Save precondition grew beyond its limit"));
    }
    // Asset checks include arbitrary binary bytes, not only UTF-8 documents.
    Ok(format!("{:x}", Sha256::digest(&data)))
}
fn validate(root: &Path, request: &CommitRequest) -> Result<(), SaveError> {
    let mut actual = BTreeSet::new();
    let mut members = BTreeSet::new();
    let mut sidecars = BTreeSet::new();
    for (count, entry) in walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_depth(65)
        .into_iter()
        .enumerate()
    {
        if count >= 8192 {
            return Err(refused("Save inventory exceeds its entry limit"));
        }
        let entry = entry.map_err(|error| {
            refused(error.io_error().map_or_else(
                || "Cannot inspect source inventory".into(),
                ToString::to_string,
            ))
        })?;
        if entry.depth() > 64
            || entry.file_type().is_symlink()
            || !(entry.file_type().is_file() || entry.file_type().is_dir())
        {
            return Err(refused("Save inventory contains linked/special sources"));
        }
        if entry.file_type().is_file() {
            let path = entry
                .path()
                .strip_prefix(root)
                .map_err(refused)?
                .to_str()
                .ok_or_else(|| refused("Save source paths must be Unicode"))?
                .replace(std::path::MAIN_SEPARATOR, "/");
            if path.ends_with(".scene.ron") || path.ends_with(".prefab.ron") {
                actual.insert(path.clone());
            }
            if entry
                .path()
                .extension()
                .is_none_or(|extension| extension != "meta")
            {
                members.insert(path);
            } else {
                sidecars.insert(path);
            }
        }
    }
    let expected: BTreeSet<_> = request.expected_sources.iter().cloned().collect();
    let expected_members: BTreeSet<_> = request
        .checks
        .iter()
        .filter(|check| check.expected_hash.is_some())
        .filter(|check| !check.path.ends_with(".meta"))
        .map(|check| check.path.clone())
        .collect();
    if request.checks.len() > 8192
        || expected.len() > 128
        || expected.len() != request.expected_sources.len()
        || actual != expected
        || members != expected_members
        || sidecars
            != request
                .checks
                .iter()
                .filter(|check| check.path.ends_with(".meta") && check.expected_hash.is_some())
                .map(|check| check.path.clone())
                .collect()
    {
        return Err(refused("Source inventory changed; Save refused"));
    }
    let mut checked = BTreeSet::new();
    let mut total = 0u64;
    for check in &request.checks {
        portable(&check.path)?;
        if !checked.insert(&check.path) {
            return Err(refused("Duplicate Save precondition"));
        }
        directories(
            root.join(&check.path)
                .parent()
                .ok_or_else(|| refused("Source parent missing"))?,
        )?;
        let actual = match std::fs::symlink_metadata(root.join(&check.path)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(refused(error)),
            Ok(metadata) => {
                total = total.saturating_add(metadata.len());
                if total > 64 * 1024 * 1024 {
                    return Err(refused("Save preconditions exceed the source byte budget"));
                }
                Some(hash(&root.join(&check.path))?)
            }
        };
        let already_saved = request.files.iter().any(|file| {
            file.path == check.path && actual == file.source.as_deref().map(content_hash)
        });
        if actual != check.expected_hash && !already_saved {
            return Err(refused(format!("Save conflict: {}", check.path)));
        }
    }
    if expected.iter().any(|path| !checked.contains(path))
        || request
            .files
            .iter()
            .any(|file| !checked.contains(&file.path))
    {
        return Err(refused("Incomplete Save source preconditions"));
    }
    for file in &request.files {
        let bytes = storage::read_bounded(&root.join(&file.path), 65_536).map_err(refused)?;
        let document = PrefabDocument::parse(std::str::from_utf8(&bytes).map_err(refused)?)
            .map_err(refused)?;
        if Some(document.id) != file.document {
            return Err(refused("Save cannot replace an existing document identity"));
        }
    }
    Ok(())
}
fn lease(root: &Path, shared: bool) -> Result<File, SaveError> {
    let mut name = root
        .file_name()
        .ok_or_else(|| refused("Source root has no name"))?
        .to_os_string();
    name.push(".kedra-save.lock");
    let path: PathBuf = root
        .parent()
        .ok_or_else(|| refused("Source root has no parent"))?
        .join(name);
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if !metadata.is_file() || metadata.is_symlink() => {
            return Err(refused("Save lease is not a regular file"));
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(refused(error)),
        _ => (),
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(refused)?;
    if shared {
        file.try_lock_shared().map_err(refused)?;
    } else {
        file.try_lock().map_err(refused)?;
    }
    Ok(file)
}
#[cfg(unix)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}
#[cfg(not(unix))]
fn sync_directory(_: &Path) -> std::io::Result<()> {
    Ok(())
}
