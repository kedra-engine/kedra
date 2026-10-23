//! Owner-bound subscriptions to host-selected subsystem notifications.
//!
//! Native Bevy observers call notify; Lua cannot publish global string events.
//! Notifications are owned data, queued without entering a VM, and delivered in
//! Last before child admission and sequence resumption.
use crate::{diagnostics::fault, *};
use std::collections::{BTreeMap, VecDeque};

struct Handler {
    name: &'static str,
    callback: lua::Function,
}
struct Delivery {
    id: u64,
    payload: Value,
    bytes: usize,
    serial: u64,
}

#[derive(Default)]
pub(crate) struct Mailbox {
    handlers: BTreeMap<u64, Handler>,
    queue: VecDeque<Delivery>,
    next: u64,
    bytes: usize,
    overflow: bool,
    serial: u64,
    pub current: Option<(lua::Function, Value)>,
}

/// Queue one curated native notification for current subscriptions on this target.
/// No Lua callback runs here. Payload limits use the ordinary JSON boundary.
/// Each owner has 32 subscriptions and 128 queued deliveries / 64 KiB; overflow
/// fails that owner at the next delivery boundary instead of silently losing data.
/// Notifications without subscribers are not retained for future subscriptions.
pub fn notify(
    world: &mut World,
    target: Entity,
    name: &'static str,
    payload: Value,
) -> Result<(), ScriptError> {
    values::validate(&payload).map_err(|error| ScriptError::new(ErrorKind::InvalidInput, error))?;
    let bytes = serde_json::to_vec(&payload)
        .map_err(|error| ScriptError::new(ErrorKind::InvalidInput, error))?
        .len();
    let mut overflow = false;
    for instance in world.query::<&Instance>().iter(world) {
        if instance.target != Some(target) || instance.status != Status::Running {
            continue;
        }
        let mut mailbox = instance.events.lock();
        let ids: Vec<_> = mailbox
            .handlers
            .iter()
            .filter_map(|(id, handler)| (handler.name == name).then_some(*id))
            .collect();
        for id in ids {
            if mailbox.queue.len() >= 128 || mailbox.bytes + bytes > 64 * 1024 {
                mailbox.overflow = true;
                overflow = true;
                break;
            }
            let Some(serial) = mailbox.serial.checked_add(1) else {
                mailbox.overflow = true;
                overflow = true;
                break;
            };
            mailbox.serial = serial;
            mailbox.queue.push_back(Delivery {
                id,
                payload: payload.clone(),
                bytes,
                serial,
            });
            mailbox.bytes += bytes;
        }
    }
    if overflow {
        Err(ScriptError::new(
            ErrorKind::Budget,
            "Script event queue overflow",
        ))
    } else {
        Ok(())
    }
}

struct Connection {
    scope: Scope,
    id: u64,
}
impl lua::UserData for Connection {
    fn add_methods<M: lua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("disconnect", |_, connection, ()| {
            connection.scope.with_world(|world| {
                let mut mailbox = world
                    .get::<Instance>(connection.scope.run.owner)
                    .ok_or_else(|| fault(ErrorKind::Stale, "Subscription owner is retired"))?
                    .events
                    .lock();
                mailbox.handlers.remove(&connection.id);
                let mut removed_bytes = 0;
                mailbox.queue.retain(|delivery| {
                    if delivery.id == connection.id {
                        removed_bytes += delivery.bytes;
                        false
                    } else {
                        true
                    }
                });
                mailbox.bytes -= removed_bytes;
                Ok(())
            })
        });
    }
}

pub(crate) fn install(lua: &lua::Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    let api = lua.create_table()?;
    api.set(
        "on",
        lua.create_function(
            move |_, (target, name, callback): (lua::AnyUserData, String, lua::Function)| {
                let target = *target.borrow::<EntityHandle>()?;
                scope.with_world(|world| {
                    scope.resolve_entity(world, &target)?;
                    let instance = world
                        .get::<Instance>(scope.run.owner)
                        .ok_or_else(|| fault(ErrorKind::Stale, "Subscription owner is retired"))?;
                    let name = instance
                        .program
                        .events
                        .iter()
                        .copied()
                        .find(|allowed| *allowed == name)
                        .ok_or_else(|| {
                            fault(ErrorKind::Permission, "Subsystem event is not authorized")
                        })?;
                    let mut mailbox = instance.events.lock();
                    if mailbox.handlers.len() >= 32 {
                        return Err(fault(
                            ErrorKind::Capacity,
                            "Script subscription capacity exhausted",
                        ));
                    }
                    mailbox.next = mailbox.next.checked_add(1).ok_or_else(|| {
                        fault(ErrorKind::Capacity, "Subscription identities exhausted")
                    })?;
                    let id = mailbox.next;
                    mailbox.handlers.insert(id, Handler { name, callback });
                    Ok(Connection {
                        scope: scope.clone(),
                        id,
                    })
                })
            },
        )?,
    )?;
    api.set_readonly(true);
    ctx.set("events", api)
}

pub(crate) fn deliver(world: &mut World) {
    // Snapshot delivery counts first: reentrant native notifications wait for a
    // later application update, even when another owner has not dispatched yet.
    let owners: Vec<_> = world
        .query::<(Entity, &Instance)>()
        .iter(world)
        .filter(|(_, instance)| instance.status == Status::Running)
        .map(|(owner, instance)| {
            let mailbox = instance.events.lock();
            (owner, mailbox.queue.len().min(16), mailbox.serial)
        })
        .collect();
    for (owner, count, boundary) in owners {
        let run = Run {
            world: world.id(),
            owner,
        };
        let Some(instance) = world.get::<Instance>(owner) else {
            continue;
        };
        if instance.events.lock().overflow {
            let error = ScriptError::at(
                &instance.program,
                ErrorKind::Budget,
                "event",
                "Script event queue overflow",
            );
            let _ = execution::finish(world, run, Status::Failed(error), "budget_exhausted");
            continue;
        }
        for _ in 0..count {
            let Some(instance) = world.get::<Instance>(owner) else {
                break;
            };
            if instance.status != Status::Running {
                break;
            }
            let prepared = {
                let mut mailbox = instance.events.lock();
                if mailbox
                    .queue
                    .front()
                    .is_none_or(|delivery| delivery.serial > boundary)
                {
                    break;
                }
                if let Some(delivery) = mailbox.queue.pop_front() {
                    mailbox.bytes -= delivery.bytes;
                    mailbox.current = mailbox
                        .handlers
                        .get(&delivery.id)
                        .map(|handler| (handler.callback.clone(), delivery.payload));
                }
                mailbox.current.is_some()
            };
            if prepared {
                execution::run_phase(world, run, "event", vec![]);
            }
        }
    }
}

pub(crate) fn invoke(lua: &lua::Lua, mailbox: &Arc<Mutex<Mailbox>>) -> lua::Result<()> {
    let current = mailbox.lock().current.take();
    if let Some((function, payload)) = current {
        let thread = lua.create_thread(function)?;
        thread.resume::<lua::Value>(values::to_lua(lua, &payload)?)?;
        if thread.status() != lua::ThreadStatus::Finished {
            return Err(fault(
                ErrorKind::Runtime,
                "Event handlers cannot yield; start a sequence",
            ));
        }
    }
    Ok(())
}
