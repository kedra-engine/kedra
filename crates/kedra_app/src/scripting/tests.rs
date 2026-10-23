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
    register(
        &mut app,
        Program::sequence(
            "identity",
            "entry.luau",
            "function run(ctx) local id = ctx.app.window(); assert(type(id) == 'string'); return {window = id} end",
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
}
