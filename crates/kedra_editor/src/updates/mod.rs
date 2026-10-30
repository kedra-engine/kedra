//! Development-stage release discovery, verified downloads and explicit application.
mod exit;
mod host;
mod model;
mod restart;
#[cfg(test)]
mod tests;
mod view;

use bevy::{prelude::*, tasks::IoTaskPool};
use kedra_update::{self as backend, Kind, Release, VerifiedPackage};
use kedra_wakeup::{Task, Wakeup};
use model::{Effect, Model, Phase};
use restart::Restart;
use std::{
    path::{Path, PathBuf},
    sync::{Mutex, mpsc},
};

#[derive(Resource, Default)]
struct Updates {
    model: Model,
    installation: Option<host::Installation>,
    cache: Option<PathBuf>,
    release: Option<Release>,
    package: Option<VerifiedPackage>,
    task: Option<Task<Outcome>>,
    events: Option<Mutex<mpsc::Receiver<Event>>>,
    received: u64,
    prepared: Option<Restart>,
    exit_authorized: bool,
    project_generation: Option<u64>,
    state_directory: Option<PathBuf>,
    arguments: Vec<std::ffi::OsString>,
    handoff: Option<mpsc::SyncSender<(Restart, Vec<std::ffi::OsString>)>>,
}
enum Outcome {
    Discovered(Result<Option<Release>, String>),
    Downloaded(u64, Result<VerifiedPackage, String>),
    Revealed(u64, VerifiedPackage, Result<(), String>),
    Prepared(u64, VerifiedPackage, Result<backend::Prepared, String>),
    InvalidPackage(u64, String),
}
enum Event {
    Progress(u64, u64),
    Verifying(u64),
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Updates>()
        .add_observer(activate)
        .add_systems(Update, (poll, view::sync).chain());
    app.add_systems(
        PostUpdate,
        authorize_exit.after(bevy::text::EditableTextSystems),
    );
}

pub(crate) fn configure(world: &mut World, state: Option<&Path>) -> Result<(), String> {
    let wakeup = Wakeup::for_world(world);
    let config = host::config()?;
    let mut installation = match host::detect(&config) {
        Ok(Some(installation)) => installation,
        Ok(None) => return Ok(()),
        Err(error) => {
            warn!("Self-update disabled: {error}");
            return Ok(());
        }
    };
    if installation.kind == Kind::MacApp {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let root = executable
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .ok_or("Installed application root is missing")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        // A contending launch must not continue without a shared lease while an
        // exclusive updater is replacing this installation.
        let Some(lease) = host::reserve_update_access(&config, root)? else {
            return Ok(());
        };
        installation.macos = Some(lease);
    }
    let cache = state.map(|state| state.join("updates")).or_else(|| {
        directories::ProjectDirs::from("dev", "kedra", "Kedra")
            .map(|dirs| dirs.cache_dir().join("updates"))
    });
    let Some(cache) = cache else {
        warn!("Self-update disabled: local update cache is unavailable");
        return Ok(());
    };
    let mut updates = world.resource_mut::<Updates>();
    let kind = installation.kind;
    updates.installation = Some(installation);
    updates.cache = Some(cache);
    updates.state_directory = state
        .map(std::path::absolute)
        .transpose()
        .map_err(|e| e.to_string())?;
    updates.task = Some(wakeup.spawn(IoTaskPool::get(), async move {
        Outcome::Discovered(backend::discover(&config, kind))
    }));
    Ok(())
}

fn activate(
    event: On<bevy::ui_widgets::Activate>,
    controls: Query<(), With<view::Control>>,
    mut commands: Commands,
) {
    if controls.contains(event.entity) {
        commands.queue(activated);
    }
}

fn activated(world: &mut World) {
    let wakeup = Wakeup::for_world(world);
    let Some(mut updates) = world.remove_resource::<Updates>() else {
        return;
    };
    if updates.task.is_none() {
        match updates.model.activate() {
            Some(Effect::Download(generation)) => {
                if let (Some(release), Some(cache)) =
                    (updates.release.clone(), updates.cache.clone())
                {
                    let (sender, receiver) = mpsc::sync_channel(32);
                    let sender = wakeup.sender(sender);
                    updates.events = Some(Mutex::new(receiver));
                    updates.received = 0;
                    updates.package = None;
                    updates.task = Some(wakeup.spawn(IoTaskPool::get(), async move {
                        let result = backend::download(
                            &release,
                            &cache,
                            |bytes| {
                                let _ = sender.try_send(Event::Progress(generation, bytes));
                            },
                            || {
                                let _ = sender.send(Event::Verifying(generation));
                            },
                        );
                        Outcome::Downloaded(generation, result)
                    }));
                }
            }
            Some(Effect::Apply(generation)) => {
                if let (Some(mut package), Some(release), Some(installation)) = (
                    updates.package.take(),
                    updates.release.clone(),
                    updates.installation.clone(),
                ) {
                    if let Some(macos) = installation.macos {
                        if let Err(error) = exit::ready(world) {
                            updates.package = Some(package);
                            updates.model.applied(generation, error);
                        } else {
                            updates.project_generation = world
                                .get_resource::<crate::model::ProjectManager>()
                                .map(|m| m.generation);
                            updates.task = Some(wakeup.spawn(IoTaskPool::get(), async move {
                                let result =
                                    backend::prepare_macos(macos, package.path(), &release);
                                if result.is_err()
                                    && let Err(error) = package.reverify(&release)
                                {
                                    return Outcome::InvalidPackage(generation, error);
                                }
                                Outcome::Prepared(generation, package, result)
                            }));
                        }
                        world.insert_resource(updates);
                        return;
                    }
                    updates.task = Some(wakeup.spawn(IoTaskPool::get(), async move {
                        if let Err(error) = package.reverify(&release) {
                            return Outcome::InvalidPackage(generation, error);
                        }
                        let result = (|| {
                            backend::reveal_command(package.path(), installation.kind)
                                .spawn()
                                .map_err(|e| format!("Cannot open the package location: {e}"))?;
                            package.persist();
                            Ok(())
                        })();
                        Outcome::Revealed(generation, package, result)
                    }));
                } else {
                    updates.model.invalidated(
                        generation,
                        "Verified update is unavailable; download again".into(),
                    );
                }
            }
            None => {}
        }
    }
    world.insert_resource(updates);
}

fn poll(world: &mut World) {
    let Some(mut updates) = world.remove_resource::<Updates>() else {
        return;
    };
    let outcome = updates.task.as_mut().and_then(Task::poll);
    if let Some(receiver) = &updates.events {
        let events = receiver
            .lock()
            .map(|receiver| receiver.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Progress(generation, bytes) if generation == updates.model.generation => {
                    updates.received = bytes
                }
                Event::Verifying(generation) => {
                    updates
                        .model
                        .receive(generation, Phase::Verifying, String::new())
                }
                _ => {}
            }
        }
    }
    if let Some(outcome) = outcome {
        updates.task = None;
        match outcome {
            Outcome::InvalidPackage(generation, error)
                if generation == updates.model.generation =>
            {
                updates.package = None;
                updates
                    .model
                    .invalidated(generation, format!("{error}; activate to download again"));
            }
            Outcome::Discovered(Ok(Some(release))) => {
                info!(
                    "Update {} available from source {}",
                    release.version(),
                    release.source_revision()
                );
                updates.release = Some(release);
                updates.model.available();
            }
            Outcome::Discovered(Ok(None)) => {}
            Outcome::Discovered(Err(error)) => warn!("Update discovery unavailable: {error}"),
            Outcome::Downloaded(generation, result) if generation == updates.model.generation => {
                updates.events = None;
                match result {
                    Ok(package) => {
                        updates.package = Some(package);
                        updates
                            .model
                            .receive(generation, Phase::Ready, String::new());
                    }
                    Err(error) => updates.model.receive(generation, Phase::Error, error),
                }
            }
            Outcome::Revealed(generation, package, result)
                if generation == updates.model.generation =>
            {
                updates.package = Some(package);
                updates
                    .model
                    .applied(generation, result.err().unwrap_or_default());
            }
            Outcome::Prepared(generation, package, result)
                if generation == updates.model.generation =>
            {
                updates.package = Some(package);
                match result {
                    Ok(prepared) => updates.prepared = Some(Restart::new(prepared)),
                    Err(error) => updates.model.applied(generation, error),
                }
            }
            _ => {}
        }
    }
    world.insert_resource(updates);
}

fn authorize_exit(world: &mut World) {
    let Some(mut updates) = world.remove_resource::<Updates>() else {
        return;
    };
    if updates.prepared.is_some() && !updates.exit_authorized {
        let owner = world
            .get_resource::<crate::model::ProjectManager>()
            .map(|m| m.generation);
        let result = if owner != updates.project_generation {
            Err("Project changed while the update was prepared; activate Update again".into())
        } else {
            exit::ready(world).and_then(|()| crate::workspace::save_for_update(world))
        };
        match result {
            Ok(()) => {
                updates.arguments.clear();
                if let Some(project) = world
                    .get_resource::<crate::model::ProjectManager>()
                    .and_then(|m| m.project())
                {
                    updates.arguments.push(project.root.as_os_str().to_owned());
                }
                if let Some(path) = updates.state_directory.clone() {
                    updates.arguments.push("--state-directory".into());
                    updates.arguments.push(path.into_os_string());
                }
                if let (Some(sender), Some(prepared)) = (&updates.handoff, updates.prepared.take())
                {
                    match sender.try_send((prepared, std::mem::take(&mut updates.arguments))) {
                        Ok(()) => {
                            updates.exit_authorized = true;
                            world.write_message(AppExit::Success);
                        }
                        Err(_) => {
                            let generation = updates.model.generation;
                            updates.model.applied(
                                generation,
                                "Update restart handoff is unavailable; the editor was kept open"
                                    .into(),
                            );
                        }
                    }
                } else {
                    let generation = updates.model.generation;
                    updates.model.applied(
                        generation,
                        "Update restart handoff is unavailable; the editor was kept open".into(),
                    );
                }
            }
            Err(error) => {
                updates.prepared = None;
                let generation = updates.model.generation;
                updates.model.applied(generation, error);
            }
        }
    }
    world.insert_resource(updates);
}

pub(crate) fn handoff(world: &mut World) -> mpsc::Receiver<(Restart, Vec<std::ffi::OsString>)> {
    let (sender, receiver) = mpsc::sync_channel(1);
    world.resource_mut::<Updates>().handoff = Some(sender);
    receiver
}

pub(crate) fn bind(
    world: &mut World,
    view: &kedra_ui::authored::view::UiView,
) -> Result<(), String> {
    view::bind(world, view)
}
