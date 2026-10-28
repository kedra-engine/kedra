//! Retained collection presentation. Fields use ordinary source-neutral bindings;
//! structural actions propose complete native lists, never optimistic local drafts.
use crate::controls::{Control, ControlEvent};
use bevy::prelude::*;
use kedra_ui::fields::{FieldAccess, TextField};
use std::sync::Arc;
mod input;
mod interaction;
pub mod presentation;
mod value;
mod view;
pub(crate) use view::{CollectionEditor, Kind};

/// Retained selection, native readiness or a structural proposal changed. Hosts
/// refresh presentation; this event never grants permission to publish a source.
#[derive(Event)]
pub struct Changed;

#[derive(Component)]
struct State {
    value: Box<dyn PartialReflect>,
    revision: u64,
    ids: Vec<u64>,
    next_id: u64,
    selected: Option<u64>,
    pending: Option<Proposal>,
    diagnostic: Entity,
    area: Entity,
    queued: usize,
    classes: bool,
    drag: Option<interaction::Drag>,
    focus: Option<interaction::Focus>,
    focus_ready: bool,
    inserted_image: bool,
    expanded: std::collections::BTreeSet<u64>,
    restoration: Option<presentation::Presentation>,
}
#[derive(Component)]
struct ImageInsertion(Entity);

/// Presentation acknowledgment after a host publishes the first required image.
/// This selects/focuses the new item on source refresh; it cannot construct a
/// resource, acknowledge a pending load or publish source/history itself.
pub fn resource_inserted(world: &mut World, endpoint: Entity) {
    if let Some(owner) = world.get::<ImageInsertion>(endpoint).map(|marker| marker.0)
        && let Some(mut state) = world.get_mut::<State>(owner)
    {
        state.inserted_image = true;
    }
}
struct Proposal {
    before: presentation::Presentation,
    binding: Arc<crate::controls::ControlBinding>,
    value: Arc<dyn PartialReflect>,
    ids: Vec<u64>,
    selected: Option<u64>,
    focus: Option<interaction::Focus>,
}
#[derive(Component, Clone)]
struct Action {
    owner: Entity,
    revision: u64,
    kind: Operation,
}
#[derive(Clone, Copy)]
enum Operation {
    Add,
    Remove(usize),
    Move(usize, usize),
    Select(u64),
    Expand(u64),
}

fn within(world: &World, mut entity: Entity, root: Entity) -> bool {
    loop {
        if entity == root {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
}
/// Whether queued/pending structural proposals or native item text need settlement.
/// `except` excludes only that array's pending complete proposal, never its queued
/// actions or unaccepted text. Hidden native drafts remain unsettled.
pub fn unsettled(world: &World, root: Entity, except: Option<Entity>) -> bool {
    world.iter_entities().any(|entity| {
        within(world, entity.id(), root)
            && (entity.get::<State>().is_some_and(|state| {
                state.queued > 0
                    || state.drag.is_some()
                    || (Some(entity.id()) != except && state.pending.is_some())
            }) || (entity.contains::<TextField>() && TextField::unsettled(world, entity.id())))
    })
}
/// Original array endpoint for an outstanding complete structural proposal.
/// Fields and choices inside items use their ordinary full native source paths.
pub fn proposal_owner(world: &World, entity: Entity) -> Option<Entity> {
    world.get::<State>(entity)?.pending.as_ref().map(|_| entity)
}
/// Settle the exact host-validated structural proposal after synchronous source
/// publication. Rejection keeps source presentation, item identities and selection.
/// Replaced bindings and unrelated values cannot settle another pending request.
pub fn respond(world: &mut World, event: &ControlEvent, accepted: bool) {
    if world
        .get::<Control>(event.entity)
        .is_none_or(|control| !Arc::ptr_eq(&control.0, &event.binding))
    {
        return;
    }
    let Some(mut state) = world.get_mut::<State>(event.entity) else {
        return;
    };
    let Some(pending) = &state.pending else {
        return;
    };
    if !Arc::ptr_eq(&pending.binding, &event.binding) || !Arc::ptr_eq(&pending.value, &event.value)
    {
        return;
    }
    let Some(pending) = state.pending.take() else {
        return;
    };
    if accepted {
        state.ids = pending.ids;
        state.selected = pending.selected;
        state.focus = pending.focus;
        state.focus_ready = false;
        if let Some(interaction::Focus::Item(id)) = pending.focus {
            state.expanded.insert(id);
        }
    }
    world.trigger(Changed);
}
pub(crate) fn install(app: &mut App) {
    app.add_message::<input::ActionRequest>()
        .add_observer(input::action);
    interaction::install(app);
    app.add_systems(
        PostUpdate,
        input::actions.after(bevy::text::EditableTextSystems),
    );
}
fn feedback(world: &mut World, root: Entity, error: Option<String>) {
    let Some(diagnostic) = world.get::<State>(root).map(|state| state.diagnostic) else {
        return;
    };
    if let Some(mut text) = world.get_mut::<Text>(diagnostic) {
        text.0 = error.clone().unwrap_or_default();
    }
    kedra_ui::docking::class(world, diagnostic, "hidden", error.is_none());
}
fn writable(world: &World, entity: Entity) -> bool {
    world
        .get::<FieldAccess>(entity)
        .is_some_and(|access| !access.is_read_only())
        && world.get::<bevy::ui::InteractionDisabled>(entity).is_none()
}
