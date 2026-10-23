//! Bounded owner-bound sequence admission after native callback dispatch.
use crate::{diagnostics::fault, *};

pub(crate) enum Job {
    Pending(Program, Value),
    Active(Run),
    Terminal(Status),
}

struct Handle {
    scope: Scope,
    job: Arc<Mutex<Job>>,
}
impl lua::UserData for Handle {}

pub(crate) fn install(lua: &lua::Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    let api = lua.create_table()?;
    let start_scope = scope.clone();
    api.set(
        "start",
        lua.create_function(move |lua, (path, args): (String, Option<lua::Value>)| {
            // Use Luau's resolver without evaluating the selected entry. Its cache is
            // namespaced separately from module exports; no custom import parser.
            let path: String = lua
                .named_registry_value::<lua::Function>("_kedra_sequence_resolver")?
                .call(path)
                .map_err(|error| fault(ErrorKind::Permission, error))?;
            let args = args
                .map(|value| values::from_lua(lua, value))
                .transpose()?
                .unwrap_or(Value::Null);
            start_scope.with_world(|world| {
                let mut instance = world
                    .get_mut::<Instance>(start_scope.run.owner)
                    .ok_or_else(|| fault(ErrorKind::Stale, "Sequence owner is retired"))?;
                instance.jobs.retain(|job| {
                    !matches!(*job.lock(), Job::Terminal(_)) || Arc::strong_count(job) > 1
                });
                if instance.jobs.len() >= 32 {
                    return Err(fault(
                        ErrorKind::Capacity,
                        "Owned sequence handle capacity exhausted",
                    ));
                }
                let program = instance
                    .program
                    .sequences
                    .iter()
                    .find(|program| program.path == path)
                    .cloned()
                    .ok_or_else(|| {
                        fault(
                            ErrorKind::Permission,
                            "Sequence is not authorized by this host",
                        )
                    })?;
                let job = Arc::new(Mutex::new(Job::Pending(program, args)));
                instance.jobs.push(job.clone());
                Ok(Handle {
                    scope: start_scope.clone(),
                    job,
                })
            })
        })?,
    )?;
    let status_scope = scope.clone();
    api.set(
        "status",
        lua.create_function(move |lua, handle: lua::AnyUserData| {
            let handle = handle.borrow::<Handle>()?;
            validate(&status_scope, &handle)?;
            status_scope.with_world(|_| Ok(()))?;
            let value = match &*handle.job.lock() {
                Job::Pending(..) => serde_json::json!({"state":"queued"}),
                Job::Active(_) => serde_json::json!({"state":"running"}),
                Job::Terminal(status) => status_value(status),
            };
            values::to_lua(lua, &value)
        })?,
    )?;
    api.set(
        "cancel",
        lua.create_function(move |_, handle: lua::AnyUserData| {
            let handle = handle.borrow::<Handle>()?;
            validate(&scope, &handle)?;
            scope.with_world(|world| {
                let run = cancel_job(&handle.job);
                if let Some(run) = run {
                    crate::retire(world, run).map_err(lua::Error::external)?;
                }
                Ok(())
            })
        })?,
    )?;
    api.set_readonly(true);
    ctx.set("sequences", api)
}

fn validate(scope: &Scope, handle: &Handle) -> lua::Result<()> {
    if scope.run != handle.scope.run {
        return Err(fault(
            ErrorKind::Permission,
            "Sequence handle belongs to another owner",
        ));
    }
    Ok(())
}

fn status_value(status: &Status) -> Value {
    match status {
        Status::Running => serde_json::json!({"state":"running"}),
        Status::Completed(value) => serde_json::json!({"state":"completed","result":value}),
        Status::Cancelled => serde_json::json!({"state":"cancelled"}),
        Status::Failed(error) => {
            serde_json::json!({"state":"failed","error":{"kind":error.kind.code(),"message":error.message,"path":error.path,"phase":error.phase}})
        }
    }
}

fn cancel_job(job: &Arc<Mutex<Job>>) -> Option<Run> {
    let mut job = job.lock();
    let run = match &*job {
        Job::Active(run) => Some(*run),
        Job::Pending(..) => None,
        Job::Terminal(_) => return None,
    };
    *job = Job::Terminal(Status::Cancelled);
    run
}

pub(crate) fn revoke(world: &mut World, jobs: &[Arc<Mutex<Job>>]) {
    for job in jobs {
        if let Some(run) = cancel_job(job) {
            let _ = execution::finish(world, run, Status::Cancelled, "cancelled");
            world.despawn(run.owner);
        }
    }
}

pub(crate) fn admit(world: &mut World) {
    let owners: Vec<_> = world
        .query::<(Entity, &Instance)>()
        .iter(world)
        .filter(|(_, instance)| {
            instance.status == Status::Running && instance.program.kind == Kind::Behavior
        })
        .map(|(owner, instance)| (owner, instance.target, instance.jobs.clone()))
        .collect();
    for (owner, target, jobs) in owners {
        for job in jobs {
            if world
                .get::<Instance>(owner)
                .is_none_or(|instance| instance.status != Status::Running)
            {
                break;
            }
            let candidate = {
                let mut job = job.lock();
                if matches!(*job, Job::Pending(..)) {
                    match std::mem::replace(&mut *job, Job::Terminal(Status::Cancelled)) {
                        Job::Pending(program, args) => Some((program, args)),
                        _ => None,
                    }
                } else {
                    None
                }
            };
            if let Some((program, args)) = candidate {
                *job.lock() =
                    match execution::start(world, target, Some(owner), program, args, None) {
                        Ok(run) => Job::Active(run),
                        Err(error) => Job::Terminal(Status::Failed(error)),
                    };
            }
        }
    }
}

pub(crate) fn collect(world: &mut World) {
    let jobs: Vec<_> = world
        .query::<&Instance>()
        .iter(world)
        .flat_map(|instance| instance.jobs.clone())
        .collect();
    for job in jobs {
        let run = match &*job.lock() {
            Job::Active(run) => Some(*run),
            _ => None,
        };
        if let Some(run) = run {
            let status = crate::status(world, run).cloned();
            if status != Some(Status::Running) {
                *job.lock() = Job::Terminal(status.unwrap_or(Status::Cancelled));
                let _ = crate::retire(world, run);
            }
        }
    }
}
