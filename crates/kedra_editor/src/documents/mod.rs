//! Project-owned source sessions. Browser selection is never document activation.
pub(crate) mod components;
mod context;
mod creation;
mod folders;
mod models;
pub(crate) mod structure;
pub(crate) use structure::ObjectKind;
mod editing;
pub(crate) use editing::{PreparedChange, PropertyChange};
pub(crate) mod environments;
#[cfg(test)]
mod import_tests;
pub(crate) mod inspection;
mod references;
mod saving;
use kedra_authoring::source;
use kedra_authoring_fs::source as source_files;
#[cfg(test)]
#[path = "transactions/tests.rs"]
mod transaction_tests;
use bevy::{prelude::*, tasks::IoTaskPool};
use kedra_authoring::editing::SourceEdit;
use kedra_authoring::persistence::Workspace;
use kedra_docking::model::ViewId;
use kedra_scene::{
    document::{DocumentId, NodeAddress},
    reflection::ComponentPolicies,
};
use kedra_wakeup::{Task, Wakeup};
pub(crate) use references::{ReferenceRequest, ResourceContext};
use std::{collections::BTreeMap, path::PathBuf};
type SourceResult = (u64, Result<Option<source::Admission>, String>);

pub(crate) struct Opened {
    // Observation retains the admitted locator; source ownership stays in Workspace.
    #[cfg(debug_assertions)]
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
    pub(crate) environments: kedra_authoring::environment::Edits,
    creation: creation::Creation,
    scope: Option<Workspace>,
    render_source: Option<kedra_authoring::persistence::SourceSnapshot>,
    save: saving::State,
    scope_identity: kedra_editor_context::ScopeId,
    pub(crate) selection: kedra_editor_context::Selection,
    pending_selection: Option<String>,
    compiled: BTreeMap<DocumentId, std::sync::Arc<kedra_scene::instance::CompiledPrefab>>,
    compiled_revisions: BTreeMap<DocumentId, u64>,
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
    /// Admit the existing source catalog for passive consumers without opening a
    /// document. Shares the ordinary Open task/lease and never executes source code.
    pub(crate) fn ensure_source(&mut self, world: &World, root: PathBuf) {
        if self.scope.is_some() || self.task.is_some() || self.creating() || self.error.is_some() {
            return;
        }
        let clock = self.clock;
        let registry = world.resource::<AppTypeRegistry>().clone();
        let policies = world.resource::<ComponentPolicies>().clone();
        self.task = Some(
            Wakeup::for_world(world).spawn(IoTaskPool::get(), async move {
                let result = source_files::read(&root, "game")
                    .and_then(|snapshot| source::admit(snapshot, &registry.read(), &policies))
                    .map(Some);
                (clock, result)
            }),
        );
    }
    pub(crate) fn imported(
        &mut self,
        source: kedra_authoring::persistence::SourceSnapshot,
        types: &bevy::reflect::TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        if self.environments.unsettled() {
            return Err("Save or discard environment edits before importing assets".into());
        }
        let Some(scope) = &mut self.scope else {
            return Ok(());
        };
        if !scope.source.documents.iter().all(|old| {
            source
                .documents
                .iter()
                .any(|new| old.path == new.path && old.hash == new.hash)
        }) || scope.source.index.as_ref().map(|i| &i.hash)
            != source.index.as_ref().map(|i| &i.hash)
        {
            return Err(
                "Sources changed outside import; reopen the project to reconcile them".into(),
            );
        }
        self.compiled = source::reconcile(scope, source, types, policies)?;
        self.environments = default();
        self.paths = scope
            .working
            .documents()
            .keys()
            .filter_map(|id| scope.path(*id).map(|p| (PathBuf::from(p), *id)))
            .collect();
        self.render_source = None;
        self.compiled_revisions = self.revisions();
        self.reconcile_selection();
        for (id, opened) in &mut self.opened {
            if let Some(compiled) = self.compiled.get(id) {
                opened.graph = std::sync::Arc::new(compiled.graph().clone());
            }
        }
        Ok(())
    }
    pub(crate) fn contains(&self, document: DocumentId) -> bool {
        self.scope
            .as_ref()
            .is_some_and(|scope| scope.working.documents().contains_key(&document))
    }
    pub(crate) fn edit_scope(&self) -> kedra_editor_context::ScopeId {
        self.scope_identity
    }
    pub(crate) fn begin_edit(&self, label: &str, grants: &[String]) -> Result<SourceEdit, String> {
        SourceEdit::begin(
            self.scope.as_ref().ok_or("Source workspace unavailable")?,
            label,
            grants,
        )
    }
    pub(crate) fn edit_workspace(&mut self) -> Result<&mut Workspace, String> {
        self.scope
            .as_mut()
            .ok_or("Source workspace unavailable".into())
    }
    pub(crate) fn commit_edit(
        &mut self,
        edit: &mut SourceEdit,
        types: &bevy::reflect::TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<Option<kedra_authoring::workspace::HistoryId>, String> {
        let outcome = edit.commit(self.edit_workspace()?, types, policies)?;
        self.compiled = outcome.compiled;
        self.compiled_revisions = self.revisions();
        self.reconcile_selection();
        for (id, opened) in &mut self.opened {
            if let Some(compiled) = self.compiled.get(id) {
                opened.graph = std::sync::Arc::new(compiled.graph().clone());
            }
        }
        Ok(outcome.history)
    }
    pub(crate) fn source_ready(&self) -> bool {
        self.scope.is_some() && !self.opening()
    }
    pub(crate) fn update_clean(&self) -> bool {
        !self.environments.unsettled()
            && !self.creating()
            && self.pending.is_none()
            && self.task.is_none()
            && self
                .scope
                .as_ref()
                .is_none_or(|scope| scope.pending().is_none() && !scope.working.is_dirty())
    }
    /// Immutable admission baseline used only to verify a renderer delivery.
    pub(crate) fn scene_source(&self) -> Option<&kedra_authoring::persistence::SourceSnapshot> {
        self.render_source
            .as_ref()
            .or_else(|| self.scope.as_ref().map(|scope| &scope.source))
    }
    pub fn opening(&self) -> bool {
        self.pending.is_some()
    }
    #[cfg(debug_assertions)]
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({"active": self.active, "pending": self.pending, "read_only": false,
            "editable_components":["scene.transform","scene.primitive","scene.surface","scene.sun","scene.camera","scene.model","scene.environment","ui.caption","ui.layout","ui.classes","ui.image","ui.stylesheet","ui.background","ui.border","ui.shadow","ui.z","ui.global_z","ui.outline","ui.spin","ui.editable","ui.pickable","ui.modal","ui.id","ui.screen"],
            "collection_structural_editing":true,"resource_assignment":true,
            "save_available":self.can_save(), "saving":self.save_busy(), "save_pending":self.save_pending(), "save_error":self.save.error, "can_undo":self.can_history(false), "can_redo":self.can_history(true),
            "documents": self.opened.values().map(|d| serde_json::json!({"document":d.document, "path":d.path, "view":d.view, "selection":d.selected,
                "dirty":self.scope.as_ref().is_some_and(|scope| scope.working.dirty(d.document))})).collect::<Vec<_>>()})
    }
    pub fn request(&mut self, world: &World, root: PathBuf, path: PathBuf) -> Result<(), String> {
        if self.creating() {
            return Err("Finish scene creation before opening another source.".into());
        }
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
            self.ensure_source(world, root);
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
                self.task = Some(
                    Wakeup::for_world(world).spawn(IoTaskPool::get(), async move {
                        let result = if identity.is_some() {
                            source_files::check_inventory(&root, &path, &hash, identity)
                        } else {
                            source_files::check(&root, &path, &hash)
                        };
                        (clock, result.map(|_| None))
                    }),
                );
            }
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Option<Ready> {
        if let Some((clock, result)) = self.task.as_mut().and_then(Task::poll) {
            self.task = None;
            // The inventory belongs to this coordinator/project, not the requested
            // tab. Only the most recent locator can activate after it is admitted.
            match result {
                Ok(Some((scope, compiled))) => {
                    self.render_source = Some(scope.source.clone());
                    self.paths = scope
                        .working
                        .documents()
                        .keys()
                        .filter_map(|id| scope.path(*id).map(|path| (PathBuf::from(path), *id)))
                        .collect();
                    self.compiled_revisions = scope
                        .working
                        .documents()
                        .iter()
                        .map(|(id, document)| (*id, document.revision))
                        .collect();
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
                    self.pending_selection = None;
                }
                _ => {}
            }
        }
        if self.scope.is_none() || self.checked != self.pending {
            return None;
        }
        if let Some(path) = self.pending_selection.take() {
            self.select_asset_path(&path);
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
                "{}\n\n{}\n{} source nodes · {} nested instances\n\nUse Hierarchy to navigate composed nodes and Inspector to inspect selected values. Transform can be edited in memory with source Undo/Redo. Scene View requires a matching verified delivery and supported source data. Save active document writes accepted source changes; unresolved drafts must be accepted or cancelled first.",
                self.scope.as_ref()?.path(id)?,
                id.0,
                doc.nodes.len(),
                doc.instances.len()
            ),
            NodeAddress::from(doc.root),
        ))
    }
    pub fn published(&mut self, ready: Ready, view: ViewId, selected: NodeAddress) {
        let follow =
            self.active != Some(ready.document) || !self.opened.contains_key(&ready.document);
        self.opened
            .retain(|id, opened| *id == ready.document || opened.view != view);
        self.opened.entry(ready.document).or_insert(Opened {
            #[cfg(debug_assertions)]
            path: ready.path,
            document: ready.document,
            view,
            selected: Some(selected),
            graph: ready.graph,
        });
        self.active = Some(ready.document);
        if follow {
            self.follow_active_selection();
        }
    }
    pub fn activate_view(&mut self, view: ViewId) {
        if let Some(opened) = self.opened.values().find(|opened| opened.view == view)
            && self.active != Some(opened.document)
        {
            self.active = Some(opened.document);
            self.follow_active_selection();
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
        if !self.set_node_selection(document, node.clone(), false) {
            return false;
        }
        if let Some(opened) = self.opened.get_mut(&document) {
            opened.selected = Some(node);
        }
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
        self.selection.clear();
        true
    }
}

#[cfg(test)]
mod tests;
