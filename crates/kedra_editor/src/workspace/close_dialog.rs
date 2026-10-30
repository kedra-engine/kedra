//! Editor exit authorization. Saving never implies discarding a draft or failed publication.
use super::*;

#[derive(Component)]
struct Dialog {
    view: UiView,
    waiting: bool,
}
#[derive(Component, Clone, Copy)]
enum Choice {
    Save,
    Discard,
    Cancel,
}

pub(super) fn install(app: &mut App) {
    app.add_observer(activate).add_observer(cancel);
}
pub(super) fn open(world: &mut World, state: &EditorWorkspace) -> Result<(), String> {
    if world.query::<&Dialog>().iter(world).next().is_some() {
        return Ok(());
    }
    let view = crate::views::mount_asset(
        world,
        state.surfaces[&state.host].root,
        "editor://prefabs/close-dialog.prefab.ron",
        "close-dialog",
    )?;
    let actions = view.element("actions").map_err(|e| e.to_string())?;
    for (choice, label, id) in [
        (Choice::Cancel, "Cancel", "cancel"),
        (Choice::Discard, "Exit without saving", "discard"),
        (Choice::Save, "Save and exit", "save"),
    ] {
        let button = crate::views::mount_asset(
            world,
            actions,
            "editor://prefabs/button.prefab.ron",
            &format!("close-dialog/{id}"),
        )?;
        let caption = world
            .query::<(Entity, &ChildOf, &Text)>()
            .iter(world)
            .find_map(|(e, p, _)| (p.parent() == button.root).then_some(e))
            .ok_or("Button caption missing")?;
        world.get_mut::<Text>(caption).ok_or("Caption retired")?.0 = label.into();
        world.entity_mut(button.root).insert((
            choice,
            kedra_ui::ElementId::new(format!("close-dialog/{id}")),
        ));
    }
    let root = view.root;
    world.entity_mut(root).insert(Dialog {
        view,
        waiting: false,
    });
    refresh(world, state);
    Ok(())
}
pub(super) fn refresh(world: &mut World, state: &EditorWorkspace) {
    let dialog = world
        .query::<(Entity, &Dialog)>()
        .iter(world)
        .next()
        .map(|(e, d)| {
            (
                e,
                d.waiting,
                d.view.element("sources").ok(),
                d.view.element("error").ok(),
            )
        });
    let Some((root, waiting, sources, error)) = dialog else {
        return;
    };
    let drafts = super::settlement::drafts(world, state);
    let operations = super::document_create::pending(world)
        || super::import_pending(world)
        || state.documents.creating()
        || state.documents.opening();
    if waiting && state.documents.update_clean() && !drafts && !operations {
        world.despawn(root);
        world.write_message(bevy::window::WindowCloseRequested {
            window: state.window,
        });
        return;
    }
    if waiting
        && !state.documents.save_busy()
        && let Some(mut dialog) = world.get_mut::<Dialog>(root)
    {
        dialog.waiting = false;
    }
    if let Some(mut text) = sources.and_then(|e| world.get_mut::<Text>(e)) {
        text.0 = state.documents.unsaved_paths().join("\n");
    }
    if let Some(mut text) = error.and_then(|e| world.get_mut::<Text>(e)) {
        text.0 = if let Some(error) = state.documents.save_error() {
            error.to_owned()
        } else if operations {
            "Finish the current source operation before exiting.".into()
        } else if drafts {
            "Unaccepted property edits remain. Accept them to save, or exit without saving.".into()
        } else if state.documents.save_busy() {
            "Saving…".into()
        } else {
            String::new()
        };
    }
    let buttons: Vec<_> = world
        .query::<(Entity, &Choice)>()
        .iter(world)
        .map(|(e, c)| (e, *c))
        .collect();
    for (entity, choice) in buttons {
        let enabled = match choice {
            Choice::Cancel => true,
            Choice::Save => !operations && !drafts && state.documents.can_save_all(),
            Choice::Discard => {
                !operations && !state.documents.save_busy() && !state.documents.save_pending()
            }
        };
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
fn activate(
    event: On<bevy::ui_widgets::Activate>,
    choices: Query<&Choice>,
    mut commands: Commands,
) {
    let Ok(choice) = choices.get(event.entity) else {
        return;
    };
    let choice = *choice;
    commands.queue(move |world: &mut World| {
        if !world.contains_resource::<EditorWorkspace>() {
            return;
        }
        world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            let Some(root) = world
                .query_filtered::<Entity, With<Dialog>>()
                .iter(world)
                .next()
            else {
                return;
            };
            let result = match choice {
                Choice::Cancel => {
                    world.despawn(root);
                    return;
                }
                Choice::Save => source_history::save_all(world, &mut state),
                Choice::Discard => {
                    if super::document_create::pending(world) || super::import_pending(world) {
                        Err("Finish the current source operation first.".into())
                    } else {
                        source_history::discard(world, &mut state)
                    }
                }
            };
            match result {
                Ok(()) => {
                    if let Some(mut dialog) = world.get_mut::<Dialog>(root) {
                        dialog.waiting = true;
                    }
                }
                Err(message) => {
                    let error = world
                        .get::<Dialog>(root)
                        .and_then(|d| d.view.element("error").ok());
                    if let Some(mut text) = error.and_then(|e| world.get_mut::<Text>(e)) {
                        text.0 = message;
                    }
                }
            }
        });
    });
}
fn cancel(
    event: On<bevy::ui_widgets::RequestClose>,
    parents: Query<&ChildOf>,
    dialogs: Query<(), With<Dialog>>,
    mut commands: Commands,
) {
    if let Some(root) = std::iter::once(event.source)
        .chain(parents.iter_ancestors(event.source))
        .find(|e| dialogs.contains(*e))
    {
        commands.entity(root).try_despawn();
    }
}
