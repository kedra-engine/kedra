//! Native picker and OS drops share one project-owned asynchronous source import.
use super::*;
use bevy::{tasks::IoTaskPool, window::FileDragAndDrop};
use kedra_wakeup::{Task, Wakeup};
use std::path::PathBuf;
#[cfg(test)]
mod tests;

#[derive(Clone)]
struct Destination {
    root: PathBuf,
    directory: PathBuf,
    view: ViewId,
    window: Entity,
    created: Option<String>,
}
enum Outcome {
    Selected(Option<Vec<PathBuf>>),
    Imported(
        Result<usize, String>,
        Result<kedra_authoring::persistence::SourceSnapshot, String>,
    ),
    Dismissed,
}
#[derive(Resource)]
struct Pending {
    destination: Destination,
    task: Task<Outcome>,
    progress: Option<std::sync::Mutex<std::sync::mpsc::Receiver<String>>>,
}
pub(super) fn pending(world: &World) -> bool {
    world.contains_resource::<Pending>()
}
pub(super) fn install(app: &mut App) {
    app.add_message::<FileDragAndDrop>()
        .add_systems(Update, (drops, poll).chain());
}

fn destination(
    world: &mut World,
    state: &EditorWorkspace,
    view: ViewId,
) -> Result<Destination, String> {
    if pending(world)
        || !state.documents.update_clean()
        || settlement::source_edits(world, state)
        || document_create::pending(world)
    {
        return Err(
            "Finish the current operation and save source changes before importing.".into(),
        );
    }
    if !inspector_visible(state, view) {
        return Err("Asset Browser is no longer visible.".into());
    }
    let panel = state
        .panels
        .browsers
        .get(&view)
        .ok_or("Asset Browser is unavailable")?;
    if !panel.listing_ready() {
        return Err("Wait for a current source listing before importing.".into());
    }
    let root = world
        .resource::<crate::model::ProjectManager>()
        .project
        .as_ref()
        .ok_or("No project is open")?
        .assets
        .clone();
    let directory = panel.directory();
    kedra_import::ingest::checked_directory(&root, &directory)?;
    let window = kedra_ui::native_input::owner(world, panel.root())
        .ok_or("Browser window is unavailable")?;
    Ok(Destination {
        root,
        directory,
        view,
        window,
        created: None,
    })
}
pub(super) fn create_environment(
    world: &mut World,
    state: &mut EditorWorkspace,
    view: ViewId,
    directory: &std::path::Path,
    name: &str,
) -> Result<(), String> {
    kedra_authoring_fs::entries::name(name)?;
    if name.ends_with(".ron") && !name.ends_with(".environment.ron") {
        return Err("Use a name ending in .environment.ron".into());
    }
    if pending(world) || !state.documents.update_clean() || settlement::source_edits(world, state) {
        return Err("Finish pending source operations before creating an environment".into());
    }
    let panel = state
        .panels
        .browsers
        .get(&view)
        .ok_or("Asset Browser unavailable")?;
    if !inspector_visible(state, view) || panel.directory() != directory || !panel.listing_ready() {
        return Err("Environment creation destination changed".into());
    }
    let root = world
        .resource::<crate::model::ProjectManager>()
        .project
        .as_ref()
        .ok_or("No project open")?
        .assets
        .clone();
    let window =
        kedra_ui::native_input::owner(world, panel.root()).ok_or("Browser window unavailable")?;
    let filename = if name.ends_with(".environment.ron") {
        name.to_owned()
    } else {
        format!("{name}.environment.ron")
    };
    let destination = Destination {
        root: root.clone(),
        directory: directory.into(),
        view,
        window,
        created: Some(format!(
            "game://{}",
            directory
                .join(filename)
                .to_str()
                .ok_or("Asset path must be Unicode")?
                .replace(std::path::MAIN_SEPARATOR, "/")
        )),
    };
    let directory = directory.to_owned();
    let name = name.to_owned();
    let task = Wakeup::for_world(world).spawn(IoTaskPool::get(), async move {
        let result = kedra_import::ingest::create_environment(&root, &directory, &name);
        Outcome::Imported(result, kedra_authoring_fs::source::read(&root, "game"))
    });
    notice(world, state, "Creating environment…");
    world.insert_resource(Pending {
        destination,
        task,
        progress: None,
    });
    Ok(())
}
pub(super) fn choose(world: &mut World, state: &mut EditorWorkspace, view: ViewId) {
    let destination = match destination(world, state, view) {
        Ok(value) => value,
        Err(error) => {
            notice(world, state, &error);
            return;
        }
    };
    let mut dialog = rfd::AsyncFileDialog::new().set_title("Import files");
    dialog = bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
        match windows.get_window(destination.window) {
            Some(window) => dialog.set_parent(&**window),
            None => dialog,
        }
    });
    notice(world, state, "Choosing files to import…");
    let task = Wakeup::for_world(world).spawn(IoTaskPool::get(), async move {
        Outcome::Selected(
            dialog
                .pick_files()
                .await
                .map(|files| files.into_iter().map(|f| f.path().to_owned()).collect()),
        )
    });
    world.insert_resource(Pending {
        destination,
        task,
        progress: None,
    });
}
fn start(
    world: &mut World,
    state: &EditorWorkspace,
    destination: Destination,
    files: Vec<PathBuf>,
) {
    let root = destination.root.clone();
    let directory = destination.directory.clone();
    let (sender, receiver) = std::sync::mpsc::sync_channel(32);
    let sender = Wakeup::for_world(world).sender(sender);
    notice(world, state, "Importing assets…");
    let task = Wakeup::for_world(world).spawn(IoTaskPool::get(), async move {
        let result = kedra_import::ingest::import_files(&root, &directory, &files, |message| {
            let _ = sender.try_send(message.to_owned());
        });
        let snapshot = kedra_authoring_fs::source::read(&root, "game");
        Outcome::Imported(result, snapshot)
    });
    world.insert_resource(Pending {
        destination,
        task,
        progress: Some(std::sync::Mutex::new(receiver)),
    });
}
fn poll(world: &mut World) {
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    let Some(mut pending) = world.remove_resource::<Pending>() else {
        return;
    };
    let latest = pending
        .progress
        .as_ref()
        .and_then(|receiver| receiver.lock().ok()?.try_iter().last());
    let result = pending.task.poll();
    let Some(mut state) = world.remove_resource::<EditorWorkspace>() else {
        world.insert_resource(pending);
        return;
    };
    if let Some(message) = latest {
        notice(world, &state, &message);
    }
    match result {
        None => {
            world.insert_resource(pending);
        }
        Some(Outcome::Selected(Some(files))) => {
            // The picker pins its original directory even if the browser navigated.
            let current = world
                .resource::<crate::model::ProjectManager>()
                .project
                .as_ref()
                .is_some_and(|p| p.assets == pending.destination.root);
            if current
                && state
                    .panels
                    .browsers
                    .contains_key(&pending.destination.view)
                && state.documents.update_clean()
                && !settlement::source_edits(world, &state)
            {
                start(world, &state, pending.destination, files);
            } else {
                notice(
                    world,
                    &state,
                    "Import cancelled: project or source state changed.",
                );
            }
        }
        Some(Outcome::Selected(None)) => notice(world, &state, "Import cancelled."),
        Some(Outcome::Dismissed) => (),
        Some(Outcome::Imported(result, snapshot)) => {
            let current = world
                .resource::<crate::model::ProjectManager>()
                .project
                .as_ref()
                .is_some_and(|p| p.assets == pending.destination.root);
            if current {
                let registry = world.resource::<AppTypeRegistry>().clone();
                let admission = snapshot.and_then(|source| {
                    state.documents.imported(
                        source,
                        &registry.read(),
                        world.resource::<kedra_scene::reflection::ComponentPolicies>(),
                    )
                });
                for panel in state.panels.browsers.values_mut() {
                    if let Err(error) = panel.refresh(world) {
                        panel.message(error);
                    }
                }
                let result = result.and_then(|count| {
                    admission.map(|()| count).map_err(|error| {
                        format!(
                            "Imported {count} source file(s), but source refresh failed: {error}"
                        )
                    })
                });
                match result {
                    Ok(_) if pending.destination.created.is_some() => {
                        let path = pending.destination.created.as_deref().unwrap_or_default();
                        state.documents.select_asset(
                            world,
                            pending.destination.root.clone(),
                            path.into(),
                        );
                        notice(world, &state, "Environment created.");
                    }
                    Ok(count) => notice(
                        world,
                        &state,
                        &format!(
                            "Imported {count} asset(s) into assets/{}.",
                            pending.destination.directory.display()
                        ),
                    ),
                    Err(error) => {
                        notice(world, &state, &error);
                        let mut dialog = rfd::AsyncMessageDialog::new()
                            .set_title("Import failed")
                            .set_level(rfd::MessageLevel::Error)
                            .set_description(&error);
                        dialog = bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
                            match windows.get_window(pending.destination.window) {
                                Some(window) => dialog.set_parent(&**window),
                                None => dialog,
                            }
                        });
                        let task = Wakeup::for_world(world).spawn(IoTaskPool::get(), async move {
                            dialog.show().await;
                            Outcome::Dismissed
                        });
                        world.insert_resource(Pending {
                            destination: pending.destination,
                            task,
                            progress: None,
                        });
                    }
                }
            }
        }
    }
    world.insert_resource(state);
}

fn target(world: &World, state: &EditorWorkspace, window: Entity) -> Option<ViewId> {
    if !state.menu.is_empty() || pending(world) {
        return None;
    }
    let position = world.get::<Window>(window)?.cursor_position()?;
    state.panels.browsers.iter().find_map(|(id, panel)| {
        (inspector_visible(state, *id)
            && kedra_ui::native_input::owner(world, panel.root()) == Some(window)
            && kedra_ui::docking::rect(world, panel.root())
                .is_some_and(|rect| rect.contains(position)))
        .then_some(*id)
    })
}

fn drops(
    world: &mut World,
    mut cursor: Local<bevy::ecs::message::MessageCursor<FileDragAndDrop>>,
    mut hovering: Local<std::collections::BTreeSet<Entity>>,
) {
    let mut batches = BTreeMap::<Entity, Vec<PathBuf>>::new();
    for event in cursor.read(world.resource::<Messages<FileDragAndDrop>>()) {
        match event {
            FileDragAndDrop::DroppedFile { window, path_buf } => {
                hovering.remove(window);
                batches.entry(*window).or_default().push(path_buf.clone());
            }
            FileDragAndDrop::HoveredFile { window, .. } => {
                hovering.insert(*window);
            }
            FileDragAndDrop::HoveredFileCanceled { window } => {
                hovering.remove(window);
            }
        }
    }
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, state: Mut<EditorWorkspace>| {
        let targets: Vec<_> = hovering
            .iter()
            .filter_map(|window| target(world, &state, *window))
            .collect();
        for (id, panel) in &state.panels.browsers {
            kedra_ui::docking::class(world, panel.root(), "drop-target", targets.contains(id));
        }
        for (window, files) in batches {
            let view = target(world, &state, window);
            let Some(view) = view else {
                continue;
            };
            match destination(world, &state, view) {
                Ok(destination) => start(world, &state, destination, files),
                Err(error) => notice(world, &state, &error),
            }
        }
    });
}
