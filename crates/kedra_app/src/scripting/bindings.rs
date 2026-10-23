//! Scoped native method adapters. The same registered systems serve RPC and Luau.
use bevy::prelude::*;
use bevy_remote::{RemoteMethodSystemId, RemoteMethods};
use kedra_script::{
    Scope,
    lua::{self, Lua},
};
use serde_json::{Value, json};
use uuid::Uuid;

/// Call an explicitly selected native feature system from trusted host binding code.
/// No arbitrary method selector is given to scripts. Effects get fresh identities;
/// their returned receipts must be awaited, never treated as feature acceptance.
fn call(scope: &Scope, method: &str, mut params: Value, effect: bool) -> lua::Result<Value> {
    scope.with_world(|world| {
        if effect {
            let object = params
                .as_object_mut()
                .ok_or_else(|| lua::Error::runtime("Effect arguments must be an object"))?;
            if object.contains_key("session") || object.contains_key("id") {
                return Err(lua::Error::runtime("Effect identity is owned by the host"));
            }
            if let Some(Value::String(window)) = object.get("window") {
                let id = window.parse::<u64>().map_err(lua::Error::external)?;
                object.insert("window".into(), json!(id));
            }
            object.insert(
                "session".into(),
                json!(world.resource::<crate::AppSession>().id),
            );
            object.insert("id".into(), json!(Uuid::new_v4()));
        }
        let Some(RemoteMethodSystemId::Instant(system)) =
            world.resource::<RemoteMethods>().get(method).copied()
        else {
            return Err(lua::Error::runtime("Native method is not registered"));
        };
        world
            .run_system_with(system, Some(params))
            .map_err(lua::Error::external)?
            .map_err(|error| lua::Error::runtime(error.message))
    })
}

/// Create a zero/one-argument Lua function bound to one fixed feature method.
/// Full-width native identity fields in observations are exposed as decimal strings.
pub fn bind(
    lua: &Lua,
    scope: Scope,
    method: &'static str,
    effect: bool,
) -> lua::Result<lua::Function> {
    lua.create_function(move |lua, value: lua::Value| {
        let params = if value == lua::Value::Nil {
            Value::Null
        } else {
            kedra_script::values::from_lua(lua, value)?
        };
        let mut result = call(&scope, method, params, effect)?;
        identities(&mut result);
        kedra_script::values::to_lua(lua, &result)
    })
}

fn identities(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(identities),
        Value::Object(values) => {
            for (key, value) in values {
                if matches!(key.as_str(), "id" | "entity" | "parent" | "window") && value.is_u64() {
                    *value = Value::String(value.to_string());
                } else {
                    identities(value);
                }
            }
        }
        _ => (),
    }
}

fn observation(scope: &Scope) -> lua::Result<Value> {
    let mut value = call(scope, "app.observe", Value::Null, false)?;
    if value["truncated"] != false {
        return Err(lua::Error::runtime("Incomplete UI observation"));
    }
    let mut ids = std::collections::BTreeSet::new();
    for element in value["elements"]
        .as_array()
        .ok_or_else(|| lua::Error::runtime("Missing UI inventory"))?
    {
        if element["visible"] == true
            && !ids.insert((element["window"].to_string(), element["id"].to_string()))
        {
            return Err(lua::Error::runtime("Ambiguous visible UI identity"));
        }
    }
    identities(&mut value);
    Ok(value)
}

/// Install only the shared input/observation/capture capabilities, without editor API.
pub fn install_app_api(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    let app = lua.create_table()?;
    app.set("receipt", bind(lua, scope.clone(), "app.operation", false)?)?;
    let submit_scope = scope.clone();
    app.set(
        "submit",
        lua.create_function(move |lua, (method, value): (String, lua::Value)| {
            if !matches!(method.as_str(), "app.input" | "app.resize" | "app.capture") {
                return Err(lua::Error::runtime(
                    "Method is outside the application script scope",
                ));
            }
            let params = kedra_script::values::from_lua(lua, value)?;
            let mut receipt = call(&submit_scope, &method, params, true)?;
            identities(&mut receipt);
            kedra_script::values::to_lua(lua, &receipt)
        })?,
    )?;
    let window_scope = scope.clone();
    app.set(
        "window",
        lua.create_function(move |_, ()| {
            let value = observation(&window_scope)?;
            let windows: Vec<_> = value["windows"]
                .as_array()
                .ok_or_else(|| lua::Error::runtime("Missing windows"))?
                .iter()
                .filter(|window| window["primary"] == true)
                .collect();
            if windows.len() != 1 {
                return Err(lua::Error::runtime("Expected one primary window"));
            }
            windows[0]["id"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| lua::Error::runtime("Invalid native window identity"))
        })?,
    )?;
    app.set(
        "node",
        lua.create_function(move |lua, id: String| {
            let value = observation(&scope)?;
            let node = value["elements"]
                .as_array()
                .ok_or_else(|| lua::Error::runtime("Missing UI inventory"))?
                .iter()
                .find(|node| node["id"] == id && node["visible"] == true);
            match node {
                Some(node) => kedra_script::values::to_lua(lua, node),
                None => Ok(lua::Value::Nil),
            }
        })?,
    )?;
    app.set_readonly(true);
    let factory: lua::Function = lua
        .load(include_str!("native.luau"))
        .set_name("kedra/app.luau")
        .eval()?;
    ctx.set("app", factory.call::<lua::Table>(app)?)
}
