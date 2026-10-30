//! Editor-owned capabilities over the shared engine Luau runtime.
use bevy::prelude::*;
use kedra_script::{
    Program, Scope,
    lua::{self, Lua},
};

pub(crate) fn install(app: &mut App) {
    kedra_app::scripting::register(
        app,
        Program::sequence(
            "editor.startup.forms",
            "crates/kedra_editor/src/scripting/startup.luau",
            include_str!("scripting/startup.luau"),
            editor_api,
        ),
    );
    kedra_app::scripting::register(
        app,
        Program::sequence(
            "editor.profile.menus",
            "crates/kedra_editor/src/scripting/profile_menus.luau",
            include_str!("scripting/profile_menus.luau"),
            editor_api,
        ),
    );
}

fn editor_api(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    kedra_app::scripting::install_app_api(lua, scope.clone(), ctx)?;
    let editor = lua.create_table()?;
    editor.set(
        "state",
        kedra_app::scripting::bind(lua, scope.clone(), "editor.state", false)?,
    )?;
    editor.set(
        "workspace",
        kedra_app::scripting::bind(lua, scope, "editor.workspace", false)?,
    )?;
    editor.set_readonly(true);
    ctx.set("editor", editor)
}
