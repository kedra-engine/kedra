//! Native form commands wait for Bevy's real text-edit milestone. The synchronous
//! semantic API still uses `dispatch`, refusing unsettled edits rather than taking
//! a stale draft. Neither path changes the domain generation until preflight wins.
use super::*;
use bevy::text::EditableText;

#[derive(Resource, Default)]
pub(crate) struct Pending(Option<Request>);
pub(super) fn owner(manager: &ProjectManager) -> Option<(Entity, u64)> {
    manager.modal.map(|modal| (modal, manager.generation))
}
struct Request {
    action: Action,
    source: Option<Entity>,
    modal: Entity,
    generation: u64,
    key_command: bool,
}

pub(crate) fn request(
    world: &mut World,
    action: Action,
    source: Option<Entity>,
    owner: Option<(Entity, u64)>,
) {
    if !matches!(action, Action::Submit | Action::Cancel | Action::Browse) {
        dispatch(world, action, source);
        return;
    }
    queue(world, action, source, false, owner);
}
pub(super) fn key(world: &mut World, action: Action, source: Entity, owner: Option<(Entity, u64)>) {
    queue(world, action, Some(source), true, owner);
}
fn queue(
    world: &mut World,
    action: Action,
    source: Option<Entity>,
    key_command: bool,
    expected: Option<(Entity, u64)>,
) {
    let manager = world.resource::<ProjectManager>();
    let Some((modal, generation)) = expected else {
        return;
    };
    if owner(manager) != expected
        || manager.phase() != Phase::Projects
        || world.contains_resource::<jobs::PendingJob>()
    {
        return;
    }
    world.init_resource::<Pending>();
    let mut pending = world.resource_mut::<Pending>();
    // A deliberate cancel may supersede a submit waiting for clipboard I/O.
    if pending.0.is_none() || matches!(action, Action::Cancel) {
        pending.0 = Some(Request {
            action,
            source,
            modal,
            generation,
            key_command,
        });
    }
}
fn composing(world: &mut World) -> bool {
    world
        .query::<(&Field, &EditableText)>()
        .iter(world)
        .any(|(field, text)| matches!(field, Field::Name | Field::Directory) && text.is_composing())
}
fn editing(world: &mut World) -> bool {
    world
        .query::<(&Field, &EditableText)>()
        .iter(world)
        .any(|(field, text)| {
            matches!(field, Field::Name | Field::Directory)
                && (!text.pending_edits.is_empty() || text.pending_paste.is_some())
        })
}
pub(super) fn unsettled(world: &mut World) -> bool {
    composing(world) || editing(world)
}

pub(crate) fn apply(world: &mut World) {
    let Some(request) = world
        .get_resource_mut::<Pending>()
        .and_then(|mut p| p.0.take())
    else {
        return;
    };
    let manager = world.resource::<ProjectManager>();
    if manager.phase() != Phase::Projects
        || manager.generation != request.generation
        || manager.modal != Some(request.modal)
        || !world.entities().contains(request.modal)
        || request
            .source
            .is_some_and(|source| !world.entities().contains(source))
    {
        return;
    }
    // Also reject a Preedit queued in the same batch as Enter/Escape: at input
    // dispatch time the editor might not have entered composition yet.
    if composing(world) && (request.key_command || !matches!(request.action, Action::Cancel)) {
        return;
    }
    if !matches!(request.action, Action::Cancel) && editing(world) {
        world.resource_mut::<Pending>().0 = Some(request);
        return;
    }
    dispatch(world, request.action, request.source);
}
