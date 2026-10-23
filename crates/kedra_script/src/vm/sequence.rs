//! One finite coroutine; waits observe Bevy clocks without defining a scheduler.
use super::*;
use lua::ThreadStatus;

enum Wait {
    Updates(u64),
    Fixed(u64),
    Game(f64),
    Real(Instant),
}

pub(super) struct Sequence {
    thread: lua::Thread,
    ctx: lua::Table,
    arguments: Value,
    first: bool,
    wait: Option<Wait>,
}
impl Sequence {
    pub fn new(lua: &lua::Lua, ctx: lua::Table, arguments: Value) -> lua::Result<Self> {
        Ok(Self {
            thread: lua.create_thread(lua.globals().get::<lua::Function>("run")?)?,
            ctx,
            arguments,
            first: true,
            wait: None,
        })
    }

    pub fn ready(&self, clock: Clock, game: Option<f64>) -> bool {
        match self.wait {
            None => true,
            Some(Wait::Updates(target)) => clock.updates >= target,
            Some(Wait::Fixed(target)) => clock.fixed >= target,
            Some(Wait::Game(target)) => game.is_some_and(|time| time >= target),
            Some(Wait::Real(target)) => Instant::now() >= target,
        }
    }

    pub fn resume(
        &mut self,
        lua: &lua::Lua,
        clock: (Clock, Option<f64>),
    ) -> lua::Result<Option<Value>> {
        let value: lua::Value = if self.first {
            self.first = false;
            self.thread
                .resume((self.ctx.clone(), values::to_lua(lua, &self.arguments)?))?
        } else {
            self.thread.resume(())?
        };
        if self.thread.status() == ThreadStatus::Finished {
            return Ok(Some(if value == lua::Value::Nil {
                Value::Null
            } else {
                values::from_lua(lua, value)?
            }));
        }
        let token: lua::Table = lua.unpack(value)?;
        let kind: String = token.get("kind")?;
        let n: f64 = token.get("value")?;
        if !n.is_finite() || n <= 0.0 || n > 1_000_000.0 {
            return Err(lua::Error::runtime("Invalid wait duration"));
        }
        self.wait = Some(match kind.as_str() {
            "updates" | "fixed" if n.fract() == 0.0 => {
                if kind == "updates" {
                    Wait::Updates(clock.0.updates.saturating_add(n as u64))
                } else {
                    Wait::Fixed(clock.0.fixed.saturating_add(n as u64))
                }
            }
            "game" => Wait::Game(
                clock
                    .1
                    .ok_or_else(|| lua::Error::runtime("Game clock is not installed"))?
                    + n,
            ),
            "real" => Wait::Real(Instant::now() + Duration::from_secs_f64(n)),
            _ => return Err(lua::Error::runtime("Unsupported script wait token")),
        });
        Ok(None)
    }
}
