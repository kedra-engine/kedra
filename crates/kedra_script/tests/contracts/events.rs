//! Runtime owns curated notification order, bounded delivery and subscription lifetime.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated subscription contract assertions"
)]
use crate::support::*;
use kedra_script::{ErrorKind, Program, Status, events::notify};
use serde_json::{Value, json};

fn subscribe(
    app: &mut bevy::prelude::App,
    source: &str,
) -> (bevy::prelude::Entity, kedra_script::Run) {
    let target = app.world_mut().spawn_empty().id();
    let mut program = Program::behavior("events", "events.luau", source, game_api);
    program.events = &["activate"];
    let run = kedra_script::attach(app.world_mut(), target, program, Value::Null).unwrap();
    app.update();
    (target, run)
}

#[test]
fn notifications_are_ordered_owned_deferred_and_disconnection_clears_pending_delivery() {
    let mut app = app();
    let (target, run) = subscribe(
        &mut app,
        r#"
        function start(self, ctx)
            assert(not pcall(function() ctx.events.on(ctx.entity, "unapproved", function() end) end))
            self.expected = 1
            self.connection = ctx.events.on(ctx.entity, "activate", function(payload)
                assert(payload.index == self.expected)
                self.expected += 1
                ctx.game.increment()
                if payload.index == 2 then self.connection:disconnect() end
            end)
        end
    "#,
    );
    for index in 1..=3 {
        notify(app.world_mut(), target, "activate", json!({"index":index})).unwrap();
    }
    assert_eq!(app.world().resource::<Counter>().0, 0);
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 2);
    notify(app.world_mut(), target, "activate", json!({"index":4})).unwrap();
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 2);
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Running)
    );
}

#[test]
fn queued_events_are_discarded_on_revocation_and_never_run_on_other_targets() {
    let mut app = app();
    let (target, run) = subscribe(
        &mut app,
        "function start(self, ctx) ctx.events.on(ctx.entity, 'activate', function() ctx.game.increment() end) end",
    );
    let foreign = app.world_mut().spawn_empty().id();
    notify(app.world_mut(), foreign, "activate", json!({})).unwrap();
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 0);
    notify(app.world_mut(), target, "activate", json!({})).unwrap();
    kedra_script::cancel(app.world_mut(), run).unwrap();
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 0);
}

#[test]
fn queue_overflow_fails_owner_without_partial_batch_and_delivery_is_per_update_bounded() {
    let mut app = app();
    let (target, run) = subscribe(
        &mut app,
        "function start(self, ctx) ctx.events.on(ctx.entity, 'activate', function() ctx.game.increment() end) end",
    );
    for _ in 0..17 {
        notify(app.world_mut(), target, "activate", json!({})).unwrap();
    }
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 16);
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 17);
    for _ in 0..128 {
        notify(app.world_mut(), target, "activate", json!({})).unwrap();
    }
    assert_eq!(
        notify(app.world_mut(), target, "activate", json!({}))
            .unwrap_err()
            .kind,
        ErrorKind::Budget
    );
    app.update();
    assert!(
        matches!(kedra_script::status(app.world(), run), Some(Status::Failed(error)) if error.kind == ErrorKind::Budget)
    );
    assert_eq!(app.world().resource::<Counter>().0, 17);
}

#[test]
fn yielding_handlers_fail_owner_and_subscription_capacity_rejects_without_extra_handler() {
    let mut app = app();
    let (target, run) = subscribe(
        &mut app,
        r#"
        function start(self, ctx)
            for i = 1, 32 do
                ctx.events.on(ctx.entity, "activate", function()
                    wait.updates(1)
                    ctx.game.increment()
                end)
            end
            assert(not pcall(function() ctx.events.on(ctx.entity, "activate", function() end) end))
        end
    "#,
    );
    notify(app.world_mut(), target, "activate", json!({})).unwrap();
    app.update();
    assert!(
        matches!(kedra_script::status(app.world(), run), Some(Status::Failed(error)) if error.kind == ErrorKind::Runtime)
    );
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 0);
}

#[test]
fn reentrant_notifications_wait_until_next_update_and_byte_overflow_revokes_owner() {
    let mut app = app();
    let target = app.world_mut().spawn_empty().id();
    let mut program = Program::behavior(
        "events",
        "events.luau",
        r#"
        function start(self, ctx)
            ctx.events.on(ctx.entity, "activate", function()
                ctx.game.increment()
                ctx.emit(ctx.entity)
            end)
        end
    "#,
        |lua, scope, ctx| {
            game_api(lua, scope.clone(), ctx)?;
            ctx.set(
                "emit",
                lua.create_function(move |_, handle: kedra_script::lua::AnyUserData| {
                    let handle = *handle.borrow::<kedra_script::EntityHandle>()?;
                    scope.with_world(|world| {
                        let target = scope.resolve_entity(world, &handle)?;
                        notify(world, target, "activate", json!({}))
                            .map_err(kedra_script::lua::Error::external)
                    })
                })?,
            )
        },
    );
    program.events = &["activate"];
    let run = kedra_script::attach(app.world_mut(), target, program, Value::Null).unwrap();
    app.update();
    notify(app.world_mut(), target, "activate", json!({})).unwrap();
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 1);
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 2);
    let payload = Value::String("x".repeat(40_000));
    notify(app.world_mut(), target, "activate", payload.clone()).unwrap();
    assert_eq!(
        notify(app.world_mut(), target, "activate", payload)
            .unwrap_err()
            .kind,
        ErrorKind::Budget
    );
    app.update();
    assert!(
        matches!(kedra_script::status(app.world(), run), Some(Status::Failed(error)) if error.kind == ErrorKind::Budget)
    );
    assert_eq!(app.world().resource::<Counter>().0, 2);
}
