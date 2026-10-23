//! Editor feature adapter for shared engine automation. No second command model
//! writes project sources: UI and remote requests use the same request handler.
use crate::{actions, jobs, model::*, screens};
use bevy::prelude::*;
use bevy_remote::BrpResult;
use kedra_app::{
    RemoteAppExt,
    operations::{OperationState, Operations},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    NewForm,
    OpenForm,
    Cancel,
    Draft {
        name: Option<String>,
        directory: Option<String>,
    },
    Submit,
    Open {
        root: PathBuf,
    },
    Create {
        parent: PathBuf,
        name: String,
    },
    Projects,
    Retry,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    session: String,
    id: Uuid,
    command: Command,
}
#[derive(Resource, Default)]
struct Pending(Option<Uuid>);

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Pending>()
        .add_systems(Update, complete.after(jobs::poll));
    app.register_remote(
        "editor.state",
        "Observe startup feature state and actual drafts",
        json!({}),
        state,
    );
    app.register_remote(
        "editor.workspace",
        "Observe logical docking topology and native binding; not interaction acceptance",
        json!({}),
        workspace_state,
    );
    app.register_remote("editor.project","Execute the same validated project requests as UI",json!({"session":"app.describe.session","id":"UUID idempotency key","command":{"kind":["new_form","open_form","cancel","draft","submit","open","create","projects","retry"]}}),command);
    crate::scripting::install(app);
}
pub(crate) fn workspace_state(_: In<Option<Value>>, world: &mut World) -> BrpResult {
    let Some(workspace) = world.get_resource::<crate::workspace::EditorWorkspace>() else {
        return Ok(json!({"bound": false}));
    };
    Ok(json!({
        "bound": true,
        "native_alive": world.get::<Window>(workspace.window).is_some() && world.entities().contains(workspace.root),
        "main_host": workspace.host,
        "revision": workspace.layout.revision(),
        "layout": workspace.layout.snapshot(),
        "documents": workspace.documents.snapshot(),
        "scene_views":workspace.scene_state(world),
        "navigation_input":workspace.navigation_input(world),
    }))
}
fn state(_: In<Option<Value>>, world: &mut World) -> BrpResult {
    let model = world.resource::<ProjectManager>();
    let form = world.resource::<EntryForm>();
    let phase = if world
        .get_resource::<crate::boot::Boot>()
        .is_some_and(|boot| !boot.complete)
    {
        "boot".into()
    } else {
        format!("{:?}", model.phase()).to_lowercase()
    };
    Ok(
        json!({"phase":phase,"generation":model.generation,"preferences":model.history_path,"busy":world.contains_resource::<jobs::PendingJob>(),"ready":model.is_ready(),"error":model.loading_error(),"notice":model.notice,"project":model.project().map(|project|json!({"name":project.manifest.name,"root":project.root,"format":project.manifest.format})),"entry":form.mode.map(|mode|json!({"mode":if mode==EntryMode::Create {"create"} else {"open"},"name":form.name,"directory":form.directory,"error":form.error}))}),
    )
}
fn command(In(value): In<Option<Value>>, world: &mut World) -> BrpResult {
    if world.resource::<crate::ExplicitScene>().0 {
        return Err(kedra_app::invalid(
            "Explicit startup scenes exclude the project lifecycle",
        ));
    }
    let original = value.clone().unwrap_or(Value::Null);
    let request: Request = kedra_app::params(value)?;
    if world.resource::<kedra_app::AppSession>().id != request.session {
        return Err(kedra_app::invalid("Stale application session"));
    }
    if let Some(previous) = world.resource::<Operations>().get(request.id) {
        let previous = serde_json::to_value(previous).map_err(bevy_remote::BrpError::internal)?;
        world
            .resource_mut::<Operations>()
            .admit(request.id, &original)
            .map_err(kedra_app::invalid)?;
        return Ok(previous);
    }
    if world.resource::<Pending>().0.is_some() || world.contains_resource::<jobs::PendingJob>() {
        return Err(kedra_app::invalid("A project operation is already active"));
    }
    world
        .resource_mut::<Operations>()
        .admit(request.id, &original)
        .map_err(kedra_app::invalid)?;
    let result = execute(world, request.command);
    match result {
        Ok(true) => world.resource_mut::<Pending>().0 = Some(request.id),
        Ok(false) => {
            world
                .resource_mut::<Operations>()
                .finish(request.id, Ok(json!({"applied":true})));
        }
        Err(error) => {
            world
                .resource_mut::<Operations>()
                .finish(request.id, Err(error));
        }
    }
    serde_json::to_value(world.resource::<Operations>().get(request.id))
        .map_err(bevy_remote::BrpError::internal)
}

#[cfg(test)]
mod explicit_scene_tests {
    use super::*;
    #[test]
    fn explicit_scene_rejects_project_commands_before_admitting_any_operation() {
        let mut world = World::new();
        world.insert_resource(crate::ExplicitScene(true));
        let result = command(In(None), &mut world);
        assert!(result.is_err_and(|error| error.message.contains("exclude the project lifecycle")));
        assert!(!world.contains_resource::<Pending>());
        assert!(!world.contains_resource::<crate::jobs::PendingJob>());
    }
}
fn execute(world: &mut World, command: Command) -> Result<bool, String> {
    if world
        .get_resource::<crate::boot::Boot>()
        .is_some_and(|boot| !boot.complete)
    {
        return Err("Wait for engine startup to complete".into());
    }
    let phase = world.resource::<ProjectManager>().phase();
    match command {
        Command::NewForm | Command::OpenForm => {
            if phase != Phase::Projects || world.resource::<ProjectManager>().modal.is_some() {
                return Err("Return to projects and close the current form first".into());
            }
            actions::dispatch(
                world,
                if matches!(command, Command::NewForm) {
                    Action::New
                } else {
                    Action::Open
                },
                None,
            );
        }
        Command::Cancel => {
            if world.resource::<EntryForm>().mode.is_none() {
                return Err("No form is open".into());
            }
            actions::dispatch(world, Action::Cancel, None);
        }
        Command::Draft { name, directory } => {
            if world.resource::<EntryForm>().mode.is_none() {
                return Err("Open a project form first".into());
            }
            if name
                .iter()
                .chain(directory.iter())
                .any(|value| value.len() > 16384)
            {
                return Err("Draft value exceeds 16 KiB".into());
            }
            let mut query = world.query::<(&Field, &bevy::text::EditableText)>();
            let ready = (name.is_none()
                || query.iter(world).any(|(field, _)| *field == Field::Name))
                && (directory.is_none()
                    || query
                        .iter(world)
                        .any(|(field, _)| *field == Field::Directory));
            if !ready {
                return Err(
                    "Form fields are not mounted yet; observe readiness before editing".into(),
                );
            }
            for (field, value) in [(Field::Name, name), (Field::Directory, directory)] {
                if let Some(value) = value {
                    screens::set_field(world, field, &value);
                }
            }
            screens::capture_fields(world);
        }
        Command::Submit => {
            if world.resource::<EntryForm>().mode.is_none() {
                return Err("No form is open".into());
            }
            actions::dispatch(world, Action::Submit, None);
            if !world.contains_resource::<jobs::PendingJob>() {
                return Err(world.resource::<EntryForm>().error.clone());
            }
            return Ok(true);
        }
        Command::Open { root } => {
            actions::request_project(world, Intent::Open(root))?;
            return Ok(true);
        }
        Command::Create { parent, name } => {
            actions::request_project(world, Intent::Create { parent, name })?;
            return Ok(true);
        }
        Command::Projects => {
            if phase != Phase::Editor {
                return Err("Editor is not open".into());
            }
            actions::dispatch(world, Action::Projects, None);
        }
        Command::Retry => {
            if !world.resource_mut::<ProjectManager>().retry() {
                return Err("Only a failed read-only open can be retried".into());
            }
            jobs::prepare(world);
            return Ok(true);
        }
    }
    Ok(false)
}
fn complete(world: &mut World) {
    let Some(id) = world.resource::<Pending>().0 else {
        return;
    };
    let manager = world.resource::<ProjectManager>();
    let result = if manager.phase() == Phase::Editor {
        Some(Ok(
            json!({"opened":manager.project().map(|project|project.root.clone())}),
        ))
    } else if let Some(error) = manager.loading_error() {
        Some(Err(error.to_string()))
    } else if !world.contains_resource::<jobs::PendingJob>()
        && !world.resource::<EntryForm>().error.is_empty()
    {
        Some(Err(world.resource::<EntryForm>().error.clone()))
    } else {
        None
    };
    if let Some(result) = result {
        world.resource_mut::<Operations>().finish(id, result);
        world.resource_mut::<Pending>().0 = None;
    }
    // Defensive check: terminal receipts are never replayed by a later frame.
    if world
        .resource::<Operations>()
        .get(id)
        .is_some_and(|op| op.state != OperationState::Pending)
    {
        world.resource_mut::<Pending>().0 = None;
    }
}

#[cfg(test)]
#[path = "remote/lifecycle_tests.rs"]
mod lifecycle_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_draft_is_atomic_and_has_an_idempotent_failure_receipt() {
        let mut world = World::new();
        world.init_resource::<crate::ExplicitScene>();
        world.init_resource::<ProjectManager>();
        world.init_resource::<EntryForm>();
        world.init_resource::<Pending>();
        world.init_resource::<Operations>();
        world.insert_resource(kedra_app::AppSession { id: "test".into() });
        world.resource_mut::<EntryForm>().mode = Some(EntryMode::Create);
        let name = world
            .spawn((Field::Name, bevy::text::EditableText::new("Original")))
            .id();
        world.spawn((Field::Directory, bevy::text::EditableText::new("/original")));
        let request = json!({"session":"test","id":Uuid::new_v4(),"command":{"kind":"draft","name":"Changed","directory":"x".repeat(16385)}});
        let receipt = command(In(Some(request.clone())), &mut world)
            .unwrap_or_else(|error| panic!("{error:?}"));
        assert_eq!(receipt["state"], "failed");
        assert_eq!(
            world
                .get::<bevy::text::EditableText>(name)
                .map(|field| field.value().to_string()),
            Some("Original".into())
        );
        let retry = command(In(Some(request.clone())), &mut world)
            .unwrap_or_else(|error| panic!("{error:?}"));
        assert_eq!(retry, receipt);
        let mut conflict = request;
        conflict["command"]["name"] = json!("Different");
        assert!(command(In(Some(conflict)), &mut world).is_err());
    }
}
