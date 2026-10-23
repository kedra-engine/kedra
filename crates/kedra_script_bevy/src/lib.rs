//! Opt-in native Bevy capabilities; the script VM remains consumer-neutral.
//!
//! Transform operations address only the attached live entity's local Transform.
//! They never touch GlobalTransform, physics bodies or authored source documents.
//! UI operations use existing BackgroundColor and Activate, not a competing widget.
use bevy::prelude::*;
use kedra_script::{
    EntityHandle, ErrorKind, Scope, ScriptError,
    lua::{self, Lua},
};

/// Forward real Bevy widget activation into bounded script notifications.
/// Install alongside LuauPlugin only in hosts authorizing this subsystem. Delivery
/// runs at the script runtime's Last boundary, never recursively inside an observer.
pub struct ScriptBevyPlugin;
impl Plugin for ScriptBevyPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(
            |event: On<bevy::ui_widgets::Activate>, mut commands: Commands| {
                let target = event.entity;
                commands.queue(move |world: &mut World| {
                    if let Err(error) = kedra_script::events::notify(
                        world,
                        target,
                        "activate",
                        serde_json::json!({}),
                    ) {
                        eprintln!("Script activation delivery: {error}");
                    }
                });
            },
        );
    }
}

fn error(kind: ErrorKind, message: &str) -> lua::Error {
    lua::Error::external(ScriptError::new(kind, message))
}

/// Install local-position get/set on the current owned target only. No generic
/// component lookup, query API, parenting, world-space conversion or source editing.
/// Missing Transform, nonfinite/out-of-f32-range values and foreign/stale handles
/// fail before mutation. Position values are independent `{x, y, z}` tables.
pub fn install_transform(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    let api = lua.create_table()?;
    let read = scope.clone();
    api.set(
        "get_position",
        lua.create_function(move |lua, handle: lua::AnyUserData| {
            let handle = *handle.borrow::<EntityHandle>()?;
            let position = read.with_world(|world| {
                let entity = read.resolve_entity(world, &handle)?;
                world
                    .get::<Transform>(entity)
                    .map(|transform| transform.translation)
                    .ok_or_else(|| error(ErrorKind::InvalidInput, "Owned target has no Transform"))
            })?;
            let value = lua.create_table()?;
            value.set("x", position.x)?;
            value.set("y", position.y)?;
            value.set("z", position.z)?;
            Ok(value)
        })?,
    )?;
    api.set(
        "set_position",
        lua.create_function(move |_, (handle, value): (lua::AnyUserData, lua::Table)| {
            let handle = *handle.borrow::<EntityHandle>()?;
            let position = Vec3::new(value.get("x")?, value.get("y")?, value.get("z")?);
            if !position.is_finite() {
                return Err(error(
                    ErrorKind::InvalidInput,
                    "Position must contain finite f32 coordinates",
                ));
            }
            scope.with_world(|world| {
                let entity = scope.resolve_entity(world, &handle)?;
                let mut transform = world.get_mut::<Transform>(entity).ok_or_else(|| {
                    error(ErrorKind::InvalidInput, "Owned target has no Transform")
                })?;
                transform.translation = position;
                Ok(())
            })
        })?,
    )?;
    api.set_readonly(true);
    ctx.set("transform", api)
}

/// Install the retained UI background operation on the current owned target only.
/// Values are sRGB bytes `[red, green, blue, alpha]`; malformed values, missing UI
/// components and unowned targets fail before mutation. Widget lifecycle stays Bevy-owned.
pub fn install_ui(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    let api = lua.create_table()?;
    api.set(
        "set_background",
        lua.create_function(move |_, (handle, rgba): (lua::AnyUserData, lua::Table)| {
            let handle = *handle.borrow::<EntityHandle>()?;
            let mut bytes = [0; 4];
            if rgba
                .clone()
                .pairs::<lua::Value, lua::Value>()
                .take(5)
                .count()
                != 4
            {
                return Err(error(ErrorKind::InvalidInput, "Expected four sRGB bytes"));
            }
            for (index, byte) in bytes.iter_mut().enumerate() {
                let value: f64 = rgba.raw_get(index + 1)?;
                if !value.is_finite() || value.fract() != 0.0 || !(0.0..=255.0).contains(&value) {
                    return Err(error(
                        ErrorKind::InvalidInput,
                        "Expected four integral sRGB bytes",
                    ));
                }
                *byte = value as u8;
            }
            let [r, g, b, a] = bytes;
            scope.with_world(|world| {
                let entity = scope.resolve_entity(world, &handle)?;
                let mut color = world.get_mut::<BackgroundColor>(entity).ok_or_else(|| {
                    error(
                        ErrorKind::InvalidInput,
                        "Owned target has no BackgroundColor",
                    )
                })?;
                color.0 = Color::srgba_u8(r, g, b, a);
                Ok(())
            })
        })?,
    )?;
    api.set_readonly(true);
    ctx.set("ui", api)
}
