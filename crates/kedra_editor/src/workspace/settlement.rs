//! Host-owned draft admission shared by source history, Save, scripts and Close.
//! Tool adapters retain their own drafts. Add future tools here, not in each action
//! and not in a second source transaction/history manager.
use super::*;
pub(super) fn drafts(world: &World, state: &EditorWorkspace) -> bool {
    state
        .panels
        .inspectors
        .values()
        .any(|panel| panel.has_drafts(world))
}
pub(super) fn source_edits(world: &World, state: &EditorWorkspace) -> bool {
    state.pending.is_some() || state.gesture.is_some() || drafts(world, state)
}
