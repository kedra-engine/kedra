use kedra_authoring::{persistence::SourceSnapshot, workspace::WorkingSet};
use kedra_scene::document::{DocumentId, NodeAddress, PrefabDocument};
use serde::Serialize;
use uuid::Uuid;

/// One host-owned project/source lifetime. Retiring it invalidates old references.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ScopeId(Uuid);
impl Default for ScopeId {
    fn default() -> Self {
        Self(Uuid::new_v4())
    }
}
/// Presentation category, not a native loader type or mutation permission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum AssetKind {
    /// Existing scene or prefab document.
    Document(DocumentId),
    /// Existing non-document source file, possibly untracked.
    File,
    /// Logical folder containing admitted members.
    Folder,
}
/// Source-qualified logical asset reference. Revalidate it against the current
/// AssetLibrary before acting; neither selection nor drag grants source authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AssetRef {
    scope: ScopeId,
    path: String,
    identity: Option<Uuid>,
    kind: AssetKind,
}
impl AssetRef {
    /// Exact source-qualified path, with native case/Unicode preserved.
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Persistent asset identity when present; folders/untracked files have none.
    pub fn identity(&self) -> Option<Uuid> {
        self.identity
    }
    /// Source document/file/folder category, not a native asset type.
    pub fn kind(&self) -> &AssetKind {
        &self.kind
    }
    /// Project/source lifetime that admitted this reference.
    pub fn scope(&self) -> ScopeId {
        self.scope
    }
}
/// Full source node reference, independent of runtime entities and panel ViewIds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NodeRef {
    scope: ScopeId,
    /// Receiving document, not an upstream prefab Apply destination.
    pub document: DocumentId,
    /// Full composed node address in that document.
    pub node: NodeAddress,
}
impl NodeRef {
    /// Project/source lifetime that admitted this node reference.
    pub fn scope(&self) -> ScopeId {
        self.scope
    }
}
/// A borrowed view of the existing admitted source inventory and working catalog.
/// No filesystem scan, published-only registry or duplicate identity index is built.
pub struct AssetLibrary<'a> {
    scope: ScopeId,
    source: &'a SourceSnapshot,
    working: &'a WorkingSet,
}
impl<'a> AssetLibrary<'a> {
    /// Attach a host-owned lifetime to its current existing source authority.
    pub fn new(scope: ScopeId, source: &'a SourceSnapshot, working: &'a WorkingSet) -> Self {
        Self {
            scope,
            source,
            working,
        }
    }
    /// Resolve an exact source-qualified asset/folder path. Missing and foreign
    /// source members refuse; selecting this value never opens or loads anything.
    pub fn asset(&self, path: &str) -> Option<AssetRef> {
        let relative = path.strip_prefix(&format!("{}://", self.source.namespace))?;
        if relative.contains(['\\', ':', '#', '\0'])
            || (!relative.is_empty()
                && relative
                    .split('/')
                    .any(|part| matches!(part, "" | "." | "..")))
        {
            return None;
        }
        let (identity, kind) = if let Some(file) = self
            .source
            .documents
            .iter()
            .find(|file| file.path == relative)
        {
            let id = PrefabDocument::parse(&file.source).ok()?.id;
            (Some(id.0), AssetKind::Document(id))
        } else if let Some(asset) = self
            .source
            .assets
            .iter()
            .find(|asset| asset.path == relative)
        {
            (asset.id, AssetKind::File)
        } else {
            let prefix = if relative.is_empty() {
                String::new()
            } else {
                format!("{relative}/")
            };
            if !self
                .source
                .documents
                .iter()
                .any(|file| file.path.starts_with(&prefix))
                && !self
                    .source
                    .assets
                    .iter()
                    .any(|asset| asset.path.starts_with(&prefix))
            {
                return None;
            }
            (None, AssetKind::Folder)
        };
        Some(AssetRef {
            scope: self.scope,
            path: path.into(),
            identity,
            kind,
        })
    }
    /// Resolve a live composed source node without admitting runtime entities.
    pub fn node(&self, document: DocumentId, node: NodeAddress) -> Option<NodeRef> {
        self.working
            .catalog()
            .resolve(document)
            .ok()?
            .nodes
            .get(&node)?;
        Some(NodeRef {
            scope: self.scope,
            document,
            node,
        })
    }
    /// Validate identity and lifetime against the current source authority.
    pub fn contains_asset(&self, asset: &AssetRef) -> bool {
        asset.scope == self.scope && self.asset(&asset.path).as_ref() == Some(asset)
    }
    /// Validate node membership and lifetime after edits, Undo or scope retirement.
    pub fn contains_node(&self, node: &NodeRef) -> bool {
        node.scope == self.scope && self.node(node.document, node.node.clone()).is_some()
    }
    /// Resolve a document asset to its effective root for source inspection, without Open.
    pub fn root(&self, asset: &AssetRef) -> Option<NodeRef> {
        if !self.contains_asset(asset) {
            return None;
        }
        let AssetKind::Document(document) = asset.kind else {
            return None;
        };
        let graph = self.working.catalog().resolve(document).ok()?;
        self.node(document, graph.root)
    }
}
