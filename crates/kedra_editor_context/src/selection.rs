use super::{AssetLibrary, AssetRef, NodeRef, ScopeId};
use serde::Serialize;
/// One logical selection item. Heterogeneous selections retain their explicit types.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum SelectionItem {
    /// A source asset/folder; selection never implies document Open.
    Asset(AssetRef),
    /// A node in an existing receiving document.
    Node(NodeRef),
}
impl SelectionItem {
    fn scope(&self) -> ScopeId {
        match self {
            Self::Asset(asset) => asset.scope(),
            Self::Node(node) => node.scope(),
        }
    }
    fn current(&self, library: &AssetLibrary<'_>) -> bool {
        match self {
            Self::Asset(asset) => library.contains_asset(asset),
            Self::Node(node) => library.contains_node(node),
        }
    }
}
/// Shared selected set and active item. Windows follow or pin a snapshot locally;
/// neither operation changes active documents, sources, Save or source history.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Selection {
    items: Vec<SelectionItem>,
    active: Option<usize>,
    revision: u64,
}
impl Selection {
    /// Selected items in user order, without duplicate references.
    pub fn items(&self) -> &[SelectionItem] {
        &self.items
    }
    /// Active item is always a member of the selected set, or None when empty.
    pub fn active(&self) -> Option<&SelectionItem> {
        self.active.and_then(|index| self.items.get(index))
    }
    /// Monotonic context revision for retained consumers, not an authoring revision.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Clear shared selection without altering any sources or document tabs.
    pub fn clear(&mut self) {
        if !self.items.is_empty() {
            self.items.clear();
            self.active = None;
            self.revision = self.revision.saturating_add(1);
        }
    }
    /// Replace with a deduplicated current set, making its final item active.
    /// Missing/foreign items or more than 4096 entries refuse the entire request.
    pub fn replace(
        &mut self,
        items: Vec<SelectionItem>,
        library: &AssetLibrary<'_>,
    ) -> Result<(), String> {
        if items.len() > 4096 || items.iter().any(|item| !item.current(library)) {
            return Err("Selection contains an unavailable or foreign source item".into());
        }
        let mut unique = Vec::new();
        for item in items {
            if !unique.contains(&item) {
                unique.push(item);
            }
        }
        let active = unique.len().checked_sub(1);
        if self.items != unique || self.active != active {
            self.items = unique;
            self.active = active;
            self.revision = self.revision.saturating_add(1);
        }
        Ok(())
    }
    /// Toggle one admitted item for additive selection; removing the active item
    /// activates the final survivor. Refusal preserves the selected set and revision.
    pub fn toggle(
        &mut self,
        item: SelectionItem,
        library: &AssetLibrary<'_>,
    ) -> Result<(), String> {
        if !item.current(library) {
            return Err("Selection item is unavailable or foreign".into());
        }
        let mut items = self.items.clone();
        if let Some(index) = items.iter().position(|value| *value == item) {
            items.remove(index);
        } else {
            items.push(item);
        }
        self.replace(items, library)
    }
    /// Retire references no longer present after source changes. This is context
    /// reconciliation, never an implicit Undo, Open or filesystem operation.
    pub fn reconcile(&mut self, library: &AssetLibrary<'_>) {
        let retained = self
            .items
            .iter()
            .filter(|item| item.current(library))
            .cloned()
            .collect();
        let _ = self.replace(retained, library);
    }
}
/// Typed immutable drag payload. Receivers revalidate scope/membership and authorize
/// the requested operation separately; no payload is permission to mutate sources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragPayload {
    /// Existing source assets, for reference fields, previews or other asset consumers.
    Assets(Vec<AssetRef>),
    /// Source nodes, for hierarchy/scene consumers; never runtime Entity handles.
    Nodes(Vec<NodeRef>),
}
impl DragPayload {
    /// Capture one homogeneous selection. Mixed/empty selections refuse rather
    /// than silently discarding items. Foreign source scopes also refuse.
    pub fn capture(selection: &Selection) -> Result<Self, String> {
        let first = selection.active().ok_or("Nothing is selected for drag")?;
        if selection
            .items
            .iter()
            .any(|item| item.scope() != first.scope())
        {
            return Err("Drag spans foreign source scopes".into());
        }
        match first {
            SelectionItem::Asset(_) => selection
                .items
                .iter()
                .map(|item| match item {
                    SelectionItem::Asset(asset) => Ok(asset.clone()),
                    _ => Err("Mixed assets/nodes cannot form one drag".into()),
                })
                .collect::<Result<Vec<_>, String>>()
                .map(Self::Assets),
            SelectionItem::Node(_) => selection
                .items
                .iter()
                .map(|item| match item {
                    SelectionItem::Node(node) => Ok(node.clone()),
                    _ => Err("Mixed assets/nodes cannot form one drag".into()),
                })
                .collect::<Result<Vec<_>, String>>()
                .map(Self::Nodes),
        }
    }
    /// Revalidate all payload members against current source authority at drop time.
    pub fn current(&self, library: &AssetLibrary<'_>) -> bool {
        match self {
            Self::Assets(assets) => {
                !assets.is_empty() && assets.iter().all(|asset| library.contains_asset(asset))
            }
            Self::Nodes(nodes) => {
                !nodes.is_empty() && nodes.iter().all(|node| library.contains_node(node))
            }
        }
    }
}
