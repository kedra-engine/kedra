use crate::{actions, model::*, screens};
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, block_on, poll_once},
};
use kedra_project::{Project, recent::RecentProjects};
use std::path::PathBuf;

#[derive(Resource)]
pub(crate) struct PendingJob {
    generation: u64,
    task: Task<Outcome>,
}
enum Outcome {
    History(Result<RecentProjects, String>),
    Directory(Option<PathBuf>),
    Project(Result<Prepared, String>),
    Preflight {
        intent: Intent,
        result: Result<(), String>,
    },
}
struct Prepared {
    project: Project,
    history: RecentProjects,
    warning: Option<String>,
}

pub(crate) fn load_history(world: &mut World) {
    let path = world.resource::<ProjectManager>().history_path.clone();
    world.insert_resource(PendingJob {
        generation: 0,
        task: IoTaskPool::get().spawn(async move {
            Outcome::History(path.map_or_else(
                || Err("Recent-project storage is unavailable on this account.".into()),
                |path| RecentProjects::load(&path).map_err(|e| e.to_string()),
            ))
        }),
    });
}

pub(crate) fn browse(world: &mut World) {
    let directory = world.resource::<EntryForm>().directory.clone();
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title("Choose project directory")
        .set_directory(directory);
    if let Some(entity) = world.resource::<kedra_desktop::DesktopSurface>().window {
        dialog =
            bevy::winit::WINIT_WINDOWS.with_borrow(|windows| match windows.get_window(entity) {
                Some(window) => dialog.set_parent(&**window),
                None => dialog,
            });
    }
    world.insert_resource(PendingJob {
        generation: world.resource::<ProjectManager>().generation,
        task: IoTaskPool::get().spawn(async move {
            let selected = dialog.pick_folder().await;
            Outcome::Directory(selected.map(|handle| handle.path().to_path_buf()))
        }),
    });
}

pub(crate) fn preflight(world: &mut World, intent: Intent) {
    world.insert_resource(PendingJob {
        generation: world.resource::<ProjectManager>().generation,
        task: IoTaskPool::get().spawn(async move {
            let result = match &intent {
                Intent::Open(root) => kedra_project::open(root).map(|_| ()),
                Intent::Create { parent, name } => {
                    kedra_project::validate_creation(parent, name).map(|_| ())
                }
            }
            .map_err(|e| e.to_string());
            Outcome::Preflight { intent, result }
        }),
    });
}

pub(crate) fn prepare(world: &mut World) {
    let manager = world.resource::<ProjectManager>();
    let Some(intent) = manager.intent.clone() else {
        return;
    };
    let generation = manager.generation;
    let mut history = manager.history.clone();
    let history_path = manager
        .history_writable
        .then(|| manager.history_path.clone())
        .flatten();
    world.insert_resource(PendingJob {
        generation,
        task: IoTaskPool::get().spawn(async move {
            let result = match intent {
                Intent::Open(root) => kedra_project::open(&root),
                Intent::Create { parent, name } => kedra_project::create(&parent, &name),
            };
            Outcome::Project(result.map_err(|e| e.to_string()).map(|project| {
                history.remember(&project);
                let warning = history_path
                    .and_then(|path| history.save(&path).err())
                    .map(|e| {
                        format!("Project opened, but recent projects could not be saved: {e}")
                    });
                Prepared {
                    project,
                    history,
                    warning,
                }
            }))
        }),
    });
}

pub(crate) fn poll(world: &mut World) {
    let Some(mut job) = world.remove_resource::<PendingJob>() else {
        return;
    };
    let Some(outcome) = block_on(poll_once(&mut job.task)) else {
        world.insert_resource(job);
        return;
    };
    if job.generation != world.resource::<ProjectManager>().generation {
        return;
    }
    match outcome {
        Outcome::Preflight { intent, result } => match result {
            Ok(()) => actions::begin(world, intent),
            Err(error) => preflight_failed(world, error),
        },
        Outcome::History(result) => {
            match result {
                Ok(history) => world.resource_mut::<ProjectManager>().history = history,
                Err(error) => {
                    let mut manager = world.resource_mut::<ProjectManager>();
                    manager.history_writable = false;
                    manager.notice = format!("{error} Existing history will not be overwritten.");
                }
            }
            world.resource_mut::<ProjectManager>().list_dirty = true;
            let initial = world
                .resource_mut::<ProjectManager>()
                .initial_project
                .take();
            if let Some(path) = initial {
                actions::begin(world, Intent::Open(path));
            }
        }
        Outcome::Directory(directory) => {
            if let Some(directory) = directory {
                match directory.to_str() {
                    Some(text) => {
                        world.resource_mut::<EntryForm>().directory = text.into();
                        screens::set_field(world,Field::Directory,text);
                    }
                    None => world.resource_mut::<EntryForm>().error = "This directory name is not valid Unicode. Choose another location; the path was not modified.".into(),
                }
            }
        }
        Outcome::Project(result) => match result {
            Ok(prepared) => {
                let mut manager = world.resource_mut::<ProjectManager>();
                manager.history = prepared.history;
                manager.complete(job.generation, Ok(prepared.project));
                if let Some(warning) = prepared.warning {
                    warn!("{warning}");
                    manager.notice = warning;
                }
            }
            Err(error) => {
                world
                    .resource_mut::<ProjectManager>()
                    .complete(job.generation, Err(error));
            }
        },
    }
}

fn preflight_failed(world: &mut World, error: String) {
    if world.resource::<EntryForm>().mode.is_none() {
        world.resource_mut::<ProjectManager>().notice = error.clone();
    }
    world.resource_mut::<EntryForm>().error = error;
}

pub(crate) fn finish_loading(mut manager: ResMut<ProjectManager>) {
    // Source readiness owns completion; a transient loading window never delays it.
    if manager.phase() == Phase::Loading && manager.is_ready() {
        manager.enter_editor();
    }
}

#[cfg(test)]
#[path = "jobs/lifecycle_tests.rs"]
mod lifecycle_tests;

#[cfg(test)]
mod tests {
    #[test]
    fn preflight_failure_without_a_form_is_visible_in_manager() {
        let mut world = bevy::prelude::World::new();
        world.init_resource::<super::ProjectManager>();
        world.init_resource::<super::EntryForm>();
        super::preflight_failed(&mut world, "Manifest was not found".into());
        assert_eq!(
            world.resource::<super::ProjectManager>().notice,
            "Manifest was not found"
        );
        world.resource_mut::<super::EntryForm>().mode = Some(super::EntryMode::Open);
        super::preflight_failed(&mut world, "Unsupported source format".into());
        assert_eq!(
            world.resource::<super::EntryForm>().error,
            "Unsupported source format"
        );
        assert_eq!(
            world.resource::<super::ProjectManager>().phase(),
            super::Phase::Projects
        );
    }
}
