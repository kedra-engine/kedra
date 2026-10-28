//! Bounded read-only source snapshots, sharing admission/writer leases with Save.
use kedra_authoring::persistence::{
    CommitRequest, SourceAsset, SourceFile, SourceSnapshot, content_hash,
};
use kedra_scene::document::PrefabDocument;
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
use uuid::Uuid;
#[derive(Deserialize)]
struct Identity {
    id: Uuid,
}
#[cfg(test)]
mod tests;
fn label(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).map_or_else(
        |_| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        },
        |relative| Path::new("assets").join(relative).display().to_string(),
    )
}
fn bytes(root: &Path, path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let name = label(root, path);
    let meta = std::fs::symlink_metadata(path).map_err(|error| format!("{name}: {error}"))?;
    if !meta.is_file() || meta.is_symlink() || meta.len() > limit as u64 {
        return Err(format!(
            "Source must be a regular file of at most {limit} bytes: {name}"
        ));
    }
    let mut result = Vec::new();
    File::open(path)
        .and_then(|file| {
            let meta = file.metadata()?;
            if !meta.is_file() || meta.len() > limit as u64 {
                return Err(std::io::Error::other(
                    "Source changed to a nonregular or oversized file",
                ));
            }
            file.take(limit as u64 + 1).read_to_end(&mut result)
        })
        .map_err(|error| error.to_string())?;
    if result.len() > limit {
        return Err("Source grew beyond its admission limit".into());
    }
    Ok(result)
}
/// Read a complete existing source scope under a shared admission lease. Limits:
/// 8192 entries, depth 64, 128 documents, 64 MiB total; no links/special files.
/// Namespace is host-selected, not inferred. Never creates metadata or runs code.
/// Concurrent hostile filesystem replacement is not a sandbox guarantee.
pub fn read(root: &Path, namespace: &str) -> Result<SourceSnapshot, String> {
    read_for_save(root, namespace, None)
}
/// Re-read for the exact pending Save, permitting only its correlated recovery
/// state. Ordinary admission must call `read`; this never performs recovery writes.
pub fn read_for_save(
    root: &Path,
    namespace: &str,
    pending: Option<&CommitRequest>,
) -> Result<SourceSnapshot, String> {
    let _guard = crate::read_guard(root, pending).map_err(|error| error.to_string())?;
    let mut snapshot = SourceSnapshot {
        namespace: namespace.into(),
        generation: 1,
        documents: Vec::new(),
        assets: Vec::new(),
        index: None,
    };
    let mut total = 0usize;
    let mut identities = BTreeSet::new();
    for (count, entry) in walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_depth(65)
        .into_iter()
        .enumerate()
    {
        if count >= 8192 {
            return Err("Source scope exceeds the 8192-entry inventory limit".into());
        }
        let entry = entry.map_err(|error| {
            format!(
                "{}: {}",
                error
                    .path()
                    .map_or_else(|| "assets".into(), |path| label(root, path)),
                error.io_error().map_or_else(
                    || "Cannot read source inventory".into(),
                    ToString::to_string
                )
            )
        })?;
        if entry.depth() > 64 {
            return Err("Source scope exceeds depth 64".into());
        }
        if entry.file_type().is_symlink()
            || !(entry.file_type().is_file() || entry.file_type().is_dir())
        {
            return Err(format!(
                "Linked/special sources cannot be admitted: {}",
                label(root, entry.path())
            ));
        }
        if entry.file_type().is_dir() || entry.path().extension().is_some_and(|ext| ext == "meta") {
            continue;
        }
        let path = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| error.to_string())?
            .to_str()
            .ok_or("Source paths must be Unicode")?
            .replace(std::path::MAIN_SEPARATOR, "/");
        if !kedra_assets::delivery::normalized(&path) {
            return Err(format!("Source path is not addressable: {path}"));
        }
        let document = path.ends_with(".scene.ron") || path.ends_with(".prefab.ron");
        let data = bytes(
            root,
            entry.path(),
            if document { 65_536 } else { 8 * 1024 * 1024 },
        )?;
        total = total.saturating_add(data.len());
        if total > 64 * 1024 * 1024 {
            return Err("Source scope exceeds the 64 MiB admission budget".into());
        }
        let mut meta: PathBuf = entry.path().into();
        let mut name = entry.file_name().to_os_string();
        name.push(".meta");
        meta.set_file_name(name);
        let id = match std::fs::symlink_metadata(&meta) {
            Ok(_) => {
                let source = bytes(root, &meta, 65_536)?;
                let identity: Identity = ron::de::from_bytes(&source)
                    .map_err(|error| format!("{}: {error}", label(root, &meta)))?;
                let _: bevy::asset::meta::AssetMetaMinimal =
                    ron::de::from_bytes(&source).map_err(|error| error.to_string())?;
                if identity.id.is_nil() || !identities.insert(identity.id) {
                    return Err(format!("Duplicate/nil sidecar identity: {path}"));
                }
                Some(identity.id)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.to_string()),
        };
        if document {
            let source = String::from_utf8(data).map_err(|error| error.to_string())?;
            let parsed =
                PrefabDocument::parse(&source).map_err(|error| format!("{path}: {error}"))?;
            if id.is_some_and(|id| id != parsed.id.0) {
                return Err(format!(
                    "Document identity differs from its sidecar: {path}"
                ));
            }
            if id.is_none() && !identities.insert(parsed.id.0) {
                return Err(format!(
                    "Document identity collides with another source: {path}"
                ));
            }
            if snapshot.documents.len() >= 128 {
                return Err("Source scope exceeds 128 documents".into());
            }
            snapshot.documents.push(SourceFile {
                path,
                hash: content_hash(&source),
                source,
            });
        } else {
            snapshot.assets.push(SourceAsset {
                id,
                path,
                hash: kedra_assets::delivery::digest(&data),
                bytes: data.len() as u64,
            });
        }
    }
    snapshot.documents.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(snapshot)
}
/// Revalidate an admitted document's exact bytes and sidecar, without writing it.
pub fn check(root: &Path, path: &Path, expected: &str) -> Result<(), String> {
    check_inventory(root, path, expected, None)
}
/// Revalidate an admitted member and optional tracked sidecar identity. Changed,
/// missing, linked, oversized or unaddressable members refuse without mutation.
pub fn check_inventory(
    root: &Path,
    path: &Path,
    expected: &str,
    expected_identity: Option<Uuid>,
) -> Result<(), String> {
    let portable = path
        .to_str()
        .ok_or("Source path is not Unicode")?
        .replace(std::path::MAIN_SEPARATOR, "/");
    if !kedra_assets::delivery::normalized(&portable) {
        return Err("Source path is not addressable".into());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let mut directory = root.to_path_buf();
    for part in std::iter::once(None).chain(parent.components().map(Some)) {
        if let Some(part) = part {
            directory.push(part);
        }
        let metadata = std::fs::symlink_metadata(&directory).map_err(|error| error.to_string())?;
        if !metadata.is_dir() || metadata.is_symlink() {
            return Err("Source ancestor is no longer a regular directory".into());
        }
    }
    let document = portable.ends_with(".scene.ron") || portable.ends_with(".prefab.ron");
    let data = bytes(
        root,
        &root.join(path),
        if document { 65_536 } else { 8 * 1024 * 1024 },
    )?;
    if kedra_assets::delivery::digest(&data) != expected {
        return Err("Source changed since admission. Reopen the project to admit the new inventory; existing documents were kept.".into());
    }
    let mut meta = root.join(path);
    let mut name = path
        .file_name()
        .ok_or("Source filename missing")?
        .to_os_string();
    name.push(".meta");
    meta.set_file_name(name);
    match std::fs::symlink_metadata(&meta) {
        Ok(_) => {
            let data_meta = bytes(root, &meta, 65_536)?;
            let identity: Identity =
                ron::de::from_bytes(&data_meta).map_err(|error| error.to_string())?;
            if expected_identity.is_some_and(|expected| identity.id != expected) {
                return Err("Asset sidecar identity changed after admission".into());
            }
            let _: bevy::asset::meta::AssetMetaMinimal =
                ron::de::from_bytes(&data_meta).map_err(|error| error.to_string())?;
            if document {
                let document = PrefabDocument::parse(
                    std::str::from_utf8(&data).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                if identity.id != document.id.0 {
                    return Err("Source sidecar identity changed after admission".into());
                }
            }
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound && expected_identity.is_none() => {}
        Err(error) => return Err(error.to_string()),
    }
    Ok(())
}
