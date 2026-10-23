//! Persistent lifecycle, native schedule phases and revocation through real Bevy/BMS.
#![allow(clippy::unwrap_used, reason = "Isolated behavior contract assertions")]
use crate::support::*;
use bevy::prelude::*;
use bevy_mod_scripting::bindings::ThreadWorldContainer;
use kedra_script::{
    EntityHandle, Program, Run, Scope, Status,
    lua::{self, Lua},
};
use serde_json::Value;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Resource, Default, Clone)]
struct Log(Arc<Mutex<Vec<(String, i32)>>>);

#[derive(Resource)]
struct SavedEntity(EntityHandle);

fn bindings(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    game_api(lua, scope.clone(), ctx)?;
    let log = ThreadWorldContainer
        .try_get_context()
        .unwrap()
        .world
        .with_world_access(|world| world.resource::<Log>().clone())
        .unwrap();
    // Explicit telemetry capability has no ECS effects and remains usable in stop.
    ctx.set(
        "record",
        lua.create_function(move |_, event: (String, i32)| {
            log.0.lock().unwrap().push(event);
            Ok(())
        })?,
    )?;
    let destroy = scope.clone();
    ctx.set(
        "destroy_owner",
        lua.create_function(move |_, handle: lua::AnyUserData| {
            let handle = *handle.borrow::<EntityHandle>()?;
            destroy.with_world(|world| {
                let entity = handle.resolve(world)?;
                world.despawn(entity);
                Ok(())
            })
        })?,
    )?;
    ctx.set(
        "touch_owner",
        lua.create_function(move |_, handle: lua::AnyUserData| {
            let handle = *handle.borrow::<EntityHandle>()?;
            scope.with_world(|world| {
                let entity = handle.resolve(world)?;
                world.entity_mut(entity).insert(Name::new("Script touched"));
                world.insert_resource(SavedEntity(handle));
                Ok(())
            })
        })?,
    )
}

fn host() -> App {
    let mut app = app();
    app.init_resource::<Log>();
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app
}

fn behavior(app: &mut App, parent: Entity, source: &str) -> Run {
    kedra_script::attach(
        app.world_mut(),
        parent,
        Program::behavior("behavior", "behavior.luau", source, bindings),
        Value::Null,
    )
    .unwrap()
}

#[test]
fn instances_have_independent_state_start_once_and_run_fixed_callbacks_in_fixed_schedule() {
    let mut app = host();
    let parent = app.world_mut().spawn_empty().id();
    let source = r#"
        function start(self, ctx)
            self.count = 0
            ctx.record('start', 0)
            ctx.touch_owner(ctx.entity)
        end
        function update(self, ctx, dt)
            self.count += 1
            ctx.record('update', self.count)
        end
        function fixed_update(self, ctx, dt)
            assert(dt > 0)
            ctx.record('fixed', self.count)
        end
    "#;
    let first = behavior(&mut app, parent, source);
    let second = behavior(&mut app, parent, source);
    for _ in 0..3 {
        app.update();
    }
    let events = app.world().resource::<Log>().0.lock().unwrap().clone();
    assert_eq!(events.iter().filter(|(name, _)| name == "start").count(), 2);
    assert_eq!(
        events
            .iter()
            .filter(|(name, count)| name == "update" && *count == 3)
            .count(),
        2
    );
    assert!(!events.iter().any(|(name, _)| name == "fixed"));
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .advance_by(std::time::Duration::from_millis(20));
    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut().run_schedule(FixedUpdate);
    let events = app.world().resource::<Log>().0.lock().unwrap();
    assert_eq!(events.iter().filter(|(name, _)| name == "fixed").count(), 4);
    assert_eq!(
        app.world().get::<Name>(parent).unwrap().as_str(),
        "Script touched"
    );
    assert_eq!(
        kedra_script::status(app.world(), first),
        Some(&Status::Running)
    );
    assert_eq!(
        kedra_script::status(app.world(), second),
        Some(&Status::Running)
    );
}

#[test]
fn cancellation_and_parent_despawn_notify_stop_once_with_effects_already_revoked() {
    let mut app = host();
    let parent = app.world_mut().spawn_empty().id();
    let source = r#"
        function start(self, ctx) ctx.game.increment() end
        function stop(self, ctx, reason)
            assert(not pcall(ctx.game.increment))
            ctx.record(reason, 1)
        end
    "#;
    let cancelled = behavior(&mut app, parent, source);
    let despawned = behavior(&mut app, parent, source);
    app.update();
    kedra_script::cancel(app.world_mut(), cancelled).unwrap();
    kedra_script::cancel(app.world_mut(), cancelled).unwrap();
    app.world_mut().despawn(parent);
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 2);
    let events = app.world().resource::<Log>().0.lock().unwrap();
    assert_eq!(
        *events,
        vec![("cancelled".into(), 1), ("despawned".into(), 1)]
    );
    assert!(kedra_script::status(app.world(), despawned).is_none());
}

#[test]
fn start_can_remove_its_owner_without_running_update_or_leaking_the_vm() {
    let mut app = host();
    let parent = app.world_mut().spawn_empty().id();
    let run = behavior(
        &mut app,
        parent,
        r#"
        function start(self, ctx) ctx.destroy_owner(ctx.entity) end
        function update(self, ctx, dt) ctx.record('unexpected update', 0) end
        function stop(self, ctx, reason) ctx.record(reason, 1) end
    "#,
    );
    app.update();
    assert!(kedra_script::status(app.world(), run).is_none());
    assert_eq!(
        *app.world().resource::<Log>().0.lock().unwrap(),
        vec![("despawned".into(), 1)]
    );
    assert_eq!(
        app.world()
            .resource::<Assets<bevy_mod_scripting::asset::ScriptAsset>>()
            .len(),
        0
    );
}

#[test]
fn entity_context_rejects_foreign_worlds_and_despawned_targets() {
    let mut app = host();
    let parent = app.world_mut().spawn_empty().id();
    let run = behavior(
        &mut app,
        parent,
        "function start(self, ctx) ctx.touch_owner(ctx.entity) end",
    );
    app.update();
    let handle = app.world().resource::<SavedEntity>().0;
    assert_eq!(handle.resolve(app.world()).unwrap(), parent);
    let foreign = World::new();
    assert!(handle.resolve(&foreign).is_err());
    assert_eq!(
        app.world().get::<Name>(parent).unwrap().as_str(),
        "Script touched"
    );
    app.world_mut().despawn(parent);
    assert!(handle.resolve(app.world()).is_err());
    assert!(kedra_script::status(app.world(), run).is_none());
    assert_eq!(app.world().resource::<Counter>().0, 0);
}

#[test]
fn yielding_or_failing_callbacks_are_terminal_and_stop_failure_cannot_prevent_cleanup() {
    for callback in [
        "wait.updates(1)",
        "coroutine.yield()",
        "error('expected failure')",
    ] {
        let mut app = host();
        let parent = app.world_mut().spawn_empty().id();
        let source = format!(
            r#"
            function start(self, ctx) ctx.game.increment() end
            function update(self, ctx, dt) {callback}; ctx.game.increment() end
            function stop(self, ctx, reason) ctx.record('stopped', 1); error('stop failed') end
        "#
        );
        let run = behavior(&mut app, parent, &source);
        app.update();
        app.update();
        assert!(matches!(
            kedra_script::status(app.world(), run),
            Some(Status::Failed(_))
        ));
        assert_eq!(app.world().resource::<Counter>().0, 1);
        assert_eq!(
            *app.world().resource::<Log>().0.lock().unwrap(),
            vec![("stopped".into(), 1)]
        );
        assert_eq!(
            app.world()
                .resource::<Assets<bevy_mod_scripting::asset::ScriptAsset>>()
                .len(),
            0
        );
    }
}

#[test]
fn replacement_at_capacity_preserves_old_on_failure_and_retires_it_after_success() {
    let mut app = host();
    app.world_mut()
        .resource_mut::<kedra_script::Capacity>()
        .behaviors = 1;
    let parent = app.world_mut().spawn_empty().id();
    let old = behavior(
        &mut app,
        parent,
        "function update(self, ctx, dt) ctx.game.increment() end",
    );
    app.update();
    let invalid = Program::behavior("bad", "behavior.luau", "return {}", bindings);
    assert!(kedra_script::replace(app.world_mut(), old, invalid, Value::Null).is_err());
    assert_eq!(
        kedra_script::status(app.world(), old),
        Some(&Status::Running)
    );
    let replacement = Program::behavior(
        "new",
        "behavior.luau",
        "function start(self, ctx) ctx.game.increment() end",
        bindings,
    );
    let new = kedra_script::replace(app.world_mut(), old, replacement, Value::Null).unwrap();
    app.update();
    assert!(kedra_script::status(app.world(), old).is_none());
    assert_eq!(
        kedra_script::status(app.world(), new),
        Some(&Status::Running)
    );
    assert_eq!(app.world().resource::<Counter>().0, 2);
    let sequence = Program::sequence(
        "sequence",
        "sequence.luau",
        "function run(ctx) return 42 end",
        bindings,
    );
    app.world_mut()
        .resource_mut::<kedra_script::Capacity>()
        .sequences = 1;
    let old = kedra_script::start(app.world_mut(), sequence.clone(), Value::Null).unwrap();
    let new = kedra_script::replace(app.world_mut(), old, sequence, Value::Null).unwrap();
    assert_eq!(finish(&mut app, new), Ok(serde_json::json!(42)));
}

struct VmDrop(Arc<AtomicUsize>);
impl Drop for VmDrop {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[derive(Resource, Default)]
struct Drops(Arc<AtomicUsize>);
fn tracked(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    game_api(lua, scope, ctx)?;
    let drops = ThreadWorldContainer
        .try_get_context()
        .unwrap()
        .world
        .with_world_access(|world| world.resource::<Drops>().0.clone())
        .unwrap();
    lua.set_app_data(VmDrop(drops));
    Ok(())
}

#[test]
fn terminal_evidence_does_not_retain_vm_handles() {
    let mut app = host();
    app.init_resource::<Drops>();
    let run = kedra_script::start(
        app.world_mut(),
        Program::sequence(
            "tracked",
            "tracked.luau",
            "function run(ctx) return 1 end",
            tracked,
        ),
        Value::Null,
    )
    .unwrap();
    app.update();
    assert_eq!(app.world().resource::<Drops>().0.load(Ordering::SeqCst), 1);
    assert!(matches!(
        kedra_script::status(app.world(), run),
        Some(Status::Completed(_))
    ));
}

#[test]
fn owner_revocation_releases_subscription_closures_and_child_vms_with_terminal_evidence_retained() {
    let mut app = host();
    app.init_resource::<Drops>();
    let target = app.world_mut().spawn_empty().id();
    let mut program = Program::behavior(
        "tracked",
        "tracked.luau",
        r#"
        function start(self, ctx)
            self.job = ctx.sequences.start("./child")
            self.connection = ctx.events.on(ctx.entity, "activate", function()
                ctx.game.increment()
                self.connection:disconnect()
            end)
        end
    "#,
        tracked,
    );
    program.events = &["activate"];
    program.sequences.push(Program::sequence(
        "child",
        "child.luau",
        "function run(ctx) wait.updates(100) ctx.game.increment() end",
        tracked,
    ));
    let run = kedra_script::attach(app.world_mut(), target, program, Value::Null).unwrap();
    app.update();
    kedra_script::events::notify(app.world_mut(), target, "activate", serde_json::json!({}))
        .unwrap();
    kedra_script::cancel(app.world_mut(), run).unwrap();
    assert_eq!(app.world().resource::<Drops>().0.load(Ordering::SeqCst), 2);
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Cancelled)
    );
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 0);
}
