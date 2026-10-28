//! Shared project editor context for native windows/extensions. Selection changes
//! never grant authoring permission, activate a tab or perform filesystem effects.
use bevy::prelude::*;
pub use kedra_editor_context::{
    AssetKind, AssetLibrary, AssetRef, NodeRef, Selection, SelectionItem,
};
/// Borrow the existing admitted project inventory/catalog, without creating an index.
pub fn library(world: &World) -> Option<AssetLibrary<'_>> {
    world
        .get_resource::<crate::workspace::EditorWorkspace>()?
        .documents
        .library()
}
/// Observe shared selection. Pinned windows keep their own immutable snapshot.
pub fn selection(world: &World) -> Option<&Selection> {
    Some(
        &world
            .get_resource::<crate::workspace::EditorWorkspace>()?
            .documents
            .selection,
    )
}
/// Resolve one exact source asset; unadmitted or foreign paths return None.
pub fn asset(world: &World, path: &str) -> Option<AssetRef> {
    library(world)?.asset(path)
}
/// Replace shared selection after validating every item against the current source
/// lifetime. Unsettled editor input refuses without losing drafts or changing sources.
pub fn select(world: &mut World, items: Vec<SelectionItem>) -> Result<(), String> {
    crate::workspace::select_context(world, items)
}
