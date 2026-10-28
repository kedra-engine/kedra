//! Optional editor attachments keyed by the real source-history identity.
//! No source values, entity handles, value matching or second Undo journal live here.
use super::*;
use kedra_authoring::workspace::HistoryId;
use kedra_editor_context::ScopeId;
use kedra_scene::document::{DocumentId, NodeAddress};

#[derive(Clone)]
struct Context {
    selection: kedra_editor_context::Selection,
    active: Option<DocumentId>,
    selected: BTreeMap<DocumentId, Option<NodeAddress>>,
    inspectors: BTreeMap<ViewId, crate::inspector::history::Context>,
}
struct Entry {
    before: Context,
    after: Context,
}
#[derive(Default)]
pub(super) struct History {
    scope: Option<ScopeId>,
    entries: BTreeMap<HistoryId, Entry>,
}
pub(super) struct Capture {
    scope: ScopeId,
    revisions: BTreeMap<DocumentId, u64>,
    context: Context,
}
fn context(world: &World, state: &EditorWorkspace, proposed: bool) -> Context {
    Context {
        selection: state.documents.selection.clone(),
        active: state.documents.active,
        selected: state
            .documents
            .opened
            .iter()
            .map(|(id, doc)| (*id, doc.selected.clone()))
            .collect(),
        inspectors: state
            .panels
            .inspectors
            .iter()
            .map(|(id, panel)| (*id, panel.history_context(world, *id, proposed)))
            .collect(),
    }
}
pub(super) fn capture(world: &World, state: &EditorWorkspace) -> Capture {
    Capture {
        scope: state.documents.edit_scope(),
        revisions: state.documents.revisions(),
        context: context(world, state, false),
    }
}
pub(super) fn accepted(world: &World, state: &mut EditorWorkspace, capture: Capture) {
    let scope = state.documents.edit_scope();
    if state.history_context.scope != Some(scope) {
        state.history_context.entries.clear();
        state.history_context.scope = Some(scope);
    }
    let retained = state.documents.history_ids();
    state
        .history_context
        .entries
        .retain(|id, _| retained.contains(id));
    if capture.scope != scope || capture.revisions == state.documents.revisions() {
        return;
    }
    let Some(id) = state.documents.history_id(false) else {
        return;
    };
    let after = context(world, state, true);
    if let Some(entry) = state.history_context.entries.get_mut(&id) {
        entry.after = after;
    } else {
        state.history_context.entries.insert(
            id,
            Entry {
                before: capture.context,
                after,
            },
        );
    }
}
pub(super) fn restore(world: &mut World, state: &mut EditorWorkspace, id: HistoryId, redo: bool) {
    if state.history_context.scope != Some(state.documents.edit_scope()) {
        return;
    }
    let Some(entry) = state.history_context.entries.get(&id) else {
        return;
    };
    let context = if redo {
        entry.after.clone()
    } else {
        entry.before.clone()
    };
    if let Some(view) = context
        .active
        .and_then(|id| state.documents.opened.get(&id).map(|doc| doc.view))
        && state.layout.snapshot().location(view).is_some()
    {
        // A closed/retired surface is optional: source history has already succeeded.
        let _ = super::apply(world, state, Command::Activate(view));
    }
    for (id, selected) in context.selected {
        if let Some(document) = state.documents.opened.get_mut(&id) {
            document.selected = selected.filter(|node| document.graph.nodes.contains_key(node));
        }
    }
    for (id, context) in context.inspectors {
        let focus = inspector_visible(state, id);
        if let Some(panel) = state.panels.inspectors.get_mut(&id) {
            panel.restore_history_context(context, focus);
        }
    }
    state.documents.restore_selection(context.selection);
}
