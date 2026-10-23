//! Bounded, lossless JSON-compatible values at the scripting boundary.
use crate::lua::{self, LuaSerdeExt};
use serde_json::Value;

/// Validate depth/size and reject integers which Luau's double cannot represent.
/// Host APIs must expose full-width native identities as opaque strings/handles.
pub fn validate(value: &Value) -> Result<(), String> {
    fn visit(value: &Value, depth: usize) -> bool {
        if depth > 32 {
            return false;
        }
        match value {
            Value::Number(number) => {
                number.as_u64().is_none_or(|v| v <= 9_007_199_254_740_991)
                    && number.as_i64().is_none_or(|v| {
                        (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&v)
                    })
            }
            Value::Array(values) => values.iter().all(|value| visit(value, depth + 1)),
            Value::Object(values) => values.values().all(|value| visit(value, depth + 1)),
            _ => true,
        }
    }
    if !visit(value, 0) {
        return Err("Script value exceeds depth or lossless integer bounds".into());
    }
    if serde_json::to_vec(value).map_err(|e| e.to_string())?.len() > 64 * 1024 {
        return Err("Script value exceeds 64 KiB".into());
    }
    Ok(())
}

/// Convert host data without silently rounding integer identities.
pub fn to_lua(lua: &lua::Lua, value: &Value) -> lua::Result<lua::Value> {
    validate(value).map_err(lua::Error::runtime)?;
    lua.to_value_with(
        value,
        lua::SerializeOptions::default().set_array_metatable(true),
    )
}

/// Convert data in an initialized Kedra VM. Depth/cycle/nonfinite validation runs
/// before serde decoding; unsupported tables and oversized data fail explicitly.
pub fn from_lua(lua: &lua::Lua, value: lua::Value) -> lua::Result<Value> {
    lua.named_registry_value::<lua::Function>("_kedra_validate_value")?
        .call::<()>(value.clone())?;
    let value = lua.from_value_with(
        value,
        lua::DeserializeOptions::default().encode_empty_tables_as_array(false),
    )?;
    validate(&value).map_err(lua::Error::runtime)?;
    Ok(value)
}
