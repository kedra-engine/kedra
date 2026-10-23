//! Real Bevy/BMS host, with ordinary domain data and no editor/QA dependency.
use bevy::prelude::*;
use kedra_script::{
    Program, Run, Scope, Status,
    lua::{self, Lua},
};
use serde_json::Value;

#[derive(Resource, Default)]
pub struct Counter(pub i32);

pub fn game_api(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    let game = lua.create_table()?;
    let read = scope.clone();
    game.set(
        "count",
        lua.create_function(move |_, ()| {
            read.with_world(|world| Ok(world.resource::<Counter>().0))
        })?,
    )?;
    game.set(
        "increment",
        lua.create_function(move |_, ()| {
            scope.with_world(|world| {
                world.resource_mut::<Counter>().0 += 1;
                Ok(())
            })
        })?,
    )?;
    game.set_readonly(true);
    ctx.set("game", game)
}
pub fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        kedra_script::LuauPlugin,
    ))
    .init_resource::<Counter>();
    app.finish();
    app.cleanup();
    app
}
pub fn start(app: &mut App, source: &str) -> Result<Run, String> {
    kedra_script::start(
        app.world_mut(),
        Program::sequence("fixture", "fixture.luau", source, game_api),
        Value::Null,
    )
    .map_err(|error| error.to_string())
}
pub fn finish(app: &mut App, run: Run) -> Result<Value, String> {
    for _ in 0..100 {
        app.update();
        match kedra_script::status(app.world(), run) {
            Some(Status::Completed(value)) => return Ok(value.clone()),
            Some(Status::Failed(error)) => return Err(error.to_string()),
            Some(Status::Cancelled) => return Err("Cancelled".into()),
            None => return Err("Retired".into()),
            _ => (),
        }
    }
    Err("Did not complete in 100 updates".into())
}
