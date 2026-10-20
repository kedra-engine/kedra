//! Transport-independent canonical workspace/save protocol. No filesystem or HTTP access.
use crate::{
    failure,
    workspace::{WorkingDocument, WorkingSet},
};
use bevy::{asset::LoadFromPath, reflect::TypeRegistry};
use kedra_scene::assets::{CatalogLoader, compile_catalog};
use kedra_scene::{Result, composition::Catalog, document::*, reflection::ComponentPolicies};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

/// Digest the exact UTF-8 source bytes, not a reconstructed or reformatted document.
pub fn content_hash(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// One exact authored source at a normalized workspace-relative path.
pub struct SourceFile {
    /// Portable source path, validated before preparing writes. Native URI
    /// delimiters such as `#` are rejected, not percent-encoded into another path.
    pub path: String,
    /// Original source bytes represented as UTF-8, retained even on decode failure.
    pub source: String,
    /// SHA-256 of the original source.
    pub hash: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Atomic host-supplied source baseline, independent of working copies or gameplay.
pub struct SourceSnapshot {
    /// Explicit installed Bevy source namespace used in persisted asset references.
    pub namespace: String,
    /// Host workspace generation used for compare-and-swap commits.
    pub generation: u64,
    /// Complete scene/prefab source set.
    pub documents: Vec<SourceFile>,
    /// Explicit asset baseline for dependency checks.
    pub assets: Vec<SourceAsset>,
    /// Optional explicit source catalog file, not a hidden generated fallback.
    pub index: Option<SourceFile>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// One host-observed asset baseline; an absent UUID means untracked source data.
pub struct SourceAsset {
    /// Native persistent asset identity when assigned.
    pub id: Option<Uuid>,
    /// Normalized source path.
    pub path: String,
    /// SHA-256 of exact bytes.
    pub hash: String,
    /// Exact byte length, including zero-length assets.
    pub bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// A source read precondition required even when the file is not being modified.
pub struct FileCheck {
    /// Normalized source path.
    pub path: String,
    /// Expected digest, or explicit expected absence.
    pub expected_hash: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// One proposed source write/delete, not a live-world serialization request.
pub struct CommitFile {
    /// Authored document identity, absent for the explicit catalog index.
    pub document: Option<DocumentId>,
    /// Normalized target path.
    pub path: String,
    /// Pinned source revision represented by these bytes.
    pub revision: u64,
    /// Exact previous digest, or expected absence for a new file.
    pub expected_hash: Option<String>,
    /// Exact proposed UTF-8 source, or explicit deletion.
    pub source: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Immutable idempotent save operation; retry this exact request after a lost response.
pub struct CommitRequest {
    /// Operation identity used for receipt correlation.
    pub operation: Uuid,
    /// Required host workspace generation.
    pub expected_generation: u64,
    /// Complete source inventory precondition.
    pub expected_sources: Vec<String>,
    /// Additional unchanged source/dependency preconditions.
    pub checks: Vec<FileCheck>,
    /// Exact coupled writes and deletes.
    pub files: Vec<CommitFile>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Host receipt for one committed source member.
pub struct SavedFile {
    /// Correlated document, or the source catalog.
    pub document: Option<DocumentId>,
    /// Committed normalized path.
    pub path: String,
    /// Revision represented by the submitted bytes, not a newer local edit.
    pub revision: u64,
    /// Digest of committed bytes, or confirmed deletion.
    pub hash: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// A correlated host save acknowledgment. Receiving it never clears newer edits.
pub struct Receipt {
    /// Exact submitted operation identity.
    pub operation: Uuid,
    /// New host workspace generation.
    pub generation: u64,
    /// Complete committed member receipts.
    pub files: Vec<SavedFile>,
}

/// Retains original source bytes even when decoding/compilation fails. One save may be
/// in flight; lost responses retry the exact request, never a regenerated operation.
#[derive(Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Revision-guarded source working set, distinct from the baseline.
    pub working: WorkingSet,
    /// Last acknowledged host source baseline, including undecodable original bytes.
    pub source: SourceSnapshot,
    /// Explicit source/compilation failure. A diagnostic workspace is not a runnable fallback.
    pub diagnostic: Option<String>,
    paths: BTreeMap<DocumentId, String>,
    pending: Option<CommitRequest>,
    last_receipt: Option<Uuid>,
}
impl Workspace {
    /// Allocate an unused portable source path from a nonempty title; unsupported slugs fail.
    pub fn assign_new_path(&mut self, id: DocumentId) -> Result<()> {
        let document = &self
            .working
            .documents
            .get(&id)
            .ok_or_else(|| failure("document missing"))?
            .document;
        let mut slug = String::new();
        for character in document.title.chars() {
            if character.is_alphanumeric() {
                slug.extend(character.to_lowercase());
            } else if !slug.is_empty() && !slug.ends_with('-') {
                slug.push('-');
            }
        }
        let slug = slug.trim_matches('-');
        if slug.is_empty() {
            return Err(failure("source title needs an explicit portable path"));
        }
        let extension = match document.kind {
            DocumentKind::Prefab => "prefab.ron",
            DocumentKind::Scene => "scene.ron",
        };
        let folder = if document.kind == DocumentKind::Scene {
            "scenes"
        } else {
            "prefabs"
        };
        let mut suffix = 1;
        let path = loop {
            let path = if suffix == 1 {
                format!("{folder}/{slug}.{extension}")
            } else {
                format!("{folder}/{slug}-{suffix}.{extension}")
            };
            if !self.paths.values().any(|existing| existing == &path) {
                break path;
            }
            suffix += 1;
        };
        self.assign_path(id, path)
    }
    /// Whether the explicit catalog inventory differs from the acknowledged source set.
    pub fn index_dirty(&self) -> bool {
        let mut paths = self
            .paths
            .iter()
            .filter(|(id, _)| self.working.documents.contains_key(id))
            .map(|(_, path)| format!("{}://{path}", self.source.namespace))
            .collect::<Vec<_>>();
        paths.sort();
        self.source
            .index
            .as_ref()
            .and_then(|index| ron::from_str::<Vec<String>>(&index.source).ok())
            .is_none_or(|mut index| {
                index.sort();
                index != paths
            })
    }
    /// Explicit conflict decision: retain dirty working copies against a newly
    /// reviewed disk baseline. No source bytes are written until the next Save.
    pub fn keep_working(
        &mut self,
        source: SourceSnapshot,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
        loader: &mut dyn LoadFromPath,
    ) -> Result<()> {
        if source.namespace != self.source.namespace {
            return Err(failure(
                "workspace asset namespace cannot change during source reconciliation",
            ));
        }
        if self.pending.is_some() {
            return Err(failure("reconcile pending Save first"));
        }
        let refreshed = Self::open(source, types, policies, loader);
        if let Some(error) = refreshed.diagnostic {
            return Err(failure(error));
        }
        let dirty = self
            .working
            .documents
            .keys()
            .copied()
            .filter(|id| self.working.dirty(*id))
            .collect::<BTreeSet<_>>();
        let mut candidate = self.working.clone();
        candidate
            .documents
            .retain(|id, _| dirty.contains(id) || refreshed.working.documents.contains_key(id));
        for (id, document) in &refreshed.working.documents {
            if !dirty.contains(id) {
                candidate.clock = candidate
                    .clock
                    .max(
                        candidate
                            .documents
                            .values()
                            .map(|doc| doc.revision)
                            .max()
                            .unwrap_or(0),
                    )
                    .checked_add(1)
                    .ok_or_else(|| failure("authoring revision counter exhausted"))?;
                candidate.documents.insert(
                    *id,
                    WorkingDocument {
                        document: document.document.clone(),
                        revision: candidate.clock,
                        saved_revision: candidate.clock,
                    },
                );
            }
        }
        compile_catalog(&candidate.catalog(), types, policies, loader)?;
        let mut paths = refreshed.paths;
        for id in dirty {
            if !paths.contains_key(&id)
                && let Some(path) = self.paths.get(&id)
            {
                paths.insert(id, path.clone());
            }
        }
        validate_assignments(&candidate.catalog(), &paths, &refreshed.source)?;
        self.working = candidate;
        self.paths = paths;
        self.source = refreshed.source;
        self.diagnostic = None;
        Ok(())
    }
    /// Inspect an assigned source path; missing assignments remain explicit.
    pub fn path(&self, id: DocumentId) -> Option<&str> {
        self.paths.get(&id).map(String::as_str)
    }
    /// Admit an exact host baseline, retaining original bytes and diagnostics on any failure.
    /// Failed decoding does not install a runnable replacement source.
    pub fn open(
        source: SourceSnapshot,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
        loader: &mut dyn LoadFromPath,
    ) -> Self {
        let mut workspace = Self {
            working: WorkingSet::default(),
            source,
            diagnostic: None,
            paths: BTreeMap::new(),
            pending: None,
            last_receipt: None,
        };
        if let Err(error) = workspace.decode(types, policies, loader) {
            workspace.diagnostic = Some(error.to_string());
        }
        workspace
    }
    fn decode(
        &mut self,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
        loader: &mut dyn LoadFromPath,
    ) -> Result<()> {
        if self.source.namespace.is_empty()
            || self.source.namespace.len() > 64
            || !self
                .source
                .namespace
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(failure(
                "workspace requires an explicit portable asset namespace",
            ));
        }
        let mut working = WorkingSet::default();
        let mut paths = BTreeMap::new();
        let mut unique = BTreeSet::new();
        for file in &self.source.documents {
            if !unique.insert(file.path.clone()) || file.hash != content_hash(&file.source) {
                return Err(failure("duplicate source path or invalid source hash"));
            }
            let document = PrefabDocument::parse(&file.source)?;
            validate_path(&file.path, document.kind)?;
            paths.insert(document.id, file.path.clone());
            working.open(document)?;
        }
        for asset in &self.source.assets {
            if !unique.insert(asset.path.clone())
                || asset.path.len() > 4096
                || asset.path.contains([':', '\\', '\0'])
                || asset
                    .path
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
                || asset.hash.len() != 64
                || !asset.hash.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(failure("invalid or duplicate workspace asset baseline"));
            }
        }
        if let Some(index) = &self.source.index {
            if index.path != "catalog.prefabs.ron"
                || index.hash != content_hash(&index.source)
                || index.source.len() > 65_536
            {
                return Err(failure("invalid source catalog baseline"));
            }
            let actual = CatalogLoader::parse(&index.source)?;
            let mut resolved = BTreeSet::new();
            for reference in &actual {
                let document = working
                    .documents
                    .values()
                    .find(|entry| {
                        let document = &entry.document;
                        let extension = match document.kind {
                            DocumentKind::Prefab => "prefab.ron",
                            DocumentKind::Scene => "scene.ron",
                        };
                        reference
                            == &format!(
                                "{}://{}/asset.{extension}",
                                self.source.namespace, document.id
                            )
                            || paths.get(&document.id).is_some_and(|path| {
                                reference == &format!("{}://{path}", self.source.namespace)
                            })
                    })
                    .ok_or_else(|| {
                        failure("source catalog contains an undeclared asset reference")
                    })?;
                if !resolved.insert(document.document.id) {
                    return Err(failure("duplicate document reference in source catalog"));
                }
            }
            if resolved.len() != working.documents.len() {
                return Err(failure(
                    "source catalog must contain exactly the declared documents",
                ));
            }
        }
        compile_catalog(&working.catalog(), types, policies, loader)?;
        self.paths = paths;
        self.working = working;
        Ok(())
    }
    /// Inspect the immutable in-flight save request for same-operation retry.
    pub fn pending(&self) -> Option<&CommitRequest> {
        self.pending.as_ref()
    }
    /// Assign a validated unused source path; collisions and pending saves reject changes.
    pub fn assign_path(&mut self, id: DocumentId, path: String) -> Result<()> {
        if self.pending.is_some() {
            return Err(failure("save in flight"));
        }
        if self.paths.contains_key(&id) {
            return Err(failure("managed rename requires a workspace transaction"));
        }
        let document = self
            .working
            .documents
            .get(&id)
            .ok_or_else(|| failure("document missing"))?;
        validate_path(&path, document.document.kind)?;
        if self.paths.values().any(|p| p == &path)
            || self.source.assets.iter().any(|asset| asset.path == path)
        {
            return Err(failure("source path already assigned"));
        }
        self.paths.insert(id, path);
        Ok(())
    }
    /// Validate and pin coupled source bytes and preconditions into one idempotent request.
    /// No filesystem or live world is modified; a host writer owns actual commit.
    pub fn prepare_save(
        &mut self,
        selected: Option<DocumentId>,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
        loader: &mut dyn LoadFromPath,
    ) -> Result<&CommitRequest> {
        if self.diagnostic.is_some() {
            return Err(failure(
                "workspace sources are read-only until diagnostics are resolved",
            ));
        }
        if self.pending.is_some() {
            return Err(failure(
                "reconcile the pending save before starting another",
            ));
        }
        let working = self.working.catalog();
        validate_assignments(&working, &self.paths, &self.source)?;
        // Reject invalid dirty sources, including components/patch schemas, before I/O.
        let snapshots = compile_catalog(&working, types, policies, loader)?;
        let mut group: BTreeSet<_> = match selected {
            Some(id) => {
                if !working.documents.contains_key(&id) {
                    return Err(failure("document missing"));
                }
                self.working.save_group(id)
            }
            None => self
                .working
                .documents
                .keys()
                .copied()
                .filter(|id| self.working.dirty(*id))
                .collect(),
        };
        loop {
            let before = group.len();
            for id in group.clone() {
                group.extend(self.working.save_group(id));
                group.extend(
                    working
                        .resolve(id)?
                        .dependencies
                        .into_iter()
                        .filter(|id| self.working.dirty(*id)),
                );
                group.extend(
                    snapshots[&id]
                        .document_links()
                        .iter()
                        .copied()
                        .filter(|id| self.working.dirty(*id)),
                );
            }
            if group.len() == before {
                break;
            }
        }
        let mut committed = Catalog::default();
        for file in &self.source.documents {
            let doc = PrefabDocument::parse(&file.source)?;
            committed.documents.insert(doc.id, doc);
        }
        let mut files = Vec::new();
        for id in group {
            let document = self.working.documents.get(&id).ok_or_else(|| {
                failure("document deletion requires a managed delete transaction")
            })?;
            if !self.working.dirty(id) {
                continue;
            }
            let path = self
                .paths
                .get(&id)
                .ok_or_else(|| failure("assign a source path before Save"))?
                .clone();
            let original = self.source.documents.iter().find(|file| file.path == path);
            let source = document.document.serialize()?;
            // Validate the exact proposed bytes, including pretty-print expansion.
            PrefabDocument::parse(&source)?;
            committed.documents.insert(id, document.document.clone());
            files.push(CommitFile {
                document: Some(id),
                path,
                revision: document.revision,
                expected_hash: original.map(|file| file.hash.clone()),
                source: Some(source),
            });
        }
        // Validate the actual post-commit graph, not a graph containing unrelated unsaved edits.
        compile_catalog(&committed, types, policies, loader)?;
        validate_assignments(&committed, &self.paths, &self.source)?;
        let mut paths = committed
            .documents
            .keys()
            .map(|id| {
                self.paths
                    .get(id)
                    .map(|path| format!("{}://{path}", self.source.namespace))
                    .ok_or_else(|| failure("committed document has no source path"))
            })
            .collect::<Result<Vec<_>>>()?;
        paths.sort();
        let index_source =
            ron::ser::to_string_pretty(&paths, ron::ser::PrettyConfig::default())? + "\n";
        CatalogLoader::parse(&index_source)?;
        if self
            .source
            .index
            .as_ref()
            .is_none_or(|index| index.source != index_source)
        {
            files.push(CommitFile {
                document: None,
                path: "catalog.prefabs.ron".into(),
                revision: files.iter().map(|file| file.revision).max().unwrap_or(1),
                expected_hash: self.source.index.as_ref().map(|file| file.hash.clone()),
                source: Some(index_source),
            });
        }
        if files.is_empty() {
            return Err(failure("nothing to save"));
        }
        let mut targets = BTreeSet::new();
        if files.iter().any(|file| !targets.insert(&file.path)) {
            return Err(failure("duplicate Save target path"));
        }
        let mut checks: Vec<_> = self
            .source
            .documents
            .iter()
            .map(|file| FileCheck {
                path: file.path.clone(),
                expected_hash: Some(file.hash.clone()),
            })
            .collect();
        checks.extend(self.source.assets.iter().map(|asset| FileCheck {
            path: asset.path.clone(),
            expected_hash: Some(asset.hash.clone()),
        }));
        checks.push(FileCheck {
            path: "catalog.prefabs.ron".into(),
            expected_hash: self.source.index.as_ref().map(|index| index.hash.clone()),
        });
        self.pending = Some(CommitRequest {
            operation: Uuid::new_v4(),
            expected_generation: self.source.generation,
            expected_sources: self
                .source
                .documents
                .iter()
                .map(|file| file.path.clone())
                .collect(),
            checks,
            files,
        });
        self.pending
            .as_ref()
            .ok_or_else(|| failure("save request missing"))
    }
    /// Verify an exact receipt and update the baseline without clearing newer working edits.
    pub fn acknowledge(&mut self, receipt: &Receipt) -> Result<()> {
        if self.pending.is_none() && self.last_receipt == Some(receipt.operation) {
            return Ok(());
        }
        let request = self
            .pending
            .as_ref()
            .ok_or_else(|| failure("no pending save"))?;
        if request.operation != receipt.operation
            || Some(receipt.generation) != request.expected_generation.checked_add(1)
            || receipt.files.len() != request.files.len()
        {
            return Err(failure("save receipt does not match pending transaction"));
        }
        let mut paths = BTreeSet::new();
        for saved in &receipt.files {
            let submitted = request
                .files
                .iter()
                .find(|file| file.path == saved.path)
                .ok_or_else(|| failure("unexpected saved path"))?;
            if !paths.insert(&saved.path)
                || submitted.document != saved.document
                || submitted.revision != saved.revision
                || submitted.source.as_deref().map(content_hash) != saved.hash
            {
                return Err(failure("save receipt content mismatch"));
            }
        }
        let mut revisions = BTreeMap::new();
        for file in &request.files {
            if file.document.is_none() {
                self.source.index = file.source.as_ref().map(|source| SourceFile {
                    path: file.path.clone(),
                    source: source.clone(),
                    hash: content_hash(source),
                });
                continue;
            }
            self.source.documents.retain(|old| old.path != file.path);
            if let Some(source) = &file.source {
                self.source.documents.push(SourceFile {
                    path: file.path.clone(),
                    source: source.clone(),
                    hash: content_hash(source),
                });
            }
            if let Some(id) = file.document {
                revisions.insert(id, file.revision);
            }
        }
        self.source.generation = receipt.generation;
        self.working.acknowledge(&revisions);
        self.last_receipt = Some(receipt.operation);
        self.pending = None;
        Ok(())
    }
    /// Only call after the writer conclusively refuses this operation. A timeout is
    /// ambiguous and must retain pending bytes for status/retry reconciliation.
    pub fn refused(&mut self, operation: Uuid) -> Result<()> {
        if self
            .pending
            .as_ref()
            .is_none_or(|request| request.operation != operation)
        {
            return Err(failure("unknown save operation"));
        }
        self.pending = None;
        Ok(())
    }
}
// Validate the complete merge and independently defend restored session state at Save.
// Unassigned new documents are allowed until selected for Save; undo-retained path
// reservations do not claim a path while their document is absent from this catalog.
fn validate_assignments(
    catalog: &Catalog,
    paths: &BTreeMap<DocumentId, String>,
    source: &SourceSnapshot,
) -> Result<()> {
    let mut baseline = BTreeMap::new();
    let mut inventory = BTreeSet::from(["catalog.prefabs.ron".to_owned()]);
    for asset in &source.assets {
        if !inventory.insert(asset.path.clone()) {
            return Err(failure("duplicate or reserved source asset path"));
        }
    }
    for file in &source.documents {
        if !inventory.insert(file.path.clone()) {
            return Err(failure("duplicate workspace source path"));
        }
        baseline.insert(file.path.clone(), PrefabDocument::parse(&file.source)?.id);
    }
    let mut assigned = BTreeSet::from(["catalog.prefabs.ron".to_owned()]);
    assigned.extend(source.assets.iter().map(|asset| asset.path.clone()));
    for (id, document) in &catalog.documents {
        if let Some(path) = paths.get(id) {
            validate_path(path, document.kind)?;
            if !assigned.insert(path.clone()) || baseline.get(path).is_some_and(|owner| owner != id)
            {
                return Err(failure(
                    "workspace source path is owned by another document or asset",
                ));
            }
        }
    }
    Ok(())
}
fn validate_path(path: &str, kind: DocumentKind) -> Result<()> {
    let extension = match kind {
        DocumentKind::Prefab => ".prefab.ron",
        DocumentKind::Scene => ".scene.ron",
    };
    if path.len() > 4096
        || path.contains([':', '#'])
        || path.contains('\0')
        || !path.ends_with(extension)
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(failure("invalid workspace document path"));
    }
    let native = bevy::asset::AssetPath::try_parse(path)
        .map_err(|_| failure("document path is not a native asset path"))?;
    if native.label().is_some()
        || native.source() != &bevy::asset::io::AssetSourceId::Default
        || native.path() != std::path::Path::new(path)
    {
        return Err(failure(
            "document path changes meaning as a native asset reference",
        ));
    }
    Ok(())
}
