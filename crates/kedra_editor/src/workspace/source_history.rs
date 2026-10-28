//! Explicit source history never borrows the local text undo shortcut.
use super::*;
use bevy::{
    input::keyboard::{Key, KeyboardInput},
    input_focus::FocusedInput,
};
pub(super) fn install(app: &mut App) {
    app.add_observer(keyboard);
    app.add_systems(Update, close_guard.before(kedra_desktop::CloseAdmission));
}
fn close_guard(
    world: &mut World,
    mut cursor: Local<bevy::ecs::message::MessageCursor<bevy::window::WindowCloseRequested>>,
) {
    let requests: Vec<_> = cursor
        .read(world.resource::<Messages<bevy::window::WindowCloseRequested>>())
        .map(|event| event.window)
        .collect();
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, state: Mut<EditorWorkspace>| {
        let window = state.window;
        let blocked = !state.documents.update_clean() || super::settlement::drafts(world, &state);
        if blocked && requests.contains(&window) {
            notice(world, &state, "Unsaved source changes, property drafts or a pending Save remain. Accept/cancel drafts, then Save or explicitly discard changes before closing. Resolve pending Save with Retry Save.");
        }
        if let Ok(mut entity) = world.get_entity_mut(window) {
            if blocked { entity.insert(kedra_desktop::CloseBlocked); } else { entity.remove::<kedra_desktop::CloseBlocked>(); }
        }
    });
}
pub(super) fn discard(world: &mut World, state: &mut EditorWorkspace) -> Result<(), String> {
    let registry = world.resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let policies = world.resource::<kedra_scene::reflection::ComponentPolicies>();
    let change = state.documents.prepare_discard(&types, policies)?;
    if !state.documents.change_current(&change) {
        return Err("Source discard baseline changed".into());
    }
    // Explicit all-source discard retires every draft and scope identity. It is
    // not Undo, Save, external reload, or a reverse filesystem transaction.
    for panel in state.panels.inspectors.values_mut() {
        panel.discard_binding(world);
    }
    state.documents.publish_change(change);
    state.history_context = default();
    notice(
        world,
        state,
        "Unsaved source changes discarded. Files were not changed.",
    );
    Ok(())
}
pub(super) fn save(world: &mut World, state: &mut EditorWorkspace) -> Result<(), String> {
    if super::settlement::drafts(world, state) {
        return Err(
            "Accept or cancel property drafts before Save. Enter accepts; Escape cancels.".into(),
        );
    }
    let root = world
        .resource::<crate::model::ProjectManager>()
        .project
        .as_ref()
        .ok_or("No project is open")?
        .assets
        .clone();
    state.documents.begin_save(world, root)?;
    notice(world, state, "Saving source…");
    Ok(())
}
pub(super) fn apply(
    world: &mut World,
    state: &mut EditorWorkspace,
    redo: bool,
) -> Result<(), String> {
    if super::settlement::drafts(world, state) {
        return Err("Accept or cancel pending property drafts before source Undo/Redo.".into());
    }
    let registry = world.resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let policies = world.resource::<kedra_scene::reflection::ComponentPolicies>();
    let operation = state
        .documents
        .history_id(redo)
        .ok_or("Source history entry unavailable")?;
    let change = state.documents.prepare_history(redo, &types, policies)?;
    if !state.documents.change_current(&change) {
        return Err("Source history baseline changed".into());
    }
    state.documents.publish_change(change);
    super::history_context::restore(world, state, operation, redo);
    Ok(())
}
fn keyboard(
    mut event: On<FocusedInput<KeyboardInput>>,
    keys: Res<ButtonInput<Key>>,
    inputs: Query<(), With<bevy::text::EditableText>>,
    state: Option<Res<EditorWorkspace>>,
    mut commands: Commands,
) {
    let Some(state) = state else {
        return;
    };
    if !state
        .surfaces
        .values()
        .any(|surface| surface.window == event.input.window)
    {
        return;
    }
    let modifier = if cfg!(target_os = "macos") {
        Key::Super
    } else {
        Key::Control
    };
    if !keys.pressed(modifier) || keys.pressed(Key::Alt) {
        return;
    }
    let letter = |value: &str, code| match &event.input.logical_key {
        Key::Character(text) if text.is_ascii() => text.eq_ignore_ascii_case(value),
        Key::Character(_) => event.input.key_code == code,
        _ => false,
    };
    if letter("s", KeyCode::KeyS) && !keys.pressed(Key::Shift) {
        event.propagate(false);
        if event.input.state.is_pressed() && !event.input.repeat {
            commands.queue(|world: &mut World| {
                if !world.contains_resource::<EditorWorkspace>() {
                    return;
                }
                world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
                    if let Err(error) = save(world, &mut state) {
                        notice(world, &state, &error);
                    }
                });
            });
        }
        return;
    }
    if inputs.contains(event.focused_entity) {
        return;
    }
    let redo = if letter("z", KeyCode::KeyZ) {
        keys.pressed(Key::Shift)
    } else if !cfg!(target_os = "macos") && letter("y", KeyCode::KeyY) {
        true
    } else {
        return;
    };
    event.propagate(false);
    if !event.input.state.is_pressed() || event.input.repeat {
        return;
    }
    commands.queue(move |world: &mut World| {
        if !world.contains_resource::<EditorWorkspace>() {
            return;
        }
        world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            if let Err(error) = apply(world, &mut state, redo) {
                notice(world, &state, &error);
            }
        });
    });
}
