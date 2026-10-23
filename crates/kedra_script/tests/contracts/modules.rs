//! Kedra's admitted-source boundary and standard resolver integration, using real VMs.
#![allow(clippy::unwrap_used, reason = "Isolated module contract assertions")]
use crate::support::*;
use kedra_script::{Program, Sources};
use serde_json::{Value, json};

fn program(source: &str) -> Program {
    Program::sequence("modules", "scripts/entry.luau", source, game_api)
}

#[test]
fn relative_alias_and_directory_imports_share_one_cache_per_vm() {
    let mut app = app();
    let mut program = program(
        r#"
        local a = require("./lib/counter")
        local b = require("@game/lib/counter")
        local package = require("./package")
        function run(ctx, args)
            assert(a == b and a.value == 1)
            a.value += 1
            ctx.game.increment()
            return {value = b.value, nested = package.value}
        end
    "#,
    );
    program
        .sources
        .insert(".luaurc", r#"{"aliases":{"game":"./scripts"}}"#)
        .unwrap();
    program
        .sources
        .insert("scripts/lib/counter.luau", "return {value = 1}")
        .unwrap();
    program
        .sources
        .insert("scripts/package/init.luau", "return require('../lib/data')")
        .unwrap();
    program
        .sources
        .insert("scripts/lib/data.luau", "return {value = 'Кедра'}")
        .unwrap();
    for _ in 0..2 {
        let run = kedra_script::start(app.world_mut(), program.clone(), Value::Null).unwrap();
        assert_eq!(
            finish(&mut app, run),
            Ok(json!({"value":2,"nested":"Кедра"}))
        );
    }
    assert_eq!(app.world().resource::<Counter>().0, 2);
}

#[test]
fn modules_loaded_during_a_sequence_keep_relative_imports_and_owned_arguments() {
    let mut app = app();
    let mut program = program(
        r#"
        function run(ctx, args)
            wait.updates(1)
            return require('./lib/actions').execute(ctx, args)
        end
    "#,
    );
    program
        .sources
        .insert(
            "scripts/lib/actions.luau",
            r#"
        local data = require('./data')
        local actions = {}
        function actions.execute(ctx, args)
            ctx.game.increment()
            return {name = args.name, value = data.value}
        end
        return table.freeze(actions)
    "#,
        )
        .unwrap();
    program
        .sources
        .insert("scripts/lib/data.luau", "return {value = 7}")
        .unwrap();
    let run = kedra_script::start(app.world_mut(), program, json!({"name":"世界"})).unwrap();
    assert_eq!(finish(&mut app, run), Ok(json!({"name":"世界","value":7})));
    assert_eq!(app.world().resource::<Counter>().0, 1);
}

#[test]
fn missing_external_and_ambiguous_imports_publish_no_owner_or_effect() {
    let mut app = app();
    for import in [
        "./missing",
        "../../Cargo",
        "/etc/passwd",
        "scripts/lib/a",
        "@outside/a",
        "./lib/a",
    ] {
        let mut program = program(&format!(
            "local module = require({import:?}); function run(ctx) ctx.game.increment() end"
        ));
        program
            .sources
            .insert(".luaurc", r#"{"aliases":{"outside":"/etc"}}"#)
            .unwrap();
        program
            .sources
            .insert("scripts/lib/a.lua", "return {}")
            .unwrap();
        program
            .sources
            .insert("scripts/lib/a.luau", "return {}")
            .unwrap();
        assert!(
            kedra_script::start(app.world_mut(), program, Value::Null).is_err(),
            "Accepted {import}"
        );
        assert_eq!(app.world().resource::<Counter>().0, 0);
        assert_eq!(
            app.world()
                .resource::<bevy::asset::Assets<bevy_mod_scripting::asset::ScriptAsset>>()
                .len(),
            0
        );
    }
}

#[test]
fn snapshot_rejects_noncanonical_duplicate_and_oversized_sources_without_replacement() {
    let mut sources = Sources::default();
    for path in [
        "../a.luau",
        "/a.luau",
        "a\\b.luau",
        "game://a.luau",
        "a/./b.luau",
        "a//b.luau",
    ] {
        assert!(sources.insert(path, "return {}").is_err());
    }
    assert!(sources.insert("a.luau", " ".repeat(65537)).is_err());
    sources.insert("a.luau", "return {value = 42}").unwrap();
    assert!(sources.insert("a.luau", "return {value = 99}").is_err());
    let mut program =
        program("local a = require('../a'); function run(ctx, args) return a.value end");
    program.sources = sources;
    let mut app = app();
    let run = kedra_script::start(app.world_mut(), program, Value::Null).unwrap();
    assert_eq!(finish(&mut app, run), Ok(json!(42)));
}

#[test]
fn replacement_uses_a_fresh_module_cache_and_invalid_candidate_preserves_old_run() {
    let mut app = app();
    let mut original = program(
        "local value = require('./value'); function run(ctx) wait.updates(2); return value end",
    );
    original
        .sources
        .insert("scripts/value.luau", "return 1")
        .unwrap();
    let old = kedra_script::start(app.world_mut(), original, Value::Null).unwrap();
    let invalid = program("require('./missing'); function run(ctx) end");
    assert!(kedra_script::replace(app.world_mut(), old, invalid, Value::Null).is_err());
    let mut replacement =
        program("local value = require('./value'); function run(ctx) return value end");
    replacement
        .sources
        .insert("scripts/value.luau", "return 2")
        .unwrap();
    let new = kedra_script::replace(app.world_mut(), old, replacement, Value::Null).unwrap();
    assert!(kedra_script::status(app.world(), old).is_none());
    assert_eq!(finish(&mut app, new), Ok(json!(2)));
}

#[test]
fn nested_configuration_matches_the_importing_module_location() {
    let mut app = app();
    let mut configured =
        program("local value = require('./nested/use'); function run(ctx) return value end");
    configured
        .sources
        .insert(".luaurc", r#"{"aliases":{"data":"./scripts/root"}}"#)
        .unwrap();
    configured
        .sources
        .insert(
            "scripts/nested/.luaurc",
            r#"{"aliases":{"data":"./local"}}"#,
        )
        .unwrap();
    configured
        .sources
        .insert("scripts/nested/use.luau", "return require('@data/value')")
        .unwrap();
    configured
        .sources
        .insert("scripts/nested/local/value.luau", "return 42")
        .unwrap();
    configured
        .sources
        .insert("scripts/root/value.luau", "return 99")
        .unwrap();
    let run = kedra_script::start(app.world_mut(), configured, Value::Null).unwrap();
    assert_eq!(finish(&mut app, run), Ok(json!(42)));
}

#[test]
fn yielding_module_initialization_obeys_sequence_waits_and_revocation_not_entry_loading() {
    let mut app = app();
    let mut yielding = program(
        r#"
        function run(ctx)
            local first = require('./yielding')
            local second = require('./yielding')
            assert(first == second)
            ctx.game.increment()
            return first.value
        end
    "#,
    );
    yielding
        .sources
        .insert(
            "scripts/yielding.luau",
            "wait.updates(2); return {value = 42}",
        )
        .unwrap();
    let run = kedra_script::start(app.world_mut(), yielding.clone(), Value::Null).unwrap();
    app.update();
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 0);
    assert_eq!(finish(&mut app, run), Ok(json!(42)));
    let cancelled = kedra_script::start(app.world_mut(), yielding.clone(), Value::Null).unwrap();
    app.update();
    kedra_script::cancel(app.world_mut(), cancelled).unwrap();
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().resource::<Counter>().0, 1);
    assert_eq!(
        kedra_script::status(app.world(), cancelled),
        Some(&kedra_script::Status::Cancelled)
    );
    let mut behavior = Program::behavior(
        "behavior",
        "scripts/behavior.luau",
        "function start(self, ctx) local value = require('./yielding'); ctx.game.increment() end",
        game_api,
    );
    behavior.sources = yielding.sources.clone();
    let behavior = kedra_script::start(app.world_mut(), behavior, Value::Null).unwrap();
    app.update();
    assert!(matches!(
        kedra_script::status(app.world(), behavior),
        Some(kedra_script::Status::Failed(_))
    ));
    yielding.source =
        "local value = require('./yielding'); function run(ctx) ctx.game.increment() end".into();
    assert!(kedra_script::start(app.world_mut(), yielding, Value::Null).is_err());
    assert_eq!(app.world().resource::<Counter>().0, 1);
    assert_eq!(
        app.world()
            .resource::<bevy::asset::Assets<bevy_mod_scripting::asset::ScriptAsset>>()
            .len(),
        0
    );
}
