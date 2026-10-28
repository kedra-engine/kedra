//! Host integration for shared asset library/selection. Document Open and active
//! tabs remain separate; per-document `selected` is remembered view context only.
use super::*;
use kedra_editor_context::{AssetLibrary, Selection, SelectionItem};
impl Documents {
    pub(crate) fn replace_selection(&mut self, items: Vec<SelectionItem>) -> Result<(), String> {
        let scope = self.scope.as_ref().ok_or("Source library is unavailable")?;
        let library = AssetLibrary::new(self.scope_identity, &scope.source, &scope.working);
        self.selection.replace(items, &library)
    }
    pub(crate) fn library(&self) -> Option<AssetLibrary<'_>> {
        let scope = self.scope.as_ref()?;
        Some(AssetLibrary::new(
            self.scope_identity,
            &scope.source,
            &scope.working,
        ))
    }
    pub(crate) fn select_asset_path(&mut self, path: &str) -> bool {
        let Some(scope) = self.scope.as_ref() else {
            return false;
        };
        let library = AssetLibrary::new(self.scope_identity, &scope.source, &scope.working);
        let Some(asset) = library.asset(path) else {
            return false;
        };
        self.selection
            .replace(vec![SelectionItem::Asset(asset)], &library)
            .is_ok()
    }
    pub(crate) fn select_asset(&mut self, world: &World, root: PathBuf, path: String) {
        if self.scope.is_some() {
            self.select_asset_path(&path);
            return;
        }
        self.pending_selection = Some(path);
        if self.task.is_none() {
            let clock = self.clock;
            let registry = world.resource::<AppTypeRegistry>().clone();
            let policies = world.resource::<ComponentPolicies>().clone();
            self.task = Some(IoTaskPool::get().spawn(async move {
                let result = source_files::read(&root, "game")
                    .and_then(|snapshot| source::admit(snapshot, &registry.read(), &policies))
                    .map(Some);
                (clock, result)
            }));
        }
    }
    pub(crate) fn set_node_selection(
        &mut self,
        document: DocumentId,
        node: NodeAddress,
        additive: bool,
    ) -> bool {
        let Some(scope) = self.scope.as_ref() else {
            return false;
        };
        let library = AssetLibrary::new(self.scope_identity, &scope.source, &scope.working);
        let Some(node) = library.node(document, node) else {
            return false;
        };
        let item = SelectionItem::Node(node);
        if additive {
            self.selection.toggle(item, &library).is_ok()
        } else {
            self.selection.replace(vec![item], &library).is_ok()
        }
    }
    pub(crate) fn follow_active_selection(&mut self) {
        let selected = self
            .active
            .and_then(|id| self.opened.get(&id))
            .and_then(|doc| doc.selected.clone().map(|node| (doc.document, node)));
        if let Some((document, node)) = selected {
            if !self.set_node_selection(document, node, false) {
                self.selection.clear();
            }
        } else {
            self.selection.clear();
        }
    }
    pub(crate) fn selection_target(&self) -> Option<inspection::Target> {
        let library = self.library()?;
        let node = match self.selection.active()? {
            SelectionItem::Node(node) if library.contains_node(node) => node.clone(),
            SelectionItem::Asset(asset) => library.root(asset)?,
            _ => return None,
        };
        self.target(node.document, node.node)
    }
    pub(crate) fn selected_nodes(&self, document: DocumentId) -> Vec<NodeAddress> {
        self.selection
            .items()
            .iter()
            .filter_map(|item| match item {
                SelectionItem::Node(node) if node.document == document => Some(node.node.clone()),
                _ => None,
            })
            .collect()
    }
    pub(crate) fn select_nodes(&mut self, document: DocumentId, nodes: Vec<NodeAddress>) -> bool {
        if self.active != Some(document) {
            return false;
        }
        let Some(scope) = self.scope.as_ref() else {
            return false;
        };
        let library = AssetLibrary::new(self.scope_identity, &scope.source, &scope.working);
        let items = nodes
            .into_iter()
            .map(|node| library.node(document, node).map(SelectionItem::Node))
            .collect::<Option<Vec<_>>>();
        let Some(items) = items else {
            return false;
        };
        if self.selection.replace(items, &library).is_err() {
            return false;
        }
        self.remember_node_selection(document);
        true
    }
    pub(crate) fn toggle_node(&mut self, document: DocumentId, node: NodeAddress) -> bool {
        if self.active != Some(document) {
            return false;
        }
        if !self.set_node_selection(document, node, true) {
            return false;
        }
        self.remember_node_selection(document);
        true
    }
    fn remember_node_selection(&mut self, document: DocumentId) {
        if let Some(opened) = self.opened.get_mut(&document) {
            opened.selected = self.selection.active().and_then(|item| match item {
                SelectionItem::Node(node) if node.document == document => Some(node.node.clone()),
                _ => None,
            });
        }
    }
    pub(crate) fn restore_selection(&mut self, mut selection: Selection) {
        if let Some(library) = self.library() {
            selection.reconcile(&library);
        } else {
            selection.clear();
        }
        if let Some(scope) = &self.scope {
            let library = AssetLibrary::new(self.scope_identity, &scope.source, &scope.working);
            let _ = self.selection.replace(selection.items().to_vec(), &library);
        } else {
            self.selection.clear();
        }
    }
    pub(crate) fn reconcile_selection(&mut self) {
        if let Some(scope) = &self.scope {
            let library = AssetLibrary::new(self.scope_identity, &scope.source, &scope.working);
            self.selection.reconcile(&library);
        } else {
            self.selection.clear();
        }
    }
    /// Explicit Discard retires old capabilities, but surviving host context can
    /// be resolved anew. Missing nodes/assets are pruned, never replaced by a row.
    pub(super) fn renew_selection_after_discard(&mut self) {
        let Some(scope) = &self.scope else {
            self.selection.clear();
            return;
        };
        let library = AssetLibrary::new(self.scope_identity, &scope.source, &scope.working);
        let items = self
            .selection
            .items()
            .iter()
            .filter_map(|item| match item {
                SelectionItem::Asset(asset) => library
                    .asset(asset.path())
                    .filter(|current| {
                        current.identity() == asset.identity() && current.kind() == asset.kind()
                    })
                    .map(SelectionItem::Asset),
                SelectionItem::Node(node) => library
                    .node(node.document, node.node.clone())
                    .map(SelectionItem::Node),
            })
            .collect();
        let _ = self.selection.replace(items, &library);
    }
}
