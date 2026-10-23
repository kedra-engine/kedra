//! Runtime owns deferred child admission, independent bindings and owner revocation.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated child lifecycle contract assertions"
)]
use crate::support::*;
use kedra_script::{Program, Status};
use serde_json::Value;

fn parent(source: &str, child: &str) -> Program {
    let mut program = Program::behavior("parent", "scripts/parent.luau", source, game_api);
    program.sequences.push(Program::sequence(
        "child",
        "scripts/child.luau",
        child,
        game_api,
    ));
    program
}

#[test]
fn child_admission_is_deferred_and_outcomes_are_owned_and_observable() {
    let mut app = app();
    let program = parent(
        r#"
        function start(self, ctx)
            self.job = ctx.sequences.start("./child", {count = 7})
            assert(ctx.sequences.status(self.job).state == "queued")
            assert(ctx.game.count() == 0)
        end
        function update(self, ctx)
            local outcome = ctx.sequences.status(self.job)
            if outcome.state == "completed" then
                assert(outcome.result.count == 7)
                ctx.game.increment()
            end
        end
    "#,
        "function run(ctx, args) ctx.game.increment() return args end",
    );
    let run = kedra_script::start(app.world_mut(), program, Value::Null).unwrap();
    app.update();
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Running)
    );
    assert_eq!(app.world().resource::<Counter>().0, 1);
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 2);
}

#[test]
fn child_permissions_and_child_bindings_do_not_inherit_parent_authority() {
    let mut app = app();
    let mut program = parent(
        r#"
        function start(self, ctx)
            assert(not pcall(function() ctx.sequences.start("./unapproved") end))
            self.job = ctx.sequences.start("./child")
        end
        function update(self, ctx)
            local outcome = ctx.sequences.status(self.job)
            if outcome.state == "failed" then
                assert(outcome.error.kind == "runtime")
                assert(outcome.error.path == "scripts/child.luau")
                ctx.game.increment()
            end
        end
    "#,
        "function run(ctx) assert(ctx.sequences == nil and ctx.events == nil) ctx.game.increment() end",
    );
    program
        .sources
        .insert("scripts/unapproved.luau", "error('must not execute')")
        .unwrap();
    program.sequences[0].bindings = |_, _, _| Ok(());
    let run = kedra_script::start(app.world_mut(), program, Value::Null).unwrap();
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 0);
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 1);
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Running)
    );
}

#[test]
fn pending_cancel_and_active_cancel_prevent_later_child_effects() {
    let mut app = app();
    let program = parent(
        r#"
        function start(self, ctx)
            local cancelled = ctx.sequences.start("./child")
            ctx.sequences.cancel(cancelled)
            ctx.sequences.cancel(cancelled)
            assert(ctx.sequences.status(cancelled).state == "cancelled")
            self.job = ctx.sequences.start("./child")
        end
        function update(self, ctx)
            if ctx.sequences.status(self.job).state == "running" then
                ctx.sequences.cancel(self.job)
            end
        end
    "#,
        "function run(ctx) ctx.game.increment() wait.updates(1) ctx.game.increment() end",
    );
    let run = kedra_script::start(app.world_mut(), program, Value::Null).unwrap();
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Running)
    );
    assert_eq!(app.world().resource::<Counter>().0, 1);
}

#[test]
fn owner_cancel_replace_failure_and_despawn_revoke_active_children() {
    for action in ["cancel", "replace", "fail", "despawn"] {
        let mut app = app();
        let target = app.world_mut().spawn_empty().id();
        let source = if action == "fail" {
            r#"
            function start(self, ctx) self.job = ctx.sequences.start("./child") end
            function update(self, ctx)
                if ctx.sequences.status(self.job).state == "running" then error("stop owner") end
            end
        "#
        } else {
            "function start(self, ctx) self.job = ctx.sequences.start('./child') end"
        };
        let program = parent(
            source,
            "function run(ctx) ctx.game.increment() wait.updates(1) ctx.game.increment() end",
        );
        let run = kedra_script::attach(app.world_mut(), target, program, Value::Null).unwrap();
        app.update();
        assert_eq!(app.world().resource::<Counter>().0, 1);
        match action {
            "cancel" => kedra_script::cancel(app.world_mut(), run).unwrap(),
            "replace" => {
                kedra_script::replace(
                    app.world_mut(),
                    run,
                    Program::behavior("new", "new.luau", "function start() end", game_api),
                    Value::Null,
                )
                .unwrap();
            }
            "despawn" => {
                app.world_mut().despawn(target);
            }
            _ => (),
        }
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<Counter>().0, 1, "{action}");
    }
}

#[test]
fn retained_handles_and_global_admission_remain_bounded() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<kedra_script::Capacity>()
        .sequences = 0;
    let program = parent(
        r#"
        function start(self, ctx)
            self.jobs = {}
            for i = 1, 32 do self.jobs[i] = ctx.sequences.start("./child") end
            assert(not pcall(function() ctx.sequences.start("./child") end))
        end
        function update(self, ctx)
            if ctx.sequences.status(self.jobs[1]).state == "failed" then
                assert(ctx.sequences.status(self.jobs[1]).error.kind == "capacity")
                assert(not pcall(function() ctx.sequences.start("./child") end))
                ctx.game.increment()
            end
        end
    "#,
        "function run(ctx) ctx.game.increment() end",
    );
    let run = kedra_script::start(app.world_mut(), program, Value::Null).unwrap();
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 0);
    app.update();
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Running)
    );
    assert_eq!(app.world().resource::<Counter>().0, 1);
}

#[test]
fn child_resolution_uses_calling_module_aliases_without_poisoning_module_exports() {
    let mut app = app();
    let mut program = Program::behavior(
        "parent",
        "scripts/parent.luau",
        r#"
        local helper = require("./lib/helper")
        function start(self, ctx)
            self.job = helper(ctx)
            assert(require("./lib/helper") == helper)
        end
        function update(self, ctx)
            local outcome = ctx.sequences.status(self.job)
            if outcome.state == "completed" then
                assert(outcome.result == "Кедра")
                ctx.game.increment()
            end
        end
    "#,
        game_api,
    );
    program
        .sources
        .insert(".luaurc", r#"{"aliases":{"game":"./scripts"}}"#)
        .unwrap();
    program
        .sources
        .insert(
            "scripts/lib/helper.luau",
            r#"
        return function(ctx)
            return ctx.sequences.start("@game/child", "Кедра")
        end
    "#,
        )
        .unwrap();
    program.sequences.push(Program::sequence(
        "child",
        "scripts/child.luau",
        "function run(ctx, args) return args end",
        game_api,
    ));
    let run = kedra_script::start(app.world_mut(), program, Value::Null).unwrap();
    app.update();
    app.update();
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Running)
    );
    assert_eq!(app.world().resource::<Counter>().0, 1);
}
