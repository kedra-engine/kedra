//! Real VM budget/value failures, not a mock scheduler or parser.
#![allow(clippy::unwrap_used, reason = "Isolated real-VM fixture assertions")]
use crate::support::*;
use kedra_script::{Limits, Program, Status};
use serde_json::{Value, json};
use std::time::Duration;

#[test]
fn memory_and_callback_budgets_stop_effects_including_caught_errors() {
    // A broken VM interrupt must not hang the Cargo runner. The parent deadline
    // remains independent of the budget being tested, including inside pcall.
    const PROBE: &str = "KEDRA_SCRIPT_BUDGET_PROBE";
    if std::env::var_os(PROBE).is_none() {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "budgets::memory_and_callback_budgets_stop_effects_including_caught_errors",
                "--nocapture",
            ])
            .env(PROBE, "1")
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "VM budget probe failed");
                return;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("VM budget probe exceeded its external process deadline");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    for source in [
        "function run(ctx) local data = string.rep('x', 100000000); ctx.game.increment() end",
        "function run(ctx) local n = 0; for i = 1, 1000000000 do n += i end; ctx.game.increment() end",
        "function run(ctx) while true do pcall(function() local n = 0; for i = 1, 1000000000 do n += i end end) end end",
    ] {
        let mut app = app();
        let run = start(&mut app, source).unwrap();
        assert!(finish(&mut app, run).is_err());
        assert_eq!(app.world().resource::<Counter>().0, 0);
    }
    for source in [
        "function start(self, ctx) while true do pcall(function() local n = 0; for i = 1, 1000000000 do n += i end end) end end",
        "function start(self, ctx) end; function update(self, ctx, dt) error('fail') end; function stop(self, ctx, reason) while true do pcall(function() local n = 0; for i = 1, 1000000000 do n += i end end) end end",
    ] {
        let mut app = app();
        let run = kedra_script::start(
            app.world_mut(),
            Program::behavior("bounded", "bounded.luau", source, game_api),
            Value::Null,
        )
        .unwrap();
        app.update();
        assert!(matches!(
            kedra_script::status(app.world(), run),
            Some(Status::Failed(_))
        ));
        assert_eq!(app.world().resource::<Counter>().0, 0);
        assert_eq!(
            app.world()
                .resource::<bevy::asset::Assets<bevy_mod_scripting::asset::ScriptAsset>>()
                .len(),
            0
        );
    }
    for module in [
        "while true do pcall(function() local n = 0; for i = 1, 1000000000 do n += i end end) end",
        "return require('./a')",
    ] {
        let mut app = app();
        let mut program = Program::sequence(
            "modules",
            "entry.luau",
            "local a = require('./a'); function run(ctx) ctx.game.increment() end",
            game_api,
        );
        program.sources.insert("a.luau", module).unwrap();
        assert!(kedra_script::start(app.world_mut(), program, Value::Null).is_err());
        assert_eq!(app.world().resource::<Counter>().0, 0);
    }
}

#[test]
fn invalid_budgets_source_and_arguments_are_rejected_before_loading() {
    let mut app = app();
    let mut program = Program::sequence(
        "invalid",
        "entry.luau",
        "function run(ctx, args) end",
        game_api,
    );
    for limits in [
        Limits {
            resumes: Some(0),
            ..Limits::default()
        },
        Limits {
            memory_bytes: 1,
            ..Limits::default()
        },
        Limits {
            callback_time: Duration::ZERO,
            ..Limits::default()
        },
        Limits {
            total_time: Some(Duration::from_secs(601)),
            ..Limits::default()
        },
    ] {
        program.limits = limits;
        assert!(kedra_script::start(app.world_mut(), program.clone(), Value::Null).is_err());
    }
    assert!(start(&mut app, &" ".repeat(65537)).is_err());
    assert!(
        kedra_script::start(
            app.world_mut(),
            Program::sequence(
                "bad\nname",
                "entry.luau",
                "function run(ctx, args) end",
                game_api
            ),
            Value::Null
        )
        .is_err()
    );
    assert!(
        kedra_script::start(
            app.world_mut(),
            Program::sequence(
                "args",
                "entry.luau",
                "function run(ctx, args) end",
                game_api
            ),
            json!({"entity":u64::MAX})
        )
        .is_err()
    );
}

#[test]
fn resume_budget_is_terminal_even_for_perpetual_small_yields() {
    let mut app = app();
    let mut program = Program::sequence(
        "budget",
        "entry.luau",
        "function run() while true do wait.updates(1) end end",
        game_api,
    );
    program.limits.resumes = Some(2);
    let run = kedra_script::start(app.world_mut(), program, Value::Null).unwrap();
    app.update();
    app.update();
    app.update();
    assert!(matches!(
        kedra_script::status(app.world(), run),
        Some(Status::Failed(_))
    ));
}

#[test]
fn returned_cycles_large_data_and_unsafe_integer_arguments_fail() {
    for source in [
        "function run() local t = {}; t.self = t; return t end",
        "function run() return string.rep('x', 65537) end",
        "function run() return {nested = {bad = 0/0}} end",
        "function run() return math.huge end",
        "function run() assert = function() end; type = function() return 'string' end; math = {}; return {bad = 0/0} end",
        "function run() local t = {}; for i = 1, 1000 do t = {t} end; return t end",
    ] {
        let mut app = app();
        let run = start(&mut app, source).unwrap();
        assert!(finish(&mut app, run).is_err());
    }
    assert!(kedra_script::values::validate(&json!(u64::MAX)).is_err());
    assert!(kedra_script::values::validate(&json!(i64::MIN)).is_err());
    assert!(kedra_script::values::validate(&json!({"entity":u64::MAX.to_string()})).is_ok());
    let mut deep = Value::Null;
    for _ in 0..34 {
        deep = json!([deep]);
    }
    assert!(kedra_script::values::validate(&deep).is_err());
}

#[test]
fn active_slots_are_bounded_but_terminal_runs_release_capacity() {
    let mut app = app();
    let mut runs = Vec::new();
    for _ in 0..32 {
        runs.push(start(&mut app, "function run() wait.updates(1000) end").unwrap());
    }
    assert!(start(&mut app, "function run() end").is_err());
    kedra_script::cancel(app.world_mut(), runs[0]).unwrap();
    assert!(start(&mut app, "function run() end").is_ok());
}
