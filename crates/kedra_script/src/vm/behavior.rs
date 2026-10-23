//! Persistent, per-instance state with strictly non-yielding lifecycle callbacks.
use super::*;

pub(super) struct Behavior {
    ctx: lua::Table,
    state: lua::Table,
    start: Option<lua::Function>,
    update: Option<lua::Function>,
    fixed_update: Option<lua::Function>,
    stop: Option<lua::Function>,
    started: bool,
    stopped: bool,
    scope: Scope,
}
impl Behavior {
    pub fn new(lua: &lua::Lua, ctx: lua::Table, scope: Scope) -> lua::Result<Self> {
        let globals = lua.globals();
        let behavior = Self {
            ctx,
            state: lua.create_table()?,
            start: globals.get("start")?,
            update: globals.get("update")?,
            fixed_update: globals.get("fixed_update")?,
            stop: globals.get("stop")?,
            started: false,
            stopped: false,
            scope,
        };
        if behavior.start.is_none()
            && behavior.update.is_none()
            && behavior.fixed_update.is_none()
            && behavior.stop.is_none()
        {
            return Err(lua::Error::runtime(
                "Behavior declares no lifecycle callbacks",
            ));
        }
        Ok(behavior)
    }

    pub fn dispatch(
        &mut self,
        lua: &lua::Lua,
        phase: &str,
        args: Vec<ScriptValue>,
    ) -> lua::Result<()> {
        if phase == "stop" {
            if self.started && !self.stopped {
                self.stopped = true;
                let reason = match args.first() {
                    Some(ScriptValue::String(reason)) => reason.as_ref(),
                    _ => return Err(lua::Error::runtime("Missing behavior stop reason")),
                };
                if let Some(function) = &self.stop {
                    invoke(
                        lua,
                        function,
                        (self.state.clone(), self.ctx.clone(), reason),
                    )?;
                }
            }
            return Ok(());
        }
        if self.stopped {
            return Err(lua::Error::runtime("Behavior has stopped"));
        }
        if !self.started {
            self.started = true;
            if let Some(function) = &self.start {
                invoke(lua, function, (self.state.clone(), self.ctx.clone()))?;
            }
            // start may synchronously cancel this run or despawn its parent.
            // Do not invoke update after revocation, even for pure Lua effects.
            self.scope.with_world(|_| Ok(()))?;
        }
        let dt = match args.first() {
            Some(ScriptValue::Float(dt)) => *dt,
            _ => return Err(lua::Error::runtime("Missing behavior timestep")),
        };
        let function = match phase {
            "update" => &self.update,
            "fixed_update" => &self.fixed_update,
            _ => return Err(lua::Error::runtime("Unknown behavior callback")),
        };
        if let Some(function) = function {
            invoke(lua, function, (self.state.clone(), self.ctx.clone(), dt))?;
        }
        Ok(())
    }
}

fn invoke(
    lua: &lua::Lua,
    function: &lua::Function,
    args: impl lua::IntoLuaMulti,
) -> lua::Result<()> {
    // A fresh coroutine makes the interrupt uncatchable by script pcall. It is
    // never retained: even a valid wait is an error in a lifecycle callback.
    let thread = lua.create_thread(function.clone())?;
    thread.resume::<lua::Value>(args)?;
    if thread.status() != lua::ThreadStatus::Finished {
        return Err(lua::Error::runtime(
            "Behavior callbacks cannot yield; use a sequence",
        ));
    }
    Ok(())
}
