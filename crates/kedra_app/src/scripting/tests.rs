//! Actual BRP/native-system/Luau integration, without a mock VM or QA driver tests.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated application scripting contract assertions"
)]
use super::*;
use bevy_remote::{BrpMessage, BrpSender};
use kedra_script::{
    Scope,
    lua::{self, Lua},
};

fn host() -> (App, String, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::input::InputPlugin,
        bevy::window::WindowPlugin {
            primary_window: None,
            exit_condition: bevy::window::ExitCondition::DontExit,
            close_when_requested: false,
            ..default()
        },
        crate::KedraAppPlugin,
    ));
    app.finish();
    app.cleanup();
    let session = app.world().resource::<crate::AppSession>().id.clone();
    let window = app
        .world_mut()
        .spawn((Window::default(), bevy::window::PrimaryWindow))
        .id();
    app.update();
    (app, session, window)
}
fn rpc(app: &mut App, method: &str, params: Value) -> BrpResult {
    let (sender, receiver) = async_channel::bounded(1);
    app.world()
        .resource::<BrpSender>()
        .try_send(BrpMessage {
            method: method.into(),
            params: Some(params),
            sender,
        })
        .unwrap();
    app.update();
    receiver.try_recv().unwrap()
}
fn receipt(app: &mut App, id: Uuid) -> Value {
    for _ in 0..100 {
        let value = rpc(app, "app.operation", json!({"id":id})).unwrap();
        if value["state"] != "pending" {
            return value;
        }
    }
    panic!("Scenario did not complete");
}

#[test]
fn shared_input_sequence_uses_real_queue_and_opaque_window_identity() {
    let (mut app, session, window) = host();
    let id = Uuid::new_v4();
    let admitted = rpc(&mut app, "app.scenario", json!({"session":session,"id":id,"scenario":"app.input",
        "arguments":{"window":window.to_bits().to_string(),"input":{"kind":"move","position":[10,10]}}})).unwrap();
    assert_eq!(admitted["state"], "pending");
    assert_eq!(receipt(&mut app, id)["state"], "succeeded");
    assert!(!app.world().resource::<crate::input::InputQueue>().busy());
    assert!(app.world().resource::<Active>().0.is_none());
}

#[test]
fn remote_source_unknown_program_and_stale_session_allocate_no_receipt() {
    let (mut app, session, _) = host();
    for request in [
        json!({"session":session,"scenario":"app.input","arguments":{},"source":"function run() end"}),
        json!({"session":session,"scenario":"not-registered","arguments":{}}),
        json!({"session":"stale","scenario":"app.input","arguments":{}}),
    ] {
        let id = Uuid::new_v4();
        let mut request = request;
        request["id"] = json!(id);
        assert!(rpc(&mut app, "app.scenario", request).is_err());
        assert!(app.world().resource::<Operations>().get(id).is_none());
    }
}

#[derive(Resource, Default)]
struct Counter(u32);
fn count_api(lua: &Lua, scope: Scope, ctx: &lua::Table) -> lua::Result<()> {
    install_app_api(lua, scope.clone(), ctx)?;
    ctx.set(
        "increment",
        lua.create_function(move |_, ()| {
            scope.with_world(|world| {
                world.resource_mut::<Counter>().0 += 1;
                Ok(())
            })
        })?,
    )
}

#[test]
fn scenario_retries_are_idempotent_and_conflicting_ids_or_concurrent_runs_fail() {
    let (mut app, session, _) = host();
    app.init_resource::<Counter>();
    register(
        &mut app,
        Program::sequence(
            "counter",
            "entry.luau",
            "function run(ctx) ctx.increment(); wait.updates(10); return {count = 1} end",
            count_api,
        ),
    );
    let id = Uuid::new_v4();
    let request = json!({"session":session,"id":id,"scenario":"counter","arguments":{}});
    rpc(&mut app, "app.scenario", request.clone()).unwrap();
    rpc(&mut app, "app.scenario", request.clone()).unwrap();
    assert_eq!(app.world().resource::<Counter>().0, 1);
    let mut changed = request.clone();
    changed["arguments"] = json!({"changed":true});
    assert!(rpc(&mut app, "app.scenario", changed).is_err());
    let mut concurrent = request;
    concurrent["id"] = json!(Uuid::new_v4());
    assert!(rpc(&mut app, "app.scenario", concurrent).is_err());
    assert_eq!(receipt(&mut app, id)["detail"], json!({"count":1}));
    assert_eq!(app.world().resource::<Counter>().0, 1);
}

#[test]
fn script_errors_scope_denial_and_load_failure_leave_application_usable() {
    let (mut app, session, _) = host();
    for (name, source) in [
        (
            "denied",
            "function run(ctx) assert(ctx.editor == nil and ctx.app.effect == nil); ctx.editor.state() end",
        ),
        (
            "failure",
            "function run() error('expected scenario failure') end",
        ),
        ("syntax", "bad syntax ("),
    ] {
        register(
            &mut app,
            Program::sequence(name, "entry.luau", source, install_app_api),
        );
        let id = Uuid::new_v4();
        rpc(
            &mut app,
            "app.scenario",
            json!({"session":session,"id":id,"scenario":name,"arguments":{}}),
        )
        .unwrap();
        assert_eq!(receipt(&mut app, id)["state"], "failed");
        assert!(app.world().resource::<Active>().0.is_none());
        assert!(rpc(&mut app, "app.describe", Value::Null).is_ok());
    }
}

#[test]
fn native_handles_are_strings_and_unsupported_stale_windows_fail_without_rounding() {
    let (mut app, session, window) = host();
    let camera = app
        .world_mut()
        .spawn((
            Camera::default(),
            bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window)),
        ))
        .id();
    let mut target = ComputedUiTargetCamera::default();
    let mut reflected = bevy::reflect::structs::DynamicStruct::default();
    reflected.insert("camera", Some(camera));
    target.apply(&reflected);
    let node = app
        .world_mut()
        .spawn((
            kedra_ui::ElementId::new("identity"),
            ComputedNode {
                size: Vec2::splat(20.),
                ..default()
            },
            UiGlobalTransform::default(),
            InheritedVisibility::VISIBLE,
            target,
        ))
        .id();
    register(
        &mut app,
        Program::sequence(
            "identity",
            "entry.luau",
            "function run(ctx) local id = ctx.app.window(); assert(type(id) == 'string'); local node = ctx.app.node('identity'); assert(type(node.entity) == 'string' and node.window == id); assert(ctx.app.node('missing') == nil); return {window = id, entity = node.entity} end",
            install_app_api,
        ),
    );
    let id = Uuid::new_v4();
    rpc(
        &mut app,
        "app.scenario",
        json!({"session":session,"id":id,"scenario":"identity","arguments":{}}),
    )
    .unwrap();
    assert_eq!(
        receipt(&mut app, id)["detail"]["window"],
        window.to_bits().to_string()
    );
    assert_eq!(
        receipt(&mut app, id)["detail"]["entity"],
        node.to_bits().to_string()
    );
    app.world_mut().spawn((
        kedra_ui::ElementId::new("identity"),
        ComputedNode {
            size: Vec2::splat(20.),
            ..default()
        },
        UiGlobalTransform::default(),
        InheritedVisibility::VISIBLE,
        target,
    ));
    let ambiguous = Uuid::new_v4();
    rpc(
        &mut app,
        "app.scenario",
        json!({"session":session,"id":ambiguous,"scenario":"identity","arguments":{}}),
    )
    .unwrap();
    let rejected = receipt(&mut app, ambiguous);
    assert_eq!(rejected["state"], "failed");
    assert!(
        rejected["detail"]
            .to_string()
            .contains("Duplicate visible UI ID: identity")
    );
    assert!(app.world().get_entity(node).is_ok());
    assert!(!app.world().resource::<crate::input::InputQueue>().busy());
    let bad = Uuid::new_v4();
    rpc(
        &mut app,
        "app.scenario",
        json!({"session":session,"id":bad,"scenario":"app.input",
        "arguments":{"window":u64::MAX.to_string(),"input":{"kind":"move","position":[10,10]}}}),
    )
    .unwrap();
    assert_eq!(receipt(&mut app, bad)["state"], "failed");
}

#[test]
fn resize_sequence_delivers_to_its_window_without_claiming_renderer_readiness() {
    let (mut app, session, window) = host();
    let other = app.world_mut().spawn(Window::default()).id();
    let other_size = app.world().get::<Window>(other).unwrap().physical_size();
    let id = Uuid::new_v4();
    rpc(
        &mut app,
        "app.scenario",
        json!({"session":session,"id":id,"scenario":"app.resize",
        "arguments":{"window":window.to_bits().to_string(),"width":800,"height":600,"dpr":2}}),
    )
    .unwrap();
    let result = receipt(&mut app, id);
    assert_eq!(result["state"], "succeeded");
    assert_eq!(
        app.world().get::<Window>(window).unwrap().size(),
        Vec2::new(800., 600.)
    );
    assert_eq!(
        app.world().get::<Window>(other).unwrap().physical_size(),
        other_size
    );
    assert!(!app.world().resource::<crate::input::InputQueue>().busy());
}

#[test]
fn performance_observation_is_read_only_and_available_to_scoped_luau() {
    let (mut app, session, _) = host();
    let text = app.world_mut().spawn(Text::new("Retained 日本語")).id();
    register(
        &mut app,
        Program::sequence(
            "performance",
            "performance.luau",
            "function run(ctx) return ctx.app.performance() end",
            install_app_api,
        ),
    );
    let id = Uuid::new_v4();
    rpc(
        &mut app,
        "app.scenario",
        json!({"session":session,"id":id,"scenario":"performance","arguments":{}}),
    )
    .unwrap();
    let result = receipt(&mut app, id);
    assert_eq!(result["state"], "succeeded");
    assert_eq!(result["detail"]["enabled"], false);
    assert!(result["detail"]["update_intervals"]["p95_ms"].is_null());
    assert_eq!(
        app.world().get::<Text>(text).map(|text| text.0.as_str()),
        Some("Retained 日本語")
    );
    assert!(!app.world().resource::<crate::input::InputQueue>().busy());
}

#[test]
fn element_click_waits_for_window_focus_without_forcing_it_and_rejects_retirement() {
    for retire in [false, true] {
        let (mut app, session, window) = host();
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        let camera = app
            .world_mut()
            .spawn((
                Camera::default(),
                bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window)),
            ))
            .id();
        let mut target = ComputedUiTargetCamera::default();
        let mut reflected = bevy::reflect::structs::DynamicStruct::default();
        reflected.insert("camera", Some(camera));
        target.apply(&reflected);
        let node = app
            .world_mut()
            .spawn((
                kedra_ui::ElementId::new("focus-target"),
                ComputedNode {
                    size: Vec2::splat(20.),
                    ..default()
                },
                UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::splat(50.))),
                InheritedVisibility::VISIBLE,
                target,
            ))
            .id();
        let id = Uuid::new_v4();
        rpc(
            &mut app,
            "app.scenario",
            json!({"session":session,"id":id,"scenario":"app.click","arguments":{"id":"focus-target"}}),
        )
        .unwrap();
        let mut moves = app
            .world()
            .resource::<Messages<bevy::window::CursorMoved>>()
            .get_cursor_current();
        for _ in 0..8 {
            assert_eq!(
                rpc(&mut app, "app.operation", json!({"id":id})).unwrap()["state"],
                "pending"
            );
            assert!(!app.world().get::<Window>(window).unwrap().focused);
            assert!(!app.world().resource::<crate::input::InputQueue>().busy());
            assert_eq!(
                moves
                    .read(
                        app.world()
                            .resource::<Messages<bevy::window::CursorMoved>>()
                    )
                    .count(),
                0
            );
        }
        if retire {
            app.world_mut().despawn(window);
            let rejected = receipt(&mut app, id);
            assert_eq!(rejected["state"], "failed");
            assert!(
                rejected["detail"]
                    .to_string()
                    .contains("window was retired")
            );
            assert!(!app.world().resource::<crate::input::InputQueue>().busy());
        } else {
            app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
            assert_eq!(receipt(&mut app, id)["state"], "succeeded");
        }
        assert!(app.world().entities().contains(node));
    }
}

#[test]
fn cancellation_is_scoped_idempotent_and_never_rolls_back_prior_effects() {
    let (mut app, session, _) = host();
    app.init_resource::<Counter>();
    register(
        &mut app,
        Program::sequence(
            "cancel",
            "entry.luau",
            "function run(ctx) ctx.increment(); wait.updates(1000); ctx.increment() end",
            count_api,
        ),
    );
    let id = Uuid::new_v4();
    rpc(
        &mut app,
        "app.scenario",
        json!({"session":session,"id":id,"scenario":"cancel","arguments":{}}),
    )
    .unwrap();
    assert!(
        rpc(
            &mut app,
            "app.scenario_cancel",
            json!({"session":session,"id":Uuid::new_v4(),"operation":Uuid::new_v4()})
        )
        .is_err()
    );
    let cancel = json!({"session":session,"id":Uuid::new_v4(),"operation":id});
    assert!(
        app.world_mut()
            .resource_mut::<Messages<bevy::window::RequestRedraw>>()
            .drain()
            .next()
            .is_some()
    );
    assert_eq!(
        rpc(&mut app, "app.scenario_cancel", cancel.clone()).unwrap()["state"],
        "succeeded"
    );
    assert_eq!(
        rpc(&mut app, "app.scenario_cancel", cancel).unwrap()["state"],
        "succeeded"
    );
    assert_eq!(receipt(&mut app, id)["state"], "failed");
    assert_eq!(app.world().resource::<Counter>().0, 1);
    assert!(app.world().resource::<Active>().0.is_none());
    app.world_mut()
        .resource_mut::<Messages<bevy::window::RequestRedraw>>()
        .clear();
    app.update();
    assert!(
        app.world()
            .resource::<Messages<bevy::window::RequestRedraw>>()
            .is_empty()
    );
}

#[test]
fn remote_scene_diagnostic_checks_logical_axes_before_requesting_capture() {
    // The checked-in diagnostic owns the authored 120 x 64 expectation. Real
    // observation/bindings/VM run here with synthetic geometry, not native layout.
    // Capture has no artifact authority, so reaching its refusal proves the
    // geometry checks passed without inventing a renderer or writing any files.
    for (size, expected) in [
        (
            Vec2::new(120., 64.),
            "Capture artifact directory is not configured",
        ),
        (Vec2::new(121., 64.), "Scene target width changed"),
        (Vec2::new(120., 65.), "Scene target height changed"),
    ] {
        let (mut app, session, window) = host();
        let camera = app
            .world_mut()
            .spawn((
                Camera::default(),
                bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window)),
            ))
            .id();
        let mut target_camera = ComputedUiTargetCamera::default();
        let mut reflected = bevy::reflect::structs::DynamicStruct::default();
        reflected.insert("camera", Some(camera));
        target_camera.apply(&reflected);
        for id in ["game.panel", "game.target"] {
            app.world_mut().spawn((
                kedra_ui::ElementId::new(id),
                ComputedNode { size, ..default() },
                UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::new(
                    210., 180.,
                ))),
                InheritedVisibility::VISIBLE,
                target_camera,
            ));
        }
        let before = crate::observe::snapshot(app.world_mut()).unwrap()["elements"].clone();
        register(
            &mut app,
            Program::sequence(
                "scene.remote",
                "scripts/Native/remote.luau",
                include_str!("../../../../tests/fixtures/test_project/scripts/Native/remote.luau"),
                install_app_api,
            ),
        );
        let id = Uuid::new_v4();
        rpc(
            &mut app,
            "app.scenario",
            json!({"session":session,"id":id,"scenario":"scene.remote","arguments":{}}),
        )
        .unwrap();
        let result = receipt(&mut app, id);
        assert_eq!(result["state"], "failed");
        assert!(result["detail"].to_string().contains(expected), "{result}");
        assert_eq!(
            crate::observe::snapshot(app.world_mut()).unwrap()["elements"],
            before
        );
        assert_eq!(
            app.world().get::<Window>(window).unwrap().title,
            Window::default().title
        );
        assert!(!app.world().resource::<crate::input::InputQueue>().busy());
        assert!(app.world().resource::<Active>().0.is_none());
    }
}
