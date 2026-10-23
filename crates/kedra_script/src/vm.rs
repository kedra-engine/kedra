//! BMS context initialization and bounded dispatch of explicit entrypoint kinds.
mod behavior;
mod sequence;

use super::*;
use bevy_mod_scripting::{
    bindings::{InteropError, ScriptValue},
    core::callbacks::ScriptCallbacks,
    lua::{IntoInteropError, LuaContext},
    prelude::ScriptAttachment,
};
use lua::VmState;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) fn world_error(error: impl std::fmt::Debug) -> lua::Error {
    lua::Error::runtime(format!("{error:?}"))
}

struct Budget {
    start: Instant,
    limit: Duration,
    exhausted: Arc<AtomicBool>,
}

pub(super) fn initialize(
    attachment: &ScriptAttachment,
    context: &mut LuaContext,
) -> Result<(), InteropError> {
    let mut loading_failure = None;
    let result = (|| -> lua::Result<()> {
        let owner = attachment
            .entity()
            .ok_or_else(|| lua::Error::runtime("Only owned script contexts are supported"))?;
        let (program, arguments, execution, world_id, started, target, mailbox, failure, sink) =
            ThreadWorldContainer
                .try_get_context()
                .map_err(world_error)?
                .world
                .with_world_access(|world| {
                    world.get::<Instance>(owner).map(|i| {
                        (
                            i.program.clone(),
                            i.arguments.clone(),
                            i.execution.clone(),
                            world.id(),
                            i.started,
                            i.target,
                            i.events.clone(),
                            i.error_kind.clone(),
                            world.resource::<Diagnostics>().clone(),
                        )
                    })
                })
                .map_err(world_error)?
                .ok_or_else(|| lua::Error::runtime("Missing script owner"))?;
        loading_failure = Some(failure.clone());
        let exhausted = Arc::new(AtomicBool::new(false));
        context.set_memory_limit(program.limits.memory_bytes)?;
        context.set_app_data(Budget {
            start: Instant::now(),
            limit: program.limits.callback_time,
            exhausted: exhausted.clone(),
        });
        context.set_interrupt(|lua| {
            let budget = lua
                .app_data_ref::<Budget>()
                .ok_or_else(|| lua::Error::runtime("Missing VM budget"))?;
            if budget.start.elapsed() >= budget.limit {
                budget.exhausted.store(true, Ordering::Relaxed);
                Ok(VmState::Yield)
            } else {
                Ok(VmState::Continue)
            }
        });
        modules::install(
            context,
            program.sources.clone(),
            &program.path,
            &program.source,
        )?;
        let runtime = context.create_table()?;
        runtime.set(
            "elapsed_ms",
            context.create_function(move |_, ()| Ok(started.elapsed().as_secs_f64() * 1000.0))?,
        )?;
        runtime.set_readonly(true);
        context.globals().set("runtime", runtime)?;
        let json = context.create_table()?;
        json.set("null", lua::LuaSerdeExt::null(&**context))?;
        json.set_readonly(true);
        context.globals().set("json", json)?;
        let validator: lua::Function = context
            .load(include_str!("values.luau"))
            .set_name("kedra/values.luau")
            .eval()?;
        context.set_named_registry_value("_kedra_validate_value", validator)?;
        context
            .load(include_str!("wait.luau"))
            .set_name("kedra/wait.luau")
            .exec()?;
        let ctx = context.create_table()?;
        if let Some(entity) = target {
            ctx.set(
                "entity",
                EntityHandle {
                    world: world_id,
                    entity,
                },
            )?;
        }
        let scope = Scope {
            run: Run {
                world: world_id,
                owner,
            },
            asset: attachment.script(),
        };
        parameters::install(context, &ctx, &program.params)?;
        diagnostics::install(
            context,
            &ctx,
            sink,
            Record {
                run: scope.run,
                target: target.map(|entity| EntityHandle {
                    world: world_id,
                    entity,
                }),
                program: program.name.clone(),
                path: program.path.clone(),
                source: None,
                line: None,
                phase: "load".into(),
                level: Level::Info,
                error_kind: None,
                message: String::new(),
                fields: Value::Null,
            },
        )?;
        if program.kind == Kind::Behavior {
            modules::sequence_resolver(context, &program)?;
            jobs::install(context, scope.clone(), &ctx)?;
            events::install(context, scope.clone(), &ctx)?;
        }
        (program.bindings)(context, scope.clone(), &ctx)?;
        ctx.set_readonly(true);
        context.sandbox(true)?;
        reset_budget(context);
        context.set_app_data(diagnostics::Phase("load"));
        // BMS 0.23 evaluates unnamed chunks. Evaluate the admitted entry here so
        // standard require uses its real project-relative path, then load an empty
        // BMS chunk. There is no alternative loader or second source evaluation.
        let entry = context
            .load(&program.source)
            .set_name(format!("@{}", program.path))
            .into_function()?;
        let loading = context.create_thread(entry)?;
        loading.resume::<lua::Value>(())?;
        if exhausted.load(Ordering::Relaxed) {
            return Err(diagnostics::fault(
                ErrorKind::Budget,
                "Luau source exceeded time budget",
            ));
        }
        if loading.status() != lua::ThreadStatus::Finished {
            return Err(lua::Error::runtime("Source evaluation cannot yield"));
        }
        *execution.lock() = Execution {
            state: match program.kind {
                Kind::Sequence => {
                    State::Sequence(sequence::Sequence::new(context, ctx, arguments)?)
                }
                Kind::Behavior => State::Behavior(behavior::Behavior::new(context, ctx, scope)?),
            },
            clock: (Clock::default(), None),
            result: None,
            events: Some(mailbox),
        };
        for phase in ["sequence", "update", "fixed_update", "event", "stop"] {
            let weak = Arc::downgrade(&execution);
            let key = attachment.clone();
            let exhausted = exhausted.clone();
            let failure = failure.clone();
            ThreadWorldContainer
                .try_get_context()
                .map_err(world_error)?
                .world
                .with_world_mut_access(|world| {
                    world
                        .resource::<ScriptCallbacks<LuaScriptingPlugin>>()
                        .callbacks
                        .write()
                        .insert(
                            (key, format!("_kedra_{phase}")),
                            Arc::new(move |args, context, _| {
                                let execution = weak
                                    .upgrade()
                                    .ok_or_else(|| InteropError::str("Script owner was retired"))?;
                                reset_budget(context);
                                context.set_app_data(diagnostics::Phase(phase));
                                let result = execution.lock().dispatch(context, phase, args);
                                if exhausted.load(Ordering::Relaxed) {
                                    *failure.lock() = Some(ErrorKind::Budget);
                                    return Err(InteropError::str(
                                        "Luau callback exceeded time budget",
                                    ));
                                }
                                if let Err(error) = &result {
                                    *failure.lock() = Some(diagnostics::classify(error));
                                }
                                result
                                    .map(ScriptValue::Bool)
                                    .map_err(IntoInteropError::to_bms_error)
                            }),
                        );
                })
                .map_err(world_error)?;
        }
        Ok(())
    })();
    if let (Err(error), Some(failure)) = (&result, loading_failure) {
        let kind = diagnostics::classify(error);
        *failure.lock() = Some(if kind == ErrorKind::Runtime {
            ErrorKind::Load
        } else {
            kind
        });
    }
    result.map_err(IntoInteropError::to_bms_error)
}

fn reset_budget(lua: &lua::Lua) {
    if let Some(mut budget) = lua.app_data_mut::<Budget>() {
        budget.start = Instant::now();
        budget.exhausted.store(false, Ordering::Relaxed);
    }
}

enum State {
    Unloaded,
    Sequence(sequence::Sequence),
    Behavior(behavior::Behavior),
}

/// Lua handles stay in native owner state; VM callbacks only hold weak references.
pub(super) struct Execution {
    state: State,
    pub clock: (Clock, Option<f64>),
    pub result: Option<Value>,
    events: Option<Arc<Mutex<events::Mailbox>>>,
}
impl Default for Execution {
    fn default() -> Self {
        Self {
            state: State::Unloaded,
            clock: (Clock::default(), None),
            result: None,
            events: None,
        }
    }
}
impl Execution {
    pub fn ready(&self, clock: Clock, game: Option<f64>) -> bool {
        match &self.state {
            State::Sequence(sequence) => sequence.ready(clock, game),
            _ => true,
        }
    }

    fn dispatch(
        &mut self,
        lua: &lua::Lua,
        phase: &str,
        args: Vec<ScriptValue>,
    ) -> lua::Result<bool> {
        if phase == "event" {
            if let Some(events) = &self.events {
                events::invoke(lua, events)?;
            }
            return Ok(false);
        }
        match &mut self.state {
            State::Sequence(sequence) if phase == "sequence" => {
                let result = sequence.resume(lua, self.clock)?;
                if let Some(value) = result {
                    self.result = Some(value);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            State::Behavior(behavior) => behavior.dispatch(lua, phase, args).map(|()| false),
            _ => Ok(false),
        }
    }
}
