//! Project-owned source sessions. Browser selection is never document activation.
pub(crate) mod inspection;
mod source;
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, block_on, poll_once},
};
use kedra_authoring::persistence::Workspace;
use kedra_docking::model::ViewId;
use kedra_scene::{
    document::{DocumentId, NodeAddress},
    reflection::ComponentPolicies,
};
use std::{collections::BTreeMap, path::PathBuf};
type SourceResult = (u64, Result<Option<source::Admission>, String>);

pub(crate) struct Opened {
    pub path: PathBuf,
    pub document: DocumentId,
    pub view: ViewId,
    pub selected: Option<NodeAddress>,
    pub graph: std::sync::Arc<kedra_scene::composition::ResolvedGraph>,
}
pub(crate) struct Ready {
    pub path: PathBuf,
    pub document: DocumentId,
    graph: std::sync::Arc<kedra_scene::composition::ResolvedGraph>,
}
#[derive(Default)]
pub(crate) struct Documents {
    scope: Option<Workspace>,
    scope_identity: inspection::ScopeIdentity,
    compiled: BTreeMap<DocumentId, std::sync::Arc<kedra_scene::instance::CompiledPrefab>>,
    paths: BTreeMap<PathBuf, DocumentId>,
    pub opened: BTreeMap<DocumentId, Opened>,
    pub active: Option<DocumentId>,
    clock: u64,
    pending: Option<PathBuf>,
    task: Option<Task<SourceResult>>,
    checked: Option<PathBuf>,
    pub error: Option<String>,
    pub failure: Option<String>,
    pub origin: Option<ViewId>,
    asset_ready: Option<PathBuf>,
}
impl Documents {
    pub(crate) fn update_clean(&self) -> bool {
        self.pending.is_none()
            && self.task.is_none()
            && self.scope.as_ref().is_none_or(|scope| {
                scope.pending().is_none()
                    && scope
                        .working
                        .documents()
                        .keys()
                        .all(|id| !scope.working.dirty(*id))
            })
    }
    /// Immutable admission baseline used only to verify a renderer delivery.
    pub(crate) fn scene_source(&self) -> Option<&kedra_authoring::persistence::SourceSnapshot> {
        self.scope.as_ref().map(|scope| &scope.source)
    }
    pub fn opening(&self) -> bool {
        self.pending.is_some()
    }
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({"active": self.active, "pending": self.pending, "read_only": true,
            "documents": self.opened.values().map(|d| serde_json::json!({"document":d.document, "path":d.path, "view":d.view, "selection":d.selected})).collect::<Vec<_>>()})
    }
    pub fn request(&mut self, world: &World, root: PathBuf, path: PathBuf) -> Result<(), String> {
        if self.pending.as_ref() == Some(&path) {
            return Ok(());
        }
        self.clock = self
            .clock
            .checked_add(1)
            .ok_or("Document request counter exhausted")?;
        self.error = None;
        self.failure = None;
        self.asset_ready = None;
        if self.scope.is_some()
            && !self.paths.contains_key(&path)
            && !self.scope.as_ref().is_some_and(|scope| {
                scope
                    .source
                    .assets
                    .iter()
                    .any(|a| std::path::Path::new(&a.path) == path)
            })
        {
            self.pending = None;
            self.checked = None;
            self.task = None;
            return Err("Source is not in the admitted inventory. Reopen the project to admit newly added documents; existing sources were kept.".into());
        }
        self.pending = Some(path.clone());
        self.checked = None;
        if self.scope.is_none() && self.task.is_none() {
            let clock = self.clock;
            let registry = world.resource::<AppTypeRegistry>().clone();
            let policies = world.resource::<ComponentPolicies>().clone();
            self.task = Some(IoTaskPool::get().spawn(async move {
                let result = source::read(&root)
                    .and_then(|snapshot| source::admit(snapshot, &registry.read(), &policies))
                    .map(Some);
                (clock, result)
            }));
        } else if let Some(scope) = &self.scope {
            if self
                .paths
                .get(&path)
                .is_some_and(|id| self.opened.contains_key(id))
            {
                self.checked = Some(path);
            } else {
                let hash = scope
                    .source
                    .documents
                    .iter()
                    .find(|d| std::path::Path::new(&d.path) == path)
                    .map(|d| d.hash.clone())
                    .or_else(|| {
                        scope
                            .source
                            .assets
                            .iter()
                            .find(|a| std::path::Path::new(&a.path) == path)
                            .map(|a| a.hash.clone())
                    })
                    .ok_or("Admitted baseline missing")?;
                let clock = self.clock;
                let identity = scope
                    .source
                    .assets
                    .iter()
                    .find(|asset| std::path::Path::new(&asset.path) == path)
                    .and_then(|asset| asset.id);
                self.task = Some(IoTaskPool::get().spawn(async move {
                    let result = if identity.is_some() {
                        source::check_inventory(&root, &path, &hash, identity)
                    } else {
                        source::check(&root, &path, &hash)
                    };
                    (clock, result.map(|_| None))
                }));
            }
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Option<Ready> {
        if let Some((clock, result)) = self
            .task
            .as_mut()
            .and_then(|task| block_on(poll_once(task)))
        {
            self.task = None;
            // The inventory belongs to this coordinator/project, not the requested
            // tab. Only the most recent locator can activate after it is admitted.
            match result {
                Ok(Some((scope, compiled))) => {
                    self.paths = source::paths(&scope);
                    self.scope = Some(scope);
                    self.compiled = compiled;
                    self.checked = self.pending.clone();
                }
                Ok(None) if clock == self.clock => {
                    self.checked = self.pending.clone();
                }
                Err(error) if clock == self.clock || self.scope.is_none() => {
                    self.error = Some(error);
                    self.pending = None;
                }
                _ => {}
            }
        }
        if self.scope.is_none() || self.checked != self.pending {
            return None;
        }
        let path = self.pending.take()?;
        let Some(document) = self.paths.get(&path).copied() else {
            if self.scope.as_ref().is_some_and(|scope| {
                scope
                    .source
                    .assets
                    .iter()
                    .any(|a| std::path::Path::new(&a.path) == path)
            }) {
                self.asset_ready = Some(path);
                return None;
            }
            self.error = Some("Source is not an admitted scene/prefab".into());
            return None;
        };
        let graph = if let Some(opened) = self.opened.get(&document) {
            opened.graph.clone()
        } else {
            match self.scope.as_ref()?.working.catalog().resolve(document) {
                Ok(graph) => std::sync::Arc::new(graph),
                Err(error) => {
                    self.error = Some(error.to_string());
                    return None;
                }
            }
        };
        Some(Ready {
            path,
            document,
            graph,
        })
    }
    pub(crate) fn take_asset(&mut self) -> Option<PathBuf> {
        self.asset_ready.take()
    }
    pub fn describe(&self, id: DocumentId) -> Option<(String, String, NodeAddress)> {
        let doc = &self.scope.as_ref()?.working.documents().get(&id)?.document;
        Some((
            doc.title.clone(),
            format!(
                "{}\n\n{}\n{} source nodes · {} nested instances\n\nSource admitted read-only. Use Hierarchy to navigate composed nodes. Inspector, rendered preview, Save and source edits are unavailable.",
                self.scope.as_ref()?.path(id)?,
                id.0,
                doc.nodes.len(),
                doc.instances.len()
            ),
            NodeAddress::from(doc.root),
        ))
    }
    pub fn published(&mut self, ready: Ready, view: ViewId, selected: NodeAddress) {
        self.opened.entry(ready.document).or_insert(Opened {
            path: ready.path,
            document: ready.document,
            view,
            selected: Some(selected),
            graph: ready.graph,
        });
        self.active = Some(ready.document);
    }
    pub fn activate_view(&mut self, view: ViewId) {
        if let Some(opened) = self.opened.values().find(|opened| opened.view == view) {
            self.active = Some(opened.document);
        }
    }
    pub fn select(&mut self, document: DocumentId, node: NodeAddress) -> bool {
        if self.active != Some(document) {
            return false;
        }
        let Some(opened) = self.opened.get_mut(&document) else {
            return false;
        };
        if !opened.graph.nodes.contains_key(&node) {
            return false;
        }
        opened.selected = Some(node);
        true
    }
    pub fn clear_selection(&mut self, view: ViewId) -> bool {
        let Some(opened) = self
            .active
            .and_then(|document| self.opened.get_mut(&document))
        else {
            return false;
        };
        if opened.view != view {
            return false;
        }
        opened.selected = None;
        true
    }
}

#[cfg(test)]
mod tests;
