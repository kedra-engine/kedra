//! Host-owned draft admission shared by source history, Save, scripts and Close.
//! Tool adapters retain their own drafts. Add future tools here, not in each action
//! and not in a second source transaction/history manager.
use super::*;
pub(super) fn drafts(world: &World, state: &EditorWorkspace) -> bool {
    super::manipulation::active(world)
        || state
            .panels
            .inspectors
            .values()
            .any(|panel| panel.has_drafts(world))
}
pub(super) fn source_edits(world: &World, state: &EditorWorkspace) -> bool {
    super::asset_import::pending(world)
        || super::folders::pending(world)
        || state.pending.is_some()
        || state.gesture.is_some()
        || drafts(world, state)
}
