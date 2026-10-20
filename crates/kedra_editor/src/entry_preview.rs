//! Modal-owned read-only project identity preview. Submission independently
//! revalidates sources; a preview is never authority to open or execute code.
use crate::model::*;
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, block_on, poll_once},
};
use std::path::PathBuf;

#[derive(Component)]
pub(crate) struct Preview {
    name: Entity,
    detail: Entity,
    requested: Option<PathBuf>,
    task: Option<Task<Result<kedra_project::Project, String>>>,
}
impl Preview {
    pub(crate) fn new(name: Entity, detail: Entity) -> Self {
        Self {
            name,
            detail,
            requested: None,
            task: None,
        }
    }
}

pub(crate) fn sync(
    form: Res<EntryForm>,
    mut previews: Query<&mut Preview>,
    mut texts: Query<&mut Text>,
) {
    if form.mode != Some(EntryMode::Open) {
        return;
    }
    let (requested, invalid) = match entry_intent(&form) {
        Ok(intent) => (Some(intent.root()), None),
        Err(error) => (None, Some(error)),
    };
    for mut preview in &mut previews {
        if let Some(error) = &invalid {
            preview.requested = None;
            preview.task = None;
            set(&mut texts, preview.name, "Choose a project folder".into());
            set(&mut texts, preview.detail, error.clone());
            continue;
        }
        if preview.requested != requested {
            preview.requested = requested.clone();
            preview.task = requested.clone().map(|path| {
                IoTaskPool::get().spawn(async move {
                    kedra_project::open(&path).map_err(|error| error.to_string())
                })
            });
            set(&mut texts, preview.name, "Checking project…".into());
            set(&mut texts, preview.detail, "kedra.toml".into());
        }
        let result = preview
            .task
            .as_mut()
            .and_then(|task| block_on(poll_once(task)));
        if let Some(result) = result {
            preview.task = None;
            let (name, detail) = match result {
                Ok(project) => (
                    project.manifest.name,
                    format!("kedra.toml · format {}", project.manifest.format),
                ),
                Err(error) => ("Project unavailable".into(), error),
            };
            set(&mut texts, preview.name, name);
            set(&mut texts, preview.detail, detail);
        }
    }
}

fn set(texts: &mut Query<&mut Text>, entity: Entity, value: String) {
    if let Ok(mut text) = texts.get_mut(entity)
        && text.0 != value
    {
        text.0 = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_and_failed_paths_never_apply_a_stale_project_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let first = kedra_project::create(directory.path(), "First")?;
        let second = kedra_project::create(directory.path(), "Second")?;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<EntryForm>()
            .add_systems(Update, sync);
        let name = app.world_mut().spawn(Text::new("")).id();
        let detail = app.world_mut().spawn(Text::new("")).id();
        app.world_mut().spawn(Preview::new(name, detail));
        app.world_mut().resource_mut::<EntryForm>().mode = Some(EntryMode::Open);
        app.world_mut().resource_mut::<EntryForm>().directory = first.root.display().to_string();
        app.update();
        app.world_mut().resource_mut::<EntryForm>().directory = second.root.display().to_string();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.world().get::<Text>(name).map(|text| text.0.as_str()) != Some("Second") {
            assert!(std::time::Instant::now() < deadline, "preview timed out");
            app.update();
            std::thread::yield_now();
        }
        assert_eq!(
            app.world().get::<Text>(detail).map(|text| text.0.as_str()),
            Some("kedra.toml · format 1")
        );
        let missing = directory.path().join("missing").display().to_string();
        app.world_mut().resource_mut::<EntryForm>().directory = missing.clone();
        while app.world().get::<Text>(name).map(|text| text.0.as_str())
            != Some("Project unavailable")
        {
            assert!(
                std::time::Instant::now() < deadline,
                "failure preview timed out"
            );
            app.update();
            std::thread::yield_now();
        }
        assert_eq!(app.world().resource::<EntryForm>().directory, missing);
        app.world_mut()
            .resource_mut::<EntryForm>()
            .directory
            .clear();
        app.update();
        assert_eq!(
            app.world().get::<Text>(name).map(|text| text.0.as_str()),
            Some("Choose a project folder")
        );
        Ok(())
    }
}
