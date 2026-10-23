//! Adapter owns narrow component mutation and real Activate forwarding, without rendering.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated native adapter contract assertions"
)]
use bevy::prelude::*;
use kedra_script::{
    EntityHandle, ErrorKind, Program, Scope, Status,
    lua::{self, Lua},
};
use serde_json::Value;

#[derive(Resource, Default)]
struct Captured(Option<EntityHandle>);

fn bindings(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    kedra_script_bevy::install_transform(lua, scope.clone(), ctx)?;
    kedra_script_bevy::install_ui(lua, scope.clone(), ctx)?;
    let read = scope.clone();
    ctx.set(
        "foreign",
        lua.create_function(move |_, ()| {
            read.with_world(|world| Ok(world.resource::<Captured>().0))
        })?,
    )?;
    ctx.set(
        "capture",
        lua.create_function(move |_, handle: lua::AnyUserData| {
            let handle = *handle.borrow::<EntityHandle>()?;
            scope.with_world(|world| {
                world.resource_mut::<Captured>().0 = Some(handle);
                Ok(())
            })
        })?,
    )
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        kedra_script::LuauPlugin,
        kedra_script_bevy::ScriptBevyPlugin,
    ))
    .init_resource::<Captured>();
    app.finish();
    app.cleanup();
    app
}

fn attach(app: &mut App, target: Entity, source: &str) -> kedra_script::Run {
    kedra_script::attach(
        app.world_mut(),
        target,
        Program::sequence("object", "object.luau", source, bindings),
        Value::Null,
    )
    .unwrap()
}

#[test]
fn local_transform_changes_only_owned_translation_and_preserves_other_fields() {
    let mut app = app();
    let original =
        Transform::from_scale(Vec3::splat(2.0)).with_rotation(Quat::from_rotation_x(0.5));
    let target = app.world_mut().spawn(original).id();
    let run = attach(
        &mut app,
        target,
        r#"
        function run(ctx)
            local position = ctx.transform.get_position(ctx.entity)
            assert(position.x == 0 and position.y == 0 and position.z == 0)
            position.x = 99
            assert(ctx.transform.get_position(ctx.entity).x == 0)
            ctx.transform.set_position(ctx.entity, {x = 1, y = 2, z = -3})
            for _, x in {0/0, math.huge, 1e100} do
                assert(not pcall(function() ctx.transform.set_position(ctx.entity, {x = x, y = 0, z = 0}) end))
            end
            return true
        end
    "#,
    );
    app.update();
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Completed(Value::Bool(true)))
    );
    let transform = app.world().get::<Transform>(target).unwrap();
    assert_eq!(transform.translation, Vec3::new(1.0, 2.0, -3.0));
    assert_eq!(transform.scale, original.scale);
    assert_eq!(transform.rotation, original.rotation);
}

#[test]
fn foreign_and_stale_objects_fail_without_mutating_either_target() {
    let mut app = app();
    let foreign = app.world_mut().spawn(Transform::default()).id();
    let capture = attach(
        &mut app,
        foreign,
        "function run(ctx) ctx.capture(ctx.entity) end",
    );
    app.update();
    kedra_script::retire(app.world_mut(), capture).unwrap();
    let target = app.world_mut().spawn(Transform::default()).id();
    let source =
        "function run(ctx) ctx.transform.set_position(ctx.foreign(), {x = 1, y = 2, z = 3}) end";
    let run = attach(&mut app, target, source);
    app.update();
    assert!(
        matches!(kedra_script::status(app.world(), run), Some(Status::Failed(error)) if error.kind == ErrorKind::Permission)
    );
    assert_eq!(
        app.world().get::<Transform>(foreign).unwrap().translation,
        Vec3::ZERO
    );
    assert_eq!(
        app.world().get::<Transform>(target).unwrap().translation,
        Vec3::ZERO
    );
    app.world_mut().despawn(foreign);
    let stale = attach(&mut app, target, source);
    app.update();
    assert!(
        matches!(kedra_script::status(app.world(), stale), Some(Status::Failed(error)) if error.kind == ErrorKind::Stale)
    );
    assert_eq!(
        app.world().get::<Transform>(target).unwrap().translation,
        Vec3::ZERO
    );
}

#[test]
fn ui_rejects_malformed_colors_and_missing_components_before_mutation() {
    let mut app = app();
    let target = app.world_mut().spawn(BackgroundColor(Color::BLACK)).id();
    let run = attach(
        &mut app,
        target,
        r#"
        function run(ctx)
            for _, rgba in {{1,2,3}, {1,2,3,256}, {1,2,3,-1}, {1,2,3,0.5}, {1,2,3,4,5}} do
                assert(not pcall(function() ctx.ui.set_background(ctx.entity, rgba) end))
            end
            assert(not pcall(function() ctx.transform.set_position(ctx.entity, {x=1,y=2,z=3}) end))
            return true
        end
    "#,
    );
    app.update();
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Completed(Value::Bool(true)))
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(target).unwrap().0,
        Color::BLACK
    );
    assert!(app.world().get::<Transform>(target).is_none());
    let missing = app.world_mut().spawn_empty().id();
    let run = attach(
        &mut app,
        missing,
        "function run(ctx) ctx.ui.set_background(ctx.entity, {1,2,3,4}) end",
    );
    app.update();
    assert!(
        matches!(kedra_script::status(app.world(), run), Some(Status::Failed(error)) if error.kind == ErrorKind::InvalidInput)
    );
    assert!(app.world().get::<BackgroundColor>(missing).is_none());
}

#[test]
fn bevy_activation_reaches_behavior_then_child_with_original_owned_object() {
    let mut app = app();
    let target = app
        .world_mut()
        .spawn((BackgroundColor(Color::BLACK), Transform::default()))
        .id();
    let mut program = Program::behavior(
        "switch",
        "switch.luau",
        r#"
        function start(self, ctx)
            ctx.events.on(ctx.entity, "activate", function()
                ctx.sequences.start("./change")
            end)
        end
    "#,
        bindings,
    );
    program.events = &["activate"];
    program.sequences.push(Program::sequence(
        "change",
        "change.luau",
        r#"
        function run(ctx)
            ctx.ui.set_background(ctx.entity, {30,100,190,255})
            ctx.transform.set_position(ctx.entity, {x=4,y=5,z=6})
        end
    "#,
        bindings,
    ));
    let run = kedra_script::attach(app.world_mut(), target, program, Value::Null).unwrap();
    app.update();
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: target });
    assert_eq!(
        app.world().get::<BackgroundColor>(target).unwrap().0,
        Color::BLACK
    );
    app.update();
    assert_eq!(
        kedra_script::status(app.world(), run),
        Some(&Status::Running)
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(target).unwrap().0,
        Color::srgba_u8(30, 100, 190, 255)
    );
    assert_eq!(
        app.world().get::<Transform>(target).unwrap().translation,
        Vec3::new(4.0, 5.0, 6.0)
    );
}
