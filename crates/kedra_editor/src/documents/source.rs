//! Read-only complete source-scope admission. Never initializes sidecars or catalogs.
use bevy::{
    asset::{AssetPath, LoadFromPath, UntypedHandle},
    reflect::TypeRegistry,
};
use kedra_assets::delivery::{digest, normalized};
use kedra_authoring::persistence::{SourceAsset, SourceFile, SourceSnapshot, Workspace};
use kedra_scene::{document::PrefabDocument, reflection::ComponentPolicies};
use serde::Deserialize;
use std::{
    any::TypeId,
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
use uuid::Uuid;
pub(super) const NAMESPACE: &str = "game";
#[derive(Deserialize)]
struct Identity {
    id: Uuid,
}

fn bytes(root: &Path, path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let label = crate::display_paths::source(root, path);
    let meta = std::fs::symlink_metadata(path).map_err(|e| format!("{label}: {e}"))?;
    if !meta.is_file() || meta.is_symlink() || meta.len() > limit as u64 {
        return Err(format!(
            "Source must be a regular file of at most {limit} bytes: {}",
            label
        ));
    }
    let mut result = Vec::new();
    File::open(path)
        .and_then(|f| {
            let meta = f.metadata()?;
            if !meta.is_file() || meta.len() > limit as u64 {
                return Err(std::io::Error::other(
                    "Source changed to a nonregular or oversized file",
                ));
            }
            f.take(limit as u64 + 1).read_to_end(&mut result)
        })
        .map_err(|e| e.to_string())?;
    if result.len() > limit {
        return Err("Source grew beyond its admission limit".into());
    }
    Ok(result)
}

/// One read-only inventory, capped at 8192 entries, depth 64, 128 documents and
/// 64 MiB aggregate data. Native replacement races are not a hostile-writer sandbox.
pub(super) fn read(root: &Path) -> Result<SourceSnapshot, String> {
    let mut snapshot = SourceSnapshot {
        namespace: NAMESPACE.into(),
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
                error.path().map_or_else(
                    || "assets".into(),
                    |path| crate::display_paths::source(root, path)
                ),
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
                crate::display_paths::source(root, entry.path())
            ));
        }
        if entry.file_type().is_dir() || entry.path().extension().is_some_and(|ext| ext == "meta") {
            continue;
        }
        let path = entry
            .path()
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_str()
            .ok_or("Source paths must be Unicode")?
            .replace(std::path::MAIN_SEPARATOR, "/");
        if !normalized(&path) {
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
                    .map_err(|e| format!("{}: {e}", crate::display_paths::source(root, &meta)))?;
                let _: bevy::asset::meta::AssetMetaMinimal =
                    ron::de::from_bytes(&source).map_err(|e| e.to_string())?;
                if identity.id.is_nil() || !identities.insert(identity.id) {
                    return Err(format!("Duplicate/nil sidecar identity: {path}"));
                }
                Some(identity.id)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        if document {
            let source = String::from_utf8(data).map_err(|e| e.to_string())?;
            let parsed = PrefabDocument::parse(&source).map_err(|e| format!("{path}: {e}"))?;
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
                hash: digest(source.as_bytes()),
                source,
            });
        } else {
            snapshot.assets.push(SourceAsset {
                id,
                path,
                hash: digest(&data),
                bytes: data.len() as u64,
            });
        }
    }
    snapshot.documents.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(snapshot)
}

pub(super) fn check(root: &Path, path: &Path, expected: &str) -> Result<(), String> {
    check_inventory(root, path, expected, None)
}

pub(super) fn check_inventory(
    root: &Path,
    path: &Path,
    expected: &str,
    expected_identity: Option<Uuid>,
) -> Result<(), String> {
    let portable = path
        .to_str()
        .ok_or("Source path is not Unicode")?
        .replace(std::path::MAIN_SEPARATOR, "/");
    if !normalized(&portable) {
        return Err("Source path is not addressable".into());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let mut directory = root.to_path_buf();
    for part in std::iter::once(None).chain(parent.components().map(Some)) {
        if let Some(part) = part {
            directory.push(part);
        }
        let metadata = std::fs::symlink_metadata(&directory).map_err(|e| e.to_string())?;
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
    if digest(&data) != expected {
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
            let identity: Identity = ron::de::from_bytes(&data_meta).map_err(|e| e.to_string())?;
            if expected_identity.is_some_and(|expected| identity.id != expected) {
                return Err("Asset sidecar identity changed after admission".into());
            }
            let _: bevy::asset::meta::AssetMetaMinimal =
                ron::de::from_bytes(&data_meta).map_err(|e| e.to_string())?;
            if document {
                let document =
                    PrefabDocument::parse(std::str::from_utf8(&data).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                if identity.id != document.id.0 {
                    return Err("Source sidecar identity changed after admission".into());
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && expected_identity.is_none() => {}
        Err(e) => return Err(e.to_string()),
    }
    Ok(())
}

/// Reflection-only source handles resolve real inventory identities, never load or
/// execute assets. These UUID handles do NOT claim renderer/dependency readiness.
struct SourceHandles<'a> {
    source: &'a SourceSnapshot,
    error: Option<String>,
}
impl LoadFromPath for SourceHandles<'_> {
    fn load_from_path_erased(
        &mut self,
        type_id: TypeId,
        path: AssetPath<'static>,
    ) -> UntypedHandle {
        let value = path.to_string();
        let resolved = value
            .split_once("://")
            .filter(|(namespace, _)| *namespace == self.source.namespace)
            .and_then(|(_, locator)| {
                let locator = locator.split('#').next()?;
                let identity = locator.split_once('/').and_then(|(id, leaf)| {
                    (leaf.starts_with("asset.") && leaf.len() > 6 && !leaf.contains('/'))
                        .then(|| id.parse::<Uuid>().ok())
                        .flatten()
                });
                self.source
                    .assets
                    .iter()
                    .find(|a| a.path == locator || (identity.is_some() && a.id == identity))
                    .and_then(|a| a.id)
                    .or_else(|| {
                        self.source
                            .documents
                            .iter()
                            .filter_map(|d| {
                                PrefabDocument::parse(&d.source).ok().map(|doc| (d, doc))
                            })
                            .find(|(d, doc)| d.path == locator || Some(doc.id.0) == identity)
                            .map(|(_, doc)| doc)
                            .map(|d| d.id.0)
                    })
            });
        if resolved.is_none() {
            self.error = Some(format!(
                "Referenced asset is outside the game source scope or has no metadata: {value}"
            ));
        }
        UntypedHandle::Uuid {
            type_id,
            uuid: resolved.unwrap_or(Uuid::nil()),
        }
    }
}

pub(super) fn admit(
    source: SourceSnapshot,
    types: &TypeRegistry,
    policies: &ComponentPolicies,
) -> Result<Admission, String> {
    let mut loader = SourceHandles {
        source: &source,
        error: None,
    };
    let (workspace, compiled) =
        Workspace::open_compiled(source.clone(), types, policies, &mut loader);
    if let Some(error) = loader.error.or_else(|| workspace.diagnostic.clone()) {
        return Err(error);
    }
    Ok((workspace, compiled))
}

pub(super) type Admission = (
    Workspace,
    BTreeMap<
        kedra_scene::document::DocumentId,
        std::sync::Arc<kedra_scene::instance::CompiledPrefab>,
    >,
);

pub(super) fn paths(workspace: &Workspace) -> BTreeMap<PathBuf, kedra_scene::document::DocumentId> {
    workspace
        .working
        .documents()
        .keys()
        .filter_map(|id| workspace.path(*id).map(|path| (PathBuf::from(path), *id)))
        .collect()
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
