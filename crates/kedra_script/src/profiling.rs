//! Standard VM instrumentation binding; owner lifetime/revocation stays in this runtime.
//! The independent profiling crate owns instrumentation semantics and the backend.
use crate::{Instance, Scope, lua};
use kedra_profiling::{Counter, Marker, Operation, Unit};
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

#[derive(Default)]
pub(super) struct State {
    markers: BTreeMap<String, Marker>,
    counters: BTreeMap<String, Counter>,
    operation_names: BTreeSet<String>,
    operations: BTreeMap<String, Operation>,
    next_operation: u64,
}
impl State {
    pub(super) fn cancel_operations(&mut self) {
        self.operations.clear();
    }
}

fn label(instance: &Instance, name: &str) -> lua::Result<String> {
    if name.is_empty() || name.len() > 96 || name.chars().any(char::is_control) {
        return Err(lua::Error::runtime(
            "Profiling labels require 1..=96 UTF-8 bytes without control characters",
        ));
    }
    Ok(format!("luau.{}::{name}", instance.program.name))
}

pub(super) fn install(lua: &lua::Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    let api = lua.create_table()?;
    let marker_scope = scope.clone();
    api.set(
        "scope",
        lua.create_function(
            move |_,
                  (name, function, arguments): (
                String,
                lua::Function,
                lua::Variadic<lua::Value>,
            )| {
                let guard = marker_scope.with_world(|world| {
                    let mut instance = world
                        .get_mut::<Instance>(marker_scope.run.owner)
                        .ok_or_else(|| lua::Error::runtime("Script profiling owner was retired"))?;
                    let name = label(&instance, &name)?;
                    if !instance.profiling.markers.contains_key(&name) {
                        if instance.profiling.markers.len() >= 64 {
                            return Err(lua::Error::runtime(
                                "Script CPU marker definition capacity reached",
                            ));
                        }
                        let marker = Marker::named(name.clone())
                            .map_err(lua::Error::external)?
                            .with_category(kedra_profiling::Category::Scripting);
                        instance.profiling.markers.insert(name.clone(), marker);
                    }
                    Ok(instance
                        .profiling
                        .markers
                        .get(&name)
                        .ok_or_else(|| lua::Error::runtime("Script CPU marker missing"))?
                        .enter())
                })?;
                // Native call deliberately forbids yielding across this CPU scope. The
                // separate operation helper supports yielding on a logical-operation track.
                let result = function.call::<lua::MultiValue>(arguments);
                drop(guard);
                result
            },
        )?,
    )?;

    let counter_scope = scope.clone();
    api.set(
        "counter",
        lua.create_function(
            move |_, (name, value, unit): (String, f64, Option<String>)| {
                let unit = match unit.as_deref().unwrap_or("count") {
                    "count" => Unit::Count,
                    "bytes" => Unit::Bytes,
                    "ms" => Unit::Milliseconds,
                    "percent" => Unit::Percent,
                    _ => {
                        return Err(lua::Error::runtime(
                            "Expected profiling unit count, bytes, ms or percent",
                        ));
                    }
                };
                if !value.is_finite() {
                    return Err(lua::Error::runtime(
                        "Profiling counters require finite values",
                    ));
                }
                counter_scope.with_world(|world| {
                    let mut instance = world
                        .get_mut::<Instance>(counter_scope.run.owner)
                        .ok_or_else(|| lua::Error::runtime("Script profiling owner was retired"))?;
                    let name = label(&instance, &name)?;
                    // Gauges from two VMs of the same program must not overwrite each
                    // other. This is a process-local diagnostic suffix, not a persisted ID.
                    let mut identity = std::collections::hash_map::DefaultHasher::new();
                    counter_scope.run.world.hash(&mut identity);
                    counter_scope.run.owner.hash(&mut identity);
                    let name = format!("{name}@{:016x}", identity.finish());
                    if let Some(counter) = instance.profiling.counters.get(&name) {
                        if counter.unit() != unit {
                            return Err(lua::Error::runtime(
                                "A profiling counter cannot change its unit",
                            ));
                        }
                        return counter.sample(value).map_err(lua::Error::external);
                    }
                    if instance.profiling.counters.len() >= 64 {
                        return Err(lua::Error::runtime(
                            "Script counter definition capacity reached",
                        ));
                    }
                    let counter = Counter::new(name.clone(), unit)
                        .map_err(lua::Error::external)?
                        .with_category(kedra_profiling::Category::Scripting);
                    counter.sample(value).map_err(lua::Error::external)?;
                    instance.profiling.counters.insert(name, counter);
                    Ok(())
                })
            },
        )?,
    )?;

    let begin_scope = scope.clone();
    let begin = lua.create_function(move |_, name: String| {
        begin_scope.with_world(|world| {
            let mut instance = world
                .get_mut::<Instance>(begin_scope.run.owner)
                .ok_or_else(|| lua::Error::runtime("Script profiling owner was retired"))?;
            let name = label(&instance, &name)?;
            if instance.profiling.operations.len() >= 32
                || (!instance.profiling.operation_names.contains(&name)
                    && instance.profiling.operation_names.len() >= 64)
            {
                return Err(lua::Error::runtime(
                    "Script logical operation capacity reached",
                ));
            }
            let next = instance
                .profiling
                .next_operation
                .checked_add(1)
                .ok_or_else(|| {
                    lua::Error::runtime("Script profiling operation identity exhausted")
                })?;
            let operation = Operation::begin_in(&name, kedra_profiling::Category::Scripting)
                .map_err(lua::Error::external)?;
            let token = next.to_string();
            instance.profiling.next_operation = next;
            instance.profiling.operation_names.insert(name);
            instance
                .profiling
                .operations
                .insert(token.clone(), operation);
            Ok(token)
        })
    })?;
    let end = lua.create_function(move |_, (token, completed): (String, bool)| {
        scope.with_world(|world| {
            let mut instance = world
                .get_mut::<Instance>(scope.run.owner)
                .ok_or_else(|| lua::Error::runtime("Script profiling owner was retired"))?;
            let operation = instance
                .profiling
                .operations
                .remove(&token)
                .ok_or_else(|| lua::Error::runtime("Stale or foreign profiling operation"))?;
            if completed {
                operation.finish();
            }
            Ok(())
        })
    })?;
    let wrapper: lua::Function = lua
        .load(include_str!("profiling.luau"))
        .set_name("kedra/profiling.luau")
        .eval()?;
    api.set("operation", wrapper.call::<lua::Function>((begin, end))?)?;
    api.set_readonly(true);
    ctx.set("profiler", api)
}
