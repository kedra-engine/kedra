//! Restored visible tabs use the same source admission as explicit Assets Open.
use super::*;

pub(super) fn poll(world: &mut World, state: &mut EditorWorkspace) {
    if state.documents.opening() || state.documents.creating() || state.documents.save_busy() {
        return;
    }
    let snapshot = state.layout.snapshot();
    state
        .panels
        .restore_sources
        .retain(|view, _| snapshot.views.contains_key(view));
    let visible = |view: &ViewId| {
        snapshot
            .nodes
            .values()
            .any(|node| matches!(node, Node::Group { active, .. } if active == view))
    };
    let next = state
        .content_view
        .filter(|view| state.panels.restore_sources.contains_key(view) && visible(view))
        .or_else(|| state.panels.restore_sources.keys().copied().find(visible));
    let Some(view) = next else {
        return;
    };
    let Some(root) = world
        .get_resource::<crate::model::ProjectManager>()
        .and_then(|manager| manager.project.as_ref())
        .map(|project| project.assets.clone())
    else {
        return;
    };
    let Some(path) = state.panels.restore_sources.remove(&view) else {
        return;
    };
    state.panels.restoring_source = Some(view);
    state.documents.origin = None;
    if let Err(error) = state.documents.request(world, root, path) {
        state.documents.error = Some(error);
    }
}
