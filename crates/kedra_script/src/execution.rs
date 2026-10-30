//! Explicit context admission, Bevy scheduling and revocable owner lifetime.
use super::*;
use bevy::ecs::{lifecycle::HookContext, world::DeferredWorld};
use bevy_mod_scripting::{
    bindings::{CurrentScriptAttachment, WorldAccessGuard, WorldGuard},
    core::{
        callbacks::ScriptCallbacks,
        context::ScriptingLoader,
        event::CallbackLabel,
        script::{Context, ScriptContexts},
    },
    prelude::*,
};

pub(super) fn start(
    world: &mut World,
    target: Option<Entity>,
    parent: Option<Entity>,
    program: Program,
    arguments: Value,
    replacing: Option<Run>,
) -> Result<Run, ScriptError> {
    if !world.contains_resource::<ScriptContexts<LuaScriptingPlugin>>() {
        return Err(ScriptError::at(
            &program,
            ErrorKind::InvalidInput,
            "admission",
            "Install LuauPlugin before starting a script",
        ));
    }
    if world.resource::<Retiring>().dispatching {
        return Err(ScriptError::at(
            &program,
            ErrorKind::InvalidInput,
            "admission",
            "Schedule script loading after the active native callback returns",
        ));
    }
    let invalid =
        |message: String| ScriptError::at(&program, ErrorKind::InvalidInput, "admission", message);
    if target.is_some_and(|entity| world.get_entity(entity).is_err())
        || parent.is_some_and(|entity| world.get_entity(entity).is_err())
    {
        return Err(ScriptError::at(
            &program,
            ErrorKind::Stale,
            "admission",
            "Script parent or target is unavailable",
        ));
    }
    modules::validate_path(&program.path).map_err(invalid)?;
    values::validate(&program.params).map_err(invalid)?;
    if !program.params.is_object() {
        return Err(invalid("Script parameters must be an object".into()));
    }
    (program.validate_params)(&program.params).map_err(invalid)?;
    if program.sequences.len() > 32
        || (program.kind == Kind::Sequence
            && (!program.sequences.is_empty() || !program.events.is_empty()))
        || program
            .sequences
            .iter()
            .any(|child| child.kind != Kind::Sequence || !child.sequences.is_empty())
        || program
            .sequences
            .iter()
            .map(|child| &child.path)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != program.sequences.len()
        || program
            .events
            .iter()
            .any(|name| name.is_empty() || name.len() > 128 || name.chars().any(char::is_control))
        || program
            .events
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != program.events.len()
    {
        return Err(invalid(
            "Invalid owned-sequence or subsystem-event admission".into(),
        ));
    }
    if program.path.ends_with(".luaurc")
        || program.name.is_empty()
        || program.name.len() > 128
        || program.name.chars().any(char::is_control)
        || program.source.len() > 64 * 1024
        || !(1024 * 1024..=64 * 1024 * 1024).contains(&program.limits.memory_bytes)
        || program.limits.callback_time.is_zero()
        || program.limits.callback_time > Duration::from_secs(1)
        || program
            .limits
            .total_time
            .is_some_and(|time| time.is_zero() || time > Duration::from_secs(600))
        || program.limits.resumes == Some(0)
        || (program.kind == Kind::Sequence
            && (program.limits.total_time.is_none() || program.limits.resumes.is_none()))
    {
        return Err(invalid(
            "Invalid script identity, source size or execution limits".into(),
        ));
    }
    values::validate(&arguments).map_err(invalid)?;
    if program.kind == Kind::Behavior && arguments != Value::Null {
        return Err(invalid(
            "Behavior has lifecycle callbacks, not sequence arguments".into(),
        ));
    }
    let limit = match program.kind {
        Kind::Sequence => world.resource::<Capacity>().sequences,
        Kind::Behavior => world.resource::<Capacity>().behaviors,
    };
    let count = world
        .query::<(Entity, &Instance)>()
        .iter(world)
        .filter(|(owner, i)| {
            i.status == Status::Running
                && i.program.kind == program.kind
                && replacing.is_none_or(|old| old.owner != *owner)
        })
        .count();
    if count >= limit {
        return Err(ScriptError::at(
            &program,
            ErrorKind::Capacity,
            "admission",
            "Script execution capacity exhausted",
        ));
    }
    let asset = world
        .resource_mut::<Assets<ScriptAsset>>()
        .add(ScriptAsset::new(&program.source).with_language(Language::Lua));
    let owner = world
        .spawn(Instance {
            program,
            asset: asset.clone(),
            arguments,
            execution: Arc::new(Mutex::new(vm::Execution::default())),
            target,
            status: Status::Running,
            active: false,
            started: Instant::now(),
            resumes: 0,
            released: false,
            jobs: Vec::new(),
            events: Arc::new(Mutex::new(events::Mailbox::default())),
            error_kind: Arc::new(Mutex::new(None)),
            profiling: profiling::State::default(),
        })
        .id();
    if let Some(parent) = parent {
        world.entity_mut(owner).insert(ChildOf(parent));
    }
    let run = Run {
        world: world.id(),
        owner,
    };
    let attachment = ScriptAttachment::EntityScript(owner, asset);
    let cache = WorldAccessGuard::setup_cache(world, CurrentScriptAttachment::empty());
    // The initializer evaluates the named entry once; BMS's own unnamed chunk is empty.
    let loaded =
        LuaScriptingPlugin::load(&attachment, b"", WorldGuard::new_exclusive(world, cache))
            .and_then(|context| {
                world
                    .resource::<ScriptContexts<LuaScriptingPlugin>>()
                    .write()
                    .insert(
                        attachment,
                        Context::LoadedAndActive(Arc::new(Mutex::new(context))),
                    )
                    .map_err(|_| {
                        bevy_mod_scripting::bindings::InteropError::str(
                            "Context policy rejected script owner",
                        )
                    })
            });
    if let Err(error) = loaded {
        let instance = world.get::<Instance>(owner).ok_or_else(|| {
            ScriptError::new(ErrorKind::Stale, "Script owner retired during load")
        })?;
        let kind = instance.error_kind.lock().unwrap_or(ErrorKind::Load);
        let error = ScriptError::at(&instance.program, kind, "load", format!("{error:?}"));
        report(world, run, &error);
        world.despawn(owner);
        flush(world);
        return Err(error);
    }
    Ok(run)
}

pub(super) fn cancel(world: &mut World, run: Run) -> Result<(), ScriptError> {
    finish(world, run, Status::Cancelled, "cancelled")?;
    flush(world);
    Ok(())
}

pub(super) struct Retirement {
    owner: Entity,
    asset: Handle<ScriptAsset>,
    // Keep VM state alive until deferred stop notification has been dispatched.
    _execution: Arc<Mutex<vm::Execution>>,
    reason: &'static str,
    notify: bool,
}

pub(super) fn finish(
    world: &mut World,
    run: Run,
    status: Status,
    reason: &'static str,
) -> Result<(), ScriptError> {
    if world.id() != run.world {
        return Err(ScriptError::new(
            ErrorKind::Stale,
            "Stale or foreign script run",
        ));
    }
    let Some(mut instance) = world.get_mut::<Instance>(run.owner) else {
        return Err(ScriptError::new(ErrorKind::Stale, "Stale script run"));
    };
    if instance.status == Status::Running {
        let failure = match &status {
            Status::Failed(error) => Some(error.clone()),
            _ => None,
        };
        instance.status = status;
        instance.profiling.cancel_operations();
        if let Some(error) = failure {
            report(world, run, &error);
        }
    }
    let mut instance = world
        .get_mut::<Instance>(run.owner)
        .ok_or_else(|| ScriptError::new(ErrorKind::Stale, "Stale script run"))?;
    if instance.released {
        return Ok(());
    }
    instance.released = true;
    let jobs = instance.jobs.clone();
    let retirement = Retirement {
        owner: run.owner,
        asset: instance.asset.clone(),
        _execution: instance.execution.clone(),
        reason,
        notify: instance.program.kind == Kind::Behavior,
    };
    world.resource_mut::<Retiring>().pending.push(retirement);
    jobs::revoke(world, &jobs);
    Ok(())
}

pub(super) fn removed(mut world: DeferredWorld, hook: HookContext) {
    let Some(instance) = world.get::<Instance>(hook.entity) else {
        return;
    };
    if instance.released {
        return;
    }
    let retirement = Retirement {
        owner: hook.entity,
        asset: instance.asset.clone(),
        _execution: instance.execution.clone(),
        reason: "despawned",
        notify: instance.program.kind == Kind::Behavior,
    };
    // Revocation is immediate through owner removal. Never re-enter BMS locks from
    // a component hook or a currently running native binding.
    world.resource_mut::<Retiring>().pending.push(retirement);
}

pub(super) fn flush(world: &mut World) {
    if world.resource::<Retiring>().dispatching {
        return;
    }
    for retired in std::mem::take(&mut world.resource_mut::<Retiring>().pending) {
        let attachment = ScriptAttachment::EntityScript(retired.owner, retired.asset.clone());
        let present = world
            .resource::<ScriptCallbacks<LuaScriptingPlugin>>()
            .callbacks
            .read()
            .contains_key(&(attachment.clone(), "_kedra_stop".into()));
        if retired.notify && present {
            let outcome = dispatch(
                world,
                attachment.clone(),
                "stop",
                vec![ScriptValue::String(retired.reason.into())],
            );
            if let Err(error) = outcome {
                if let Some(instance) = world.get::<Instance>(retired.owner) {
                    let error = ScriptError::at(
                        &instance.program,
                        instance.error_kind.lock().unwrap_or(ErrorKind::Runtime),
                        "stop",
                        error,
                    );
                    report(
                        world,
                        Run {
                            world: world.id(),
                            owner: retired.owner,
                        },
                        &error,
                    );
                } else {
                    eprintln!("Behavior stop failed: {error}");
                }
            }
        }
        *retired._execution.lock() = vm::Execution::default();
        if let Some(instance) = world.get::<Instance>(retired.owner) {
            *instance.events.lock() = events::Mailbox::default();
        }
        world
            .resource::<ScriptCallbacks<LuaScriptingPlugin>>()
            .callbacks
            .write()
            .retain(|(key, _), _| key != &attachment);
        world
            .resource::<ScriptContexts<LuaScriptingPlugin>>()
            .write()
            .remove(&attachment);
        world
            .resource_mut::<Assets<ScriptAsset>>()
            .remove(retired.asset.id());
    }
}

fn dispatch(
    world: &mut World,
    attachment: ScriptAttachment,
    phase: &str,
    args: Vec<ScriptValue>,
) -> Result<ScriptValue, BevyError> {
    let label =
        CallbackLabel::new(&format!("_kedra_{phase}")).ok_or("Invalid internal callback label")?;
    world.resource_mut::<Retiring>().dispatching = true;
    let outcome =
        RunScriptCallback::<LuaScriptingPlugin>::new(attachment, label, args, false).apply(world);
    world.resource_mut::<Retiring>().dispatching = false;
    outcome
}

pub(super) fn update(world: &mut World) {
    let dt = world
        .get_resource::<Time<Virtual>>()
        .map_or(0.0, Time::delta_secs_f64);
    step_kind(world, Kind::Behavior, "update", dt);
}
pub(super) fn fixed_update(world: &mut World) {
    let dt = world
        .get_resource::<Time<Fixed>>()
        .map_or(0.0, Time::delta_secs_f64);
    step_kind(world, Kind::Behavior, "fixed_update", dt);
}
pub(super) fn step(world: &mut World) {
    events::deliver(world);
    jobs::admit(world);
    step_kind(world, Kind::Sequence, "sequence", 0.0);
    jobs::collect(world);
    // Script clocks and behavior callbacks need ordinary updates. Until native
    // deadline seams cover every wait kind, preserve their existing cadence in
    // one runtime-owned integration, not in individual scripts or editor code.
    if world
        .query::<&Instance>()
        .iter(world)
        .any(|instance| instance.status == Status::Running)
    {
        world.resource_mut::<kedra_wakeup::WakeDemand>().active();
    }
}

fn step_kind(world: &mut World, kind: Kind, phase: &str, dt: f64) {
    flush(world);
    let world_id = world.id();
    let owners: Vec<_> = world
        .query::<(Entity, &Instance)>()
        .iter(world)
        .filter(|(_, i)| i.status == Status::Running && i.program.kind == kind)
        .map(|(owner, _)| owner)
        .collect();
    for owner in owners {
        let args = if kind == Kind::Behavior {
            vec![ScriptValue::Float(dt)]
        } else {
            vec![]
        };
        run_phase(
            world,
            Run {
                world: world_id,
                owner,
            },
            phase,
            args,
        );
    }
    flush(world);
}

pub(super) fn run_phase(world: &mut World, run: Run, phase: &str, args: Vec<ScriptValue>) {
    let owner = run.owner;
    let clock = *world.resource::<Clock>();
    let game_time = world
        .get_resource::<Time<Virtual>>()
        .map(Time::elapsed_secs_f64);
    let Some(mut instance) = world.get_mut::<Instance>(owner) else {
        return;
    };
    if instance.status != Status::Running {
        return;
    }
    if instance
        .program
        .limits
        .total_time
        .is_some_and(|time| instance.started.elapsed() >= time)
        || instance
            .program
            .limits
            .resumes
            .is_some_and(|limit| instance.resumes >= limit)
    {
        let error = ScriptError::at(
            &instance.program,
            ErrorKind::Budget,
            phase,
            "Total execution budget exhausted",
        );
        let _ = finish(world, run, Status::Failed(error), "budget_exhausted");
        return;
    }
    if phase == "sequence" && !instance.execution.lock().ready(clock, game_time) {
        return;
    }
    instance.resumes = instance.resumes.saturating_add(1);
    instance.active = true;
    *instance.error_kind.lock() = None;
    instance.execution.lock().clock = (clock, game_time);
    let attachment = ScriptAttachment::EntityScript(owner, instance.asset.clone());
    let outcome = dispatch(world, attachment, phase, args);
    if let Some(mut instance) = world.get_mut::<Instance>(owner) {
        instance.active = false;
        if instance.status != Status::Running {
            return;
        }
        let terminal = match outcome {
            Ok(ScriptValue::Bool(false)) => None,
            Ok(ScriptValue::Bool(true)) => Some(Status::Completed(
                instance
                    .execution
                    .lock()
                    .result
                    .take()
                    .unwrap_or(Value::Null),
            )),
            Ok(_) => Some(Status::Failed(ScriptError::at(
                &instance.program,
                ErrorKind::Runtime,
                phase,
                "Invalid script dispatch response",
            ))),
            Err(error) => Some(Status::Failed(ScriptError::at(
                &instance.program,
                instance.error_kind.lock().unwrap_or(ErrorKind::Runtime),
                phase,
                error,
            ))),
        };
        if let Some(status) = terminal {
            let reason = if matches!(status, Status::Failed(_)) {
                "failed"
            } else {
                "completed"
            };
            let _ = finish(world, run, status, reason);
        }
    }
}

fn report(world: &World, run: Run, error: &ScriptError) {
    if let Some(instance) = world.get::<Instance>(run.owner) {
        world.resource::<Diagnostics>().push(Record {
            run,
            target: instance.target.map(|entity| EntityHandle {
                world: world.id(),
                entity,
            }),
            program: instance.program.name.clone(),
            path: instance.program.path.clone(),
            source: None,
            line: None,
            phase: error.phase.clone().unwrap_or_default(),
            level: Level::Error,
            error_kind: Some(error.kind),
            message: error.message.clone(),
            fields: Value::Null,
        });
    }
}
