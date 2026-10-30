//! Typed editor drag lifecycle over ordinary Bevy pointer events. Producers own
//! their UI binding proof; receivers revalidate source membership at drop time.
use bevy::{
    camera::NormalizedRenderTarget,
    input::keyboard::{Key, KeyboardInput},
    input_focus::FocusedInput,
    picking::events::{PointerDragDrop, PointerDragEnd, PointerDragStart},
    picking::pointer::{PointerAction, PointerId, PointerInput},
    prelude::*,
};
pub use kedra_editor_context::DragPayload;
use kedra_editor_context::SelectionItem;
mod feedback;
#[cfg(test)]
mod tests;
#[derive(Component)]
struct Dragged {
    payload: DragPayload,
    pointer: Option<PointerId>,
    browser: bool,
    node_origin: Option<(Entity, crate::hierarchy::Binding)>,
}
/// Inspector root accepting context drops, not source mutations.
#[derive(Component)]
pub(crate) struct ContextTarget(pub kedra_docking::model::ViewId);
pub(crate) fn install(app: &mut App) {
    app.add_message::<PointerInput>()
        .add_systems(Update, pointer_cancel)
        .add_systems(PostUpdate, feedback::update)
        .add_observer(node_start)
        .add_observer(end)
        .add_observer(cancel)
        .add_observer(context_drop);
}
pub(crate) fn active(world: &mut World) -> Option<(Entity, DragPayload)> {
    let owners: Vec<_> = world
        .query_filtered::<Entity, With<Dragged>>()
        .iter(world)
        .collect();
    owners
        .into_iter()
        .find_map(|owner| payload(world, owner).map(|payload| (owner, payload)))
}
/// Begin a native tool drag with already admitted logical references. This captures
/// no authoring grant; retired UI owners and unavailable payloads refuse atomically.
pub fn begin(world: &mut World, owner: Entity, payload: DragPayload) -> Result<(), String> {
    if !world.entities().contains(owner)
        || !crate::context::library(world).is_some_and(|library| payload.current(&library))
    {
        return Err("Drag owner or source payload is unavailable".into());
    }
    world.entity_mut(owner).insert(Dragged {
        payload,
        pointer: None,
        browser: false,
        node_origin: None,
    });
    Ok(())
}
/// Read a current typed payload. Stale Browser/Hierarchy bindings and retired source
/// scopes return None even if the original pointer entity still exists.
pub fn payload(world: &World, owner: Entity) -> Option<DragPayload> {
    let dragged = world.get::<Dragged>(owner)?;
    if dragged.browser && crate::asset_browser::path_for_drop(world, owner).is_none() {
        return None;
    }
    if let Some((root, original)) = &dragged.node_origin {
        let current = world.get::<crate::hierarchy::Binding>(*root)?;
        if current.view != original.view
            || current.document != original.document
            || !current.document.is_some_and(|document| {
                crate::workspace::node_drag_available(world, current.view, document)
            })
            || !matches!((&current.action, &original.action), (crate::hierarchy::Action::Select(a), crate::hierarchy::Action::Select(b)) if a == b)
        {
            return None;
        }
    }
    crate::context::library(world)
        .filter(|library| dragged.payload.current(library))
        .map(|_| dragged.payload.clone())
}
pub(crate) fn begin_browser(world: &mut World, owner: Entity, path: &str, pointer: PointerId) {
    let Some(asset) = crate::context::asset(world, path) else {
        return;
    };
    let item = SelectionItem::Asset(asset.clone());
    let selected =
        crate::context::selection(world).filter(|selection| selection.items().contains(&item));
    let payload = match selected {
        Some(selection) => match DragPayload::capture(selection) {
            Ok(payload) => payload,
            Err(_) => return,
        },
        None => DragPayload::Assets(vec![asset]),
    };
    if begin(world, owner, payload).is_ok()
        && let Some(mut dragged) = world.get_mut::<Dragged>(owner)
    {
        dragged.browser = true;
        dragged.pointer = Some(pointer);
    }
}
fn node_start(
    event: On<PointerDragStart>,
    bindings: Query<&crate::hierarchy::Binding>,
    mut commands: Commands,
) {
    if event.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    let Ok(binding) = bindings.get(event.event_target()) else {
        return;
    };
    let crate::hierarchy::Action::Select(node) = &binding.action else {
        return;
    };
    let Some(document) = binding.document else {
        return;
    };
    let original = binding.clone();
    let node = node.clone();
    let root = event.event_target();
    let owner = event.original_event_target();
    let pointer = event.pointer.id;
    commands.queue(move |world: &mut World| {
        if !crate::workspace::node_drag_available(world, original.view, document) {
            return;
        }
        let Some(library) = crate::context::library(world) else {
            return;
        };
        let Some(node) = library.node(document, node) else {
            return;
        };
        let item = SelectionItem::Node(node.clone());
        let selected =
            crate::context::selection(world).filter(|selection| selection.items().contains(&item));
        let payload = match selected {
            Some(selection) => match DragPayload::capture(selection) {
                Ok(payload) => payload,
                Err(_) => return,
            },
            None => DragPayload::Nodes(vec![node]),
        };
        if begin(world, owner, payload).is_ok()
            && let Some(mut dragged) = world.get_mut::<Dragged>(owner)
        {
            dragged.node_origin = Some((root, original));
            dragged.pointer = Some(pointer);
        }
    });
}
fn end(event: On<PointerDragEnd>, mut commands: Commands) {
    let owner = event.original_event_target();
    commands.queue(move |world: &mut World| {
        if let Ok(mut entity) = world.get_entity_mut(owner) {
            entity.remove::<Dragged>();
        }
    });
}
fn cancel(event: On<FocusedInput<KeyboardInput>>, mut commands: Commands) {
    if event.input.logical_key != Key::Escape || !event.input.state.is_pressed() {
        return;
    }
    let window = event.input.window;
    commands.queue(move |world: &mut World| clear_window(world, window, None));
}
// PointerCancel only reaches hovered entities, and Bevy sends no DragEnd after it.
// Read its underlying input so cancellation outside a hit target also revokes proof.
fn pointer_cancel(
    world: &mut World,
    mut cursor: Local<bevy::ecs::message::MessageCursor<PointerInput>>,
) {
    let cancelled: Vec<_> = cursor
        .read(world.resource::<Messages<PointerInput>>())
        .filter_map(|input| match (&input.action, &input.location.target) {
            (PointerAction::Cancel, NormalizedRenderTarget::Window(window)) => {
                Some((window.entity(), input.pointer_id))
            }
            _ => None,
        })
        .collect();
    for (window, pointer) in cancelled {
        clear_window(world, window, Some(pointer));
    }
}
fn clear_window(world: &mut World, window: Entity, pointer: Option<PointerId>) {
    let owners: Vec<_> = world
        .query::<(Entity, &Dragged)>()
        .iter(world)
        .filter(|(owner, dragged)| {
            kedra_ui::native_input::owner(world, *owner) == Some(window)
                && pointer
                    .is_none_or(|pointer| dragged.pointer.is_none_or(|origin| origin == pointer))
        })
        .map(|(owner, _)| owner)
        .collect();
    for owner in owners {
        world.entity_mut(owner).remove::<Dragged>();
    }
}
fn context_drop(
    mut event: On<PointerDragDrop>,
    targets: Query<&ContextTarget>,
    mut commands: Commands,
) {
    let Ok(target) = targets.get(event.event_target()) else {
        return;
    };
    let view = target.0;
    let owner = event.dropped;
    event.propagate(false);
    commands.queue(move |world: &mut World| {
        let Some(payload) = payload(world, owner) else {
            return;
        };
        let items = match payload {
            DragPayload::Assets(assets) => assets.into_iter().map(SelectionItem::Asset).collect(),
            DragPayload::Nodes(nodes) => nodes.into_iter().map(SelectionItem::Node).collect(),
        };
        crate::workspace::drop_context(world, view, items);
    });
}
