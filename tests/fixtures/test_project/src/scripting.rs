//! Explicit game-owned authorization; mounting arbitrary source never executes it.
use bevy::prelude::*;
use kedra_script::{Program, ScriptError, lua};
use kedra_ui::authored::view::UiView;
use serde_json::{Value, json};

pub fn bind(world: &mut World, view: &UiView) -> Result<(), ScriptError> {
    let Some(&target) = view.elements().get("game.switch") else {
        return Ok(());
    };
    let mut program = Program::behavior(
        "game.switch",
        "scripts/Native/switch_behavior.luau",
        include_str!("../scripts/Native/switch_behavior.luau"),
        |_, _, _| Ok(()),
    );
    program.params = json!({"color":[30,100,190,255]});
    program.validate_params = validate;
    program.events = &["activate"];
    program.sequences.push(Program::sequence(
        "game.switch.activate",
        "scripts/Native/switch_activate.luau",
        include_str!("../scripts/Native/switch_activate.luau"),
        |lua: &lua::Lua, scope, ctx| kedra_script_bevy::install_ui(lua, scope, ctx),
    ));
    kedra_script::attach(world, target, program, Value::Null)?;
    Ok(())
}

fn validate(value: &Value) -> Result<(), String> {
    if value.as_object().is_some_and(|value| value.len() == 1)
        && value["color"].as_array().is_some_and(|color| {
            color.len() == 4
                && color
                    .iter()
                    .all(|channel| channel.as_u64().is_some_and(|channel| channel <= 255))
        })
    {
        Ok(())
    } else {
        Err("Expected one four-byte color parameter".into())
    }
}
