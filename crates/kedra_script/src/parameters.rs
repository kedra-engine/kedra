//! Host-validated immutable parameter snapshots, separate from sequence arguments.
use crate::{lua, values};
use serde_json::Value;

/// Host-owned typed/domain validation, performed before allocating an execution.
/// Deserialize into the host's normal serde types and enforce domain constraints;
/// this is not a parallel property/schema registry.
pub type ParameterValidator = fn(&Value) -> Result<(), String>;

pub(crate) fn empty(value: &Value) -> Result<(), String> {
    if value.as_object().is_some_and(|value| value.is_empty()) {
        Ok(())
    } else {
        Err("Nonempty parameters require a host parameter validator".into())
    }
}

pub(crate) fn install(lua: &lua::Lua, ctx: &lua::Table, value: &Value) -> lua::Result<()> {
    let value = values::to_lua(lua, value)?;
    fn freeze(value: lua::Value) -> lua::Result<()> {
        if let lua::Value::Table(table) = value {
            for pair in table.clone().pairs::<lua::Value, lua::Value>() {
                let (_, child) = pair?;
                freeze(child)?;
            }
            table.set_readonly(true);
        }
        Ok(())
    }
    freeze(value.clone())?;
    ctx.set("params", value)
}
