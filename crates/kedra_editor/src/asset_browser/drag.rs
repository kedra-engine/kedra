//! Read-only drag proof from a current tracked browser entry. Capturing it never
//! imports, moves or writes an asset, and it is not resource-assignment authority.
use super::*;
use bevy::{
    camera::NormalizedRenderTarget,
    input::keyboard::{Key, KeyboardInput},
    input_focus::FocusedInput,
    picking::events::{
        PointerDragDrop, PointerDragEnd, PointerDragEnter, PointerDragLeave, PointerDragStart,
    },
    picking::pointer::{PointerAction, PointerId, PointerInput},
};
#[derive(Component, Clone)]
struct Capture {
    root: Entity,
    binding: Binding,
    pointer: PointerId,
}
pub(super) fn install(app: &mut App) {
    app.add_message::<PointerInput>()
        .add_systems(Update, pointer_cancel)
        .add_observer(start)
        .add_observer(end)
        .add_observer(drop_entry)
        .add_observer(enter)
        .add_observer(leave)
        .add_observer(cancel);
}
pub(crate) fn path_for_drop(world: &World, dragged: Entity) -> Option<String> {
    let capture = world.get::<Capture>(dragged)?;
    let current = world.get::<Binding>(capture.root)?;
    let Action::Select(name) = &capture.binding.action else {
        return None;
    };
    if current.view != capture.binding.view
        || current.path != capture.binding.path
        || current.generation != capture.binding.generation
        || !matches!(&current.action, Action::Select(current) if current == name)
    {
        return None;
    }
    crate::workspace::browser_drag_path(
        world,
        current.view,
        &current.path,
        current.generation,
        name,
    )
}
fn start(event: On<PointerDragStart>, bindings: Query<&Binding>, mut commands: Commands) {
    if event.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    let Ok(binding) = bindings.get(event.event_target()) else {
        return;
    };
    if !matches!(binding.action, Action::Select(_) | Action::TreeNavigate(_)) {
        return;
    }
    let capture = Capture {
        root: event.event_target(),
        binding: binding.clone(),
        pointer: event.pointer.id,
    };
    // The entry binding is found while bubbling through its row, but Bevy's
    // later drop still names the original picked child (for example its label).
    let dragged = event.original_event_target();
    commands.queue(move |world: &mut World| {
        if world.entities().contains(dragged) {
            world.entity_mut(dragged).insert(capture);
            if let Some(path) = path_for_drop(world, dragged) {
                let pointer = world.get::<Capture>(dragged).map(|capture| capture.pointer);
                if let Some(pointer) = pointer {
                    crate::drag::begin_browser(world, dragged, &path, pointer);
                }
            }
        }
    });
}
fn entry(world: &World, binding: &Binding, folder: bool) -> Option<PathBuf> {
    crate::workspace::browser_entry_path(
        world,
        binding.view,
        &binding.path,
        binding.generation,
        &binding.action,
        folder,
    )
}
fn moving(world: &World, dragged: Entity) -> Option<(ViewId, PathBuf)> {
    let capture = world.get::<Capture>(dragged)?;
    let current = world.get::<Binding>(capture.root)?;
    if current.view != capture.binding.view
        || current.path != capture.binding.path
        || current.generation != capture.binding.generation
    {
        return None;
    }
    let path = entry(world, current, false)?;
    if path.as_os_str().is_empty() || entry(world, &capture.binding, false)? != path {
        return None;
    }
    Some((current.view, path))
}
fn drop_entry(mut event: On<PointerDragDrop>, bindings: Query<&Binding>, mut commands: Commands) {
    let Ok(binding) = bindings.get(event.event_target()) else {
        return;
    };
    let binding = binding.clone();
    let dragged = event.dropped;
    event.propagate(false);
    commands.queue(move |world: &mut World| {
        let Some((source_view, from)) = moving(world, dragged) else {
            return;
        };
        let Some(directory) = entry(world, &binding, true) else {
            return;
        };
        crate::workspace::folders::move_entry(world, source_view, binding.view, from, directory);
    });
}
fn enter(event: On<PointerDragEnter>, bindings: Query<&Binding>, mut commands: Commands) {
    let Ok(binding) = bindings.get(event.event_target()).cloned() else {
        return;
    };
    let target = event.event_target();
    let dragged = event.dragged;
    commands.queue(move |world: &mut World| {
        let valid = moving(world, dragged)
            .zip(entry(world, &binding, true))
            .is_some_and(|((_, from), directory)| {
                !directory.starts_with(&from) && from.parent() != Some(directory.as_path())
            });
        kedra_ui::docking::class(world, target, "drop-target", valid);
    });
}
fn leave(event: On<PointerDragLeave>, bindings: Query<&Binding>, mut commands: Commands) {
    if bindings.get(event.event_target()).is_err() {
        return;
    }
    let target = event.event_target();
    commands.queue(move |world: &mut World| {
        kedra_ui::docking::class(world, target, "drop-target", false)
    });
}
fn end(event: On<PointerDragEnd>, mut commands: Commands) {
    let entity = event.original_event_target();
    commands.queue(move |world: &mut World| {
        if let Ok(mut entity) = world.get_entity_mut(entity) {
            entity.remove::<Capture>();
        }
    });
}
#[cfg(test)]
mod tests;
fn cancel(event: On<FocusedInput<KeyboardInput>>, mut commands: Commands) {
    if event.input.logical_key != Key::Escape || !event.input.state.is_pressed() {
        return;
    }
    let window = event.input.window;
    commands.queue(move |world: &mut World| clear_window(world, window, None));
}
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
    let captures: Vec<_> = world
        .query::<(Entity, &Capture)>()
        .iter(world)
        .filter(|(entity, capture)| {
            kedra_ui::native_input::owner(world, *entity) == Some(window)
                && pointer.is_none_or(|pointer| capture.pointer == pointer)
        })
        .map(|(entity, _)| entity)
        .collect();
    for entity in captures {
        world.entity_mut(entity).remove::<Capture>();
    }
}
