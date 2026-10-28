//! One explicit startup authorization, bound to the requested project lifetime.
use bevy::prelude::*;
use kedra_script::{Program, Run, Status};
use std::{path::PathBuf, time::Instant};

/// Explicit native authorization for one program. Never inferred from project files
/// or saved preferences. Scripts receive no grants beyond these exact source paths.
pub struct Launch {
    /// Canonical project-relative path beneath Editor/.
    pub path: String,
    /// Exact game:// document paths writable during this run.
    pub sources: Vec<String>,
}
/// Owned execution outcome; retaining it does not retain a running VM or edit lease.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "state", content = "result", rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Waiting for ordinary project/source admission.
    Queued,
    /// The finite program may resume within its runtime budget.
    Running,
    /// Bounded owned JSON result. Successful explicit commits remain accepted.
    Completed(serde_json::Value),
    /// Admission/runtime failure, not rollback of already successful commits.
    Failed(String),
    /// Host/user revocation; uncommitted leases cannot publish afterwards.
    Cancelled,
}
#[derive(Resource)]
struct Execution {
    project: PathBuf,
    first: PathBuf,
    program: Option<Program>,
    requested: bool,
    run: Option<Run>,
    status: ExecutionStatus,
    since: Instant,
}
/// Read the one explicitly authorized startup invocation, if configured.
pub fn status(world: &World) -> Option<ExecutionStatus> {
    world
        .get_resource::<Execution>()
        .map(|state| state.status.clone())
}
/// Revoke an active or queued invocation. Already committed working data is not
/// discarded or saved; subsequent resumed effects are refused by runtime Scope.
pub fn cancel(world: &mut World) -> Result<(), String> {
    if !matches!(
        status(world),
        Some(ExecutionStatus::Queued | ExecutionStatus::Running)
    ) {
        return Err("No active editor script invocation exists".into());
    }
    let run = world
        .get_resource::<Execution>()
        .ok_or("No editor script invocation exists")?
        .run;
    if let Some(run) = run {
        kedra_script::cancel(world, run).map_err(|error| error.to_string())?;
    }
    let mut execution = world.resource_mut::<Execution>();
    execution.program = None;
    execution.status = ExecutionStatus::Cancelled;
    Ok(())
}
pub(crate) fn configure(
    app: &mut App,
    project: &std::path::Path,
    launch: Launch,
) -> Result<(), String> {
    let program = super::load(project, &launch.path, launch.sources.clone())?;
    let first = launch
        .sources
        .first()
        .and_then(|path| path.strip_prefix("game://"))
        .ok_or("Editor source grant missing")?
        .into();
    let project = std::fs::canonicalize(project).map_err(|error| error.to_string())?;
    app.insert_resource(Execution {
        project,
        first,
        program: Some(program),
        requested: false,
        run: None,
        status: ExecutionStatus::Queued,
        since: Instant::now(),
    });
    app.add_systems(Update, tick.after(crate::views::hydrate));
    app.add_systems(OnExit(crate::AppPhase::Editor), revoke);
    eprintln!("Editor script explicitly authorized; waiting for project source admission");
    Ok(())
}
fn revoke(world: &mut World) {
    if matches!(
        status(world),
        Some(ExecutionStatus::Queued | ExecutionStatus::Running)
    ) {
        let _ = cancel(world);
    }
}
fn tick(world: &mut World) {
    if !world.contains_resource::<Execution>() {
        return;
    }
    world.resource_scope(|world, mut execution: Mut<Execution>| {
        if !matches!(
            execution.status,
            ExecutionStatus::Queued | ExecutionStatus::Running
        ) {
            return;
        }
        let project = world
            .get_resource::<crate::model::ProjectManager>()
            .and_then(|manager| manager.project.clone());
        if let Some(project) = &project
            && project.root != execution.project
        {
            if let Some(run) = execution.run {
                let _ = kedra_script::cancel(world, run);
            }
            execution.status = ExecutionStatus::Cancelled;
            execution.program = None;
            return;
        }
        let result = (|| -> Result<(), String> {
            if let Some(run) = execution.run {
                match kedra_script::status(world, run) {
                    Some(Status::Running) => (),
                    Some(Status::Completed(value)) => {
                        execution.status = ExecutionStatus::Completed(value.clone());
                        crate::workspace::editor_script_feedback(
                            world,
                            "Editor script completed. Accepted changes remain unsaved.",
                        );
                        eprintln!(
                            "SUCCESS: editor script completed; Save remains a separate action"
                        );
                    }
                    Some(Status::Failed(error)) => return Err(error.to_string()),
                    _ => execution.status = ExecutionStatus::Cancelled,
                }
                return Ok(());
            }
            if execution.since.elapsed().as_secs() > 120 {
                return Err("Editor script source admission timed out".into());
            }
            let Some(project) = project else {
                return Ok(());
            };
            if !world.contains_resource::<crate::workspace::EditorWorkspace>() {
                return Ok(());
            }
            let ready = world.resource_scope(
                |world,
                 mut workspace: Mut<crate::workspace::EditorWorkspace>|
                 -> Result<bool, String> {
                    if !execution.requested {
                        workspace.documents.request(
                            world,
                            project.assets,
                            execution.first.clone(),
                        )?;
                        execution.requested = true;
                        return Ok(false);
                    }
                    if workspace.documents.opening() {
                        return Ok(false);
                    }
                    if !workspace.documents.source_ready() {
                        return Err(workspace
                            .documents
                            .failure
                            .clone()
                            .or_else(|| workspace.documents.error.clone())
                            .unwrap_or_else(|| {
                                "Editor script source workspace unavailable".into()
                            }));
                    }
                    Ok(true)
                },
            )?;
            if !ready {
                return Ok(());
            }
            let program = execution
                .program
                .take()
                .ok_or("Editor script invocation was already consumed")?;
            execution.run = Some(
                kedra_script::start(world, program, serde_json::Value::Null)
                    .map_err(|error| error.to_string())?,
            );
            execution.status = ExecutionStatus::Running;
            crate::workspace::editor_script_feedback(world, "Running editor script…");
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("FAILED: editor script: {error}");
            crate::workspace::editor_script_feedback(
                world,
                &format!("Editor script failed: {error}"),
            );
            execution.program = None;
            execution.status = ExecutionStatus::Failed(error);
        }
    });
}
