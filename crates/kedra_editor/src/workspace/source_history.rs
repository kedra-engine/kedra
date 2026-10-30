//! Explicit source history never borrows the local text undo shortcut.
use super::*;
use bevy::{
    input::keyboard::{Key, KeyboardInput},
    input_focus::FocusedInput,
};
pub(super) fn install(app: &mut App) {
    super::close_dialog::install(app);
    super::document_create::install(app);
    super::model_drop::install(app);
    super::asset_import::install(app);
    super::folders::install(app);
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
    let creating = super::document_create::pending(world) || super::import_pending(world);
    world.resource_scope(|world, state: Mut<EditorWorkspace>| {
        let window = state.window;
        let blocked =
            creating || !state.documents.update_clean() || super::settlement::drafts(world, &state);
        if blocked
            && requests.contains(&window)
            && let Err(error) = super::close_dialog::open(world, &state)
        {
            notice(world, &state, &error);
        }
        super::close_dialog::refresh(world, &state);
        if let Ok(mut entity) = world.get_entity_mut(window) {
            if blocked {
                entity.insert(kedra_desktop::CloseBlocked);
            } else {
                entity.remove::<kedra_desktop::CloseBlocked>();
            }
        }
    });
}
pub(super) fn discard(world: &mut World, state: &mut EditorWorkspace) -> Result<(), String> {
    if super::import_pending(world) {
        return Err("Finish importing assets before discarding sources.".into());
    }
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
    if super::settlement::source_edits(world, state) {
        return Err(
            "Finish pending source operations and accept or cancel property drafts before Save."
                .into(),
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
pub(super) fn save_all(world: &mut World, state: &mut EditorWorkspace) -> Result<(), String> {
    if super::settlement::source_edits(world, state) {
        return Err("Finish pending source operations and accept or cancel property drafts before Save All.".into());
    }
    let root = world
        .resource::<crate::model::ProjectManager>()
        .project
        .as_ref()
        .ok_or("No project is open")?
        .assets
        .clone();
    state.documents.begin_save_all(world, root)?;
    notice(world, state, "Saving all sources…");
    Ok(())
}
pub(super) fn available(
    world: &mut World,
    state: &EditorWorkspace,
    action: &input::Action,
) -> bool {
    match action {
        input::Action::SaveSource => {
            state.documents.can_save() && !super::settlement::source_edits(world, state)
        }
        input::Action::SaveAll => {
            state.documents.can_save_all() && !super::settlement::source_edits(world, state)
        }
        input::Action::SourceHistory(redo) => {
            state.documents.can_history(*redo) && !super::settlement::source_edits(world, state)
        }
        _ => true,
    }
}
pub(super) fn sync_controls(world: &mut World, state: &EditorWorkspace) {
    let controls: Vec<_> = world
        .query::<(Entity, &input::Action)>()
        .iter(world)
        .filter(|(_, action)| {
            matches!(
                action,
                input::Action::SaveSource
                    | input::Action::SaveAll
                    | input::Action::SourceHistory(_)
            )
        })
        .map(|(entity, action)| (entity, action.clone()))
        .collect();
    for (entity, action) in controls {
        let enabled = available(world, state, &action);
        if enabled {
            world
                .entity_mut(entity)
                .remove::<bevy::ui::InteractionDisabled>();
        } else {
            world
                .entity_mut(entity)
                .insert(bevy::ui::InteractionDisabled);
        }
    }
}
pub(super) fn new_document(
    world: &mut World,
    state: &mut EditorWorkspace,
    kind: kedra_scene::document::DocumentKind,
) -> Result<(), String> {
    if super::settlement::source_edits(world, state) {
        return Err("Accept or cancel property drafts before creating a document.".into());
    }
    super::document_create::open(world, state, kind)
}
pub(super) fn edit_nodes(
    world: &mut World,
    state: &mut EditorWorkspace,
    command: &crate::documents::structure::NodeCommand,
) -> Result<(), String> {
    if super::settlement::source_edits(world, state) {
        return Err("Accept or cancel property drafts before editing the hierarchy.".into());
    }
    let context = super::history_context::capture(world, state);
    let registry = world.resource::<AppTypeRegistry>().clone();
    state.documents.edit_nodes(
        command,
        &registry.read(),
        world.resource::<kedra_scene::reflection::ComponentPolicies>(),
    )?;
    super::history_context::accepted(world, state, context);
    Ok(())
}
pub(super) fn component(
    world: &mut World,
    state: &mut EditorWorkspace,
    command: &crate::documents::components::ComponentCommand,
) -> Result<(), String> {
    if super::settlement::source_edits(world, state) {
        return Err("Finish pending source edits before changing components.".into());
    }
    let context = super::history_context::capture(world, state);
    let registry = world.resource::<AppTypeRegistry>().clone();
    state.documents.change_component(
        command,
        &registry.read(),
        world.resource::<kedra_scene::reflection::ComponentPolicies>(),
    )?;
    super::history_context::accepted(world, state, context);
    Ok(())
}
pub(super) fn apply(
    world: &mut World,
    state: &mut EditorWorkspace,
    redo: bool,
) -> Result<(), String> {
    if super::settlement::source_edits(world, state) {
        return Err("Accept or cancel pending property drafts before source Undo/Redo.".into());
    }
    if let Some(path) = state.documents.selected_environment() {
        if state.documents.save_busy() || state.documents.save_pending() {
            return Err("Finish source Save before Undo/Redo".into());
        }
        return state.documents.environments.history(&path, redo);
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
    if letter("s", KeyCode::KeyS) {
        let all = keys.pressed(Key::Shift);
        event.propagate(false);
        if event.input.state.is_pressed() && !event.input.repeat {
            commands.queue(move |world: &mut World| {
                if !world.contains_resource::<EditorWorkspace>() {
                    return;
                }
                world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
                    if let Err(error) = if all {
                        save_all(world, &mut state)
                    } else {
                        save(world, &mut state)
                    } {
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
