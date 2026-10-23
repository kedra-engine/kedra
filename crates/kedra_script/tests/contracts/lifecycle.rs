//! Integration contracts use the actual Luau VM and Bevy schedules.
#![allow(clippy::unwrap_used, reason = "Isolated real-VM fixture assertions")]
use crate::support::*;
use bevy::prelude::*;
use kedra_script::{
    Program, Scope, Status,
    lua::{self, Lua},
};
use serde_json::{Value, json};

#[test]
fn update_waits_count_future_updates_and_return_owned_unicode_data() {
    let mut app = app();
    let run = start(&mut app, "function run(ctx) ctx.game.increment(); wait.updates(3); ctx.game.increment(); return {name = 'Кедра'} end").unwrap();
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 1);
    for _ in 0..2 {
        app.update();
        assert_eq!(app.world().resource::<Counter>().0, 1);
    }
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 2);
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Completed(json!({"name":"Кедра"})))
    );
}

#[test]
fn cancel_is_terminal_releases_context_and_preserves_applied_effects() {
    let mut app = app();
    let run = start(
        &mut app,
        "function run(ctx) ctx.game.increment(); wait.updates(2); ctx.game.increment() end",
    )
    .unwrap();
    app.update();
    kedra_script::cancel(app.world_mut(), run).unwrap();
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(app.world().resource::<Counter>().0, 1);
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Cancelled)
    );
    assert_eq!(
        app.world()
            .resource::<bevy::asset::Assets<bevy_mod_scripting::asset::ScriptAsset>>()
            .len(),
        0
    );
    kedra_script::retire(app.world_mut(), run).unwrap();
    assert!(kedra_script::status(app.world(), run).is_none());
    assert!(kedra_script::cancel(app.world_mut(), run).is_err());
}

#[test]
fn ordinary_parent_despawn_retires_its_coroutine() {
    let mut app = app();
    let parent = app.world_mut().spawn_empty().id();
    let run = kedra_script::attach(
        app.world_mut(),
        parent,
        Program::sequence(
            "child",
            "entry.luau",
            "function run(ctx) ctx.game.increment(); wait.updates(1); ctx.game.increment() end",
            game_api,
        ),
        Value::Null,
    )
    .unwrap();
    app.update();
    app.world_mut().despawn(parent);
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 1);
    assert!(kedra_script::status(app.world(), run).is_none());
    assert!(
        kedra_script::attach(
            app.world_mut(),
            parent,
            Program::sequence(
                "stale",
                "entry.luau",
                "function run(ctx, args) end",
                game_api
            ),
            Value::Null
        )
        .is_err()
    );
}

#[test]
fn errors_are_terminal_not_retried_and_not_rollback() {
    let mut app = app();
    let run = start(
        &mut app,
        "function run(ctx) ctx.game.increment(); error('expected failure') end",
    )
    .unwrap();
    assert!(
        finish(&mut app, run)
            .unwrap_err()
            .contains("expected failure")
    );
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 1);
}

#[test]
fn source_errors_missing_entry_and_top_level_effects_publish_no_run() {
    let mut app = app();
    for source in [
        "function run( bad",
        "local x = 1",
        "ctx.game.increment(); function run(ctx, args) end",
    ] {
        assert!(start(&mut app, source).is_err());
        assert_eq!(app.world().resource::<Counter>().0, 0);
        assert_eq!(
            app.world()
                .resource::<Assets<bevy_mod_scripting::asset::ScriptAsset>>()
                .len(),
            0
        );
    }
    let run = start(&mut app, "function run(ctx) ctx.game.increment() end").unwrap();
    assert_eq!(finish(&mut app, run), Ok(Value::Null));
}

// A test-owned capability stand-in, not the Kedra editor's selection authority.
#[derive(Resource)]
struct ToolSelection(String);

fn tool_api(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    game_api(lua, scope.clone(), ctx)?;
    let editor = lua.create_table()?;
    editor.set(
        "selection",
        lua.create_function(move |_, ()| {
            scope.with_world(|world| Ok(world.resource::<ToolSelection>().0.clone()))
        })?,
    )?;
    editor.set_readonly(true);
    ctx.set("editor", editor)
}

#[test]
fn scopes_are_independent_and_tables_readonly() {
    let mut app = app();
    app.insert_resource(ToolSelection("Main Camera".into()));
    let game_source = include_str!("../scopes/game.luau");
    let run = start(&mut app, game_source).unwrap();
    assert_eq!(finish(&mut app, run), Ok(Value::Null));
    assert_eq!(app.world().resource::<Counter>().0, 1);

    let tool = kedra_script::start(
        app.world_mut(),
        Program::sequence(
            "tool",
            "tool.luau",
            include_str!("../scopes/tool.luau"),
            tool_api,
        ),
        Value::Null,
    )
    .unwrap();
    assert_eq!(finish(&mut app, tool), Ok(Value::Null));
    // Tool authorization and globals must not leak into a subsequent game scope.
    let next_game = start(&mut app, game_source).unwrap();
    assert_eq!(finish(&mut app, next_game), Ok(Value::Null));
    for expression in ["ctx.editor.state()", "ctx.editor.selection()"] {
        let denied = start(&mut app, &format!("function run(ctx) {expression} end")).unwrap();
        assert!(finish(&mut app, denied).is_err());
    }
    assert_eq!(app.world().resource::<Counter>().0, 2);
}

#[test]
fn invalid_waits_and_unknown_yields_fail_closed() {
    for expression in [
        "wait.updates(0)",
        "wait.updates(-1)",
        "wait.updates(1.5)",
        "wait.updates(1000001)",
        "wait.updates('1')",
        "wait.updates(0/0)",
        "coroutine.yield('unsupported')",
    ] {
        let mut app = app();
        let run = start(
            &mut app,
            &format!("function run(ctx) {expression}; ctx.game.increment() end"),
        )
        .unwrap();
        assert!(finish(&mut app, run).is_err(), "Accepted {expression}");
        assert_eq!(app.world().resource::<Counter>().0, 0);
    }
}

#[test]
fn fixed_waits_do_not_count_display_updates() {
    let mut app = app();
    let run = start(
        &mut app,
        "function run(ctx) wait.fixed(2); ctx.game.increment() end",
    )
    .unwrap();
    app.update();
    // Pause Bevy virtual time so only explicitly run FixedUpdate schedules count.
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<Counter>().0, 0);
    app.world_mut().run_schedule(FixedUpdate);
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 0);
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(finish(&mut app, run), Ok(Value::Null));
    assert_eq!(app.world().resource::<Counter>().0, 1);
}

#[test]
fn failed_replacement_preserves_old_generation_success_resets_it() {
    let mut app = app();
    let old = start(
        &mut app,
        "old_global = 42; function run(ctx) ctx.game.increment(); wait.updates(3); ctx.game.increment() end",
    )
    .unwrap();
    app.update();
    assert!(
        kedra_script::replace(
            app.world_mut(),
            old,
            Program::sequence("bad", "entry.luau", "bad syntax (", game_api),
            Value::Null
        )
        .is_err()
    );
    assert_eq!(
        kedra_script::status(app.world(), old),
        Some(&Status::Running)
    );
    let new = kedra_script::replace(
        app.world_mut(),
        old,
        Program::sequence(
            "new",
            "entry.luau",
            "function run(ctx) assert(old_global == nil); wait.updates(4); ctx.game.increment() end",
            game_api,
        ),
        Value::Null,
    )
    .unwrap();
    assert!(kedra_script::status(app.world(), old).is_none());
    assert_eq!(finish(&mut app, new), Ok(Value::Null));
    assert_eq!(app.world().resource::<Counter>().0, 2);
}

#[test]
fn foreign_handles_cannot_inspect_cancel_or_replace_another_world() {
    let mut first = app();
    let run = start(&mut first, "function run() wait.updates(2) end").unwrap();
    let mut second = app();
    assert!(kedra_script::status(second.world(), run).is_none());
    assert!(kedra_script::cancel(second.world_mut(), run).is_err());
    assert!(kedra_script::retire(second.world_mut(), run).is_err());
    assert!(
        kedra_script::replace(
            second.world_mut(),
            run,
            Program::sequence(
                "foreign",
                "entry.luau",
                "function run(ctx, args) end",
                game_api
            ),
            Value::Null
        )
        .is_err()
    );
}

#[derive(Resource, Default)]
struct Saved(Option<Scope>);
fn remember(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    game_api(lua, scope.clone(), ctx)?;
    ctx.set(
        "remember",
        lua.create_function(move |_, ()| {
            scope.with_world(|world| {
                world.resource_mut::<Saved>().0 = Some(scope.clone());
                Ok(())
            })
        })?,
    )
}
fn stale(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    game_api(lua, scope.clone(), ctx)?;
    ctx.set(
        "stale",
        lua.create_function(move |_, ()| {
            let old = scope.with_world(|world| {
                world
                    .resource::<Saved>()
                    .0
                    .clone()
                    .ok_or_else(|| lua::Error::runtime("Missing saved scope"))
            })?;
            old.with_world(|world| {
                world.resource_mut::<Counter>().0 += 100;
                Ok(())
            })
        })?,
    )
}

#[test]
fn captured_revoked_scope_fails_without_poisoning_current_world_access() {
    let mut app = app();
    app.init_resource::<Saved>();
    let old = kedra_script::start(
        app.world_mut(),
        Program::sequence(
            "old",
            "entry.luau",
            "function run(ctx) ctx.remember(); wait.updates(3) end",
            remember,
        ),
        Value::Null,
    )
    .unwrap();
    app.update();
    kedra_script::cancel(app.world_mut(), old).unwrap();
    let new = kedra_script::start(
        app.world_mut(),
        Program::sequence(
            "new",
            "entry.luau",
            "function run(ctx) assert(not pcall(ctx.stale)); ctx.game.increment() end",
            stale,
        ),
        Value::Null,
    )
    .unwrap();
    assert_eq!(finish(&mut app, new), Ok(Value::Null));
    assert_eq!(app.world().resource::<Counter>().0, 1);
}

fn panicking_api(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    game_api(lua, scope.clone(), ctx)?;
    ctx.set(
        "host_failure",
        lua.create_function(move |_, ()| {
            scope.with_world::<()>(|world| {
                world.resource_mut::<Counter>().0 += 1;
                panic!("Intentional native binding failure")
            })
        })?,
    )
}

#[test]
fn native_binding_unwind_is_an_error_and_releases_borrow_without_rollback() {
    let mut app = app();
    let run = kedra_script::start(
        app.world_mut(),
        Program::sequence(
            "panic",
            "entry.luau",
            "function run(ctx) assert(not pcall(ctx.host_failure)); ctx.game.increment() end",
            panicking_api,
        ),
        Value::Null,
    )
    .unwrap();
    assert_eq!(finish(&mut app, run), Ok(Value::Null));
    assert_eq!(app.world().resource::<Counter>().0, 2);
}

#[test]
fn cancellation_before_first_resume_never_runs_the_entrypoint() {
    let mut app = app();
    let run = start(&mut app, "function run(ctx) ctx.game.increment() end").unwrap();
    kedra_script::cancel(app.world_mut(), run).unwrap();
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 0);
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Cancelled)
    );
}

#[test]
fn arguments_and_results_preserve_empty_arrays_maps_null_and_unicode() {
    let mut app = app();
    let value = json!({"array":[],"object":{},"null":null,"name":"Кедра 日本語"});
    let run = kedra_script::start(
        app.world_mut(),
        Program::sequence(
            "values",
            "entry.luau",
            "function run(ctx, args) return args end",
            game_api,
        ),
        value.clone(),
    )
    .unwrap();
    assert_eq!(finish(&mut app, run), Ok(value));
}

fn borrow_saved_api(lua: &Lua, _: Scope, ctx: &lua::Table) -> lua::Result<()> {
    use bevy_mod_scripting::bindings::ThreadWorldContainer;
    let saved = ThreadWorldContainer
        .try_get_context()
        .map_err(|error| lua::Error::runtime(format!("{error:?}")))?
        .world
        .with_world_access(|world| world.resource::<Saved>().0.clone())
        .map_err(|error| lua::Error::runtime(format!("{error:?}")))?
        .ok_or_else(|| lua::Error::runtime("Missing active saved scope"))?;
    ctx.set(
        "borrowed",
        lua.create_function(move |_, ()| {
            saved.with_world(|world| {
                world.resource_mut::<Counter>().0 += 100;
                Ok(())
            })
        })?,
    )
}
fn nested_api(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    game_api(lua, scope.clone(), ctx)?;
    ctx.set(
        "probe",
        lua.create_function(move |_, ()| {
            scope.with_world(|world| {
                world.resource_mut::<Saved>().0 = Some(scope.clone());
                let candidate = kedra_script::start(
                    world,
                    Program::sequence(
                        "nested",
                        "entry.luau",
                        "ctx.borrowed(); function run(ctx, args) end",
                        borrow_saved_api,
                    ),
                    Value::Null,
                );
                if let Ok(run) = candidate {
                    let _ = kedra_script::retire(world, run);
                    return Ok(false);
                }
                Ok(true)
            })
        })?,
    )
}

#[test]
fn even_an_active_scope_cannot_be_borrowed_by_another_context_during_nested_load() {
    let mut app = app();
    app.init_resource::<Saved>();
    let run = kedra_script::start(
        app.world_mut(),
        Program::sequence(
            "outer",
            "entry.luau",
            "function run(ctx) assert(ctx.probe()); ctx.game.increment() end",
            nested_api,
        ),
        Value::Null,
    )
    .unwrap();
    assert_eq!(finish(&mut app, run), Ok(Value::Null));
    assert_eq!(app.world().resource::<Counter>().0, 1);
}

#[derive(Resource)]
struct Parent(Entity);

fn teardown_api(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    game_api(lua, scope.clone(), ctx)?;
    ctx.set(
        "remove_parent",
        lua.create_function(move |_, ()| {
            scope.with_world(|world| {
                let parent = world.resource::<Parent>().0;
                world.despawn(parent);
                Ok(())
            })
        })?,
    )
}

#[test]
fn native_callback_can_despawn_its_parent_without_deadlock_or_later_effects() {
    let mut app = app();
    let parent = app.world_mut().spawn_empty().id();
    app.insert_resource(Parent(parent));
    let run = kedra_script::attach(
        app.world_mut(),
        parent,
        Program::sequence(
            "teardown",
            "entry.luau",
            "function run(ctx) ctx.remove_parent(); assert(not pcall(ctx.game.increment)) end",
            teardown_api,
        ),
        Value::Null,
    )
    .unwrap();
    let sibling = kedra_script::attach(
        app.world_mut(),
        parent,
        Program::sequence(
            "sibling",
            "entry.luau",
            "function run(ctx) wait.updates(100); ctx.game.increment() end",
            game_api,
        ),
        Value::Null,
    )
    .unwrap();
    app.update();
    assert!(kedra_script::status(app.world(), run).is_none());
    assert!(kedra_script::status(app.world(), sibling).is_none());
    assert_eq!(app.world().resource::<Counter>().0, 0);
    assert_eq!(
        app.world()
            .resource::<Assets<bevy_mod_scripting::asset::ScriptAsset>>()
            .len(),
        0
    );
}

#[derive(Resource)]
struct CurrentRun(kedra_script::Run);

fn cancel_self_api(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    game_api(lua, scope.clone(), ctx)?;
    ctx.set(
        "cancel_self",
        lua.create_function(move |_, ()| {
            scope.with_world(|world| {
                let run = world.resource::<CurrentRun>().0;
                kedra_script::cancel(world, run).map_err(lua::Error::runtime)
            })
        })?,
    )
}

#[test]
fn cancellation_inside_a_native_callback_stays_terminal_and_revokes_later_effects() {
    let mut app = app();
    let run = kedra_script::start(
        app.world_mut(),
        Program::sequence(
            "cancel-self",
            "entry.luau",
            "function run(ctx) ctx.cancel_self(); assert(not pcall(ctx.game.increment)); return true end",
            cancel_self_api,
        ),
        Value::Null,
    )
    .unwrap();
    app.insert_resource(CurrentRun(run));
    app.update();
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Cancelled)
    );
    assert_eq!(app.world().resource::<Counter>().0, 0);
    assert_eq!(
        app.world()
            .resource::<Assets<bevy_mod_scripting::asset::ScriptAsset>>()
            .len(),
        0
    );
}
