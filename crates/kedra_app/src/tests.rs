//! Independent non-editor host contracts using the public BRP mailbox.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated application contract assertions"
)]
use super::*;
use bevy_remote::{BrpMessage, BrpSender};

fn host() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        KedraAppPlugin,
    ));
    app.update();
    app
}
fn call(app: &mut App, method: &str, params: Option<Value>) -> BrpResult {
    let (sender, receiver) = async_channel::bounded(1);
    app.world()
        .resource::<BrpSender>()
        .try_send(BrpMessage {
            method: method.into(),
            params,
            sender,
        })
        .unwrap();
    app.update();
    receiver.try_recv().unwrap()
}
#[test]
fn game_host_exposes_only_registered_methods_and_rejects_stale_input() {
    let mut app = host();
    let description = call(&mut app, "app.describe", None).unwrap();
    assert!(description["methods"].get("editor.project").is_none());
    assert!(call(&mut app, "world.spawn_entity", Some(json!({}))).is_err());
    let window = app.world_mut().spawn(Window::default()).id();
    let id = uuid::Uuid::new_v4();
    let error=call(&mut app,"app.input",Some(json!({"session":"stale","id":id,"window":window.to_bits(),"input":{"kind":"click","position":[1,1]}}))).unwrap_err();
    assert!(error.message.contains("Stale"));
    assert!(
        app.world()
            .resource::<operations::Operations>()
            .get(id)
            .is_none()
    );
    app.world_mut().despawn(window);
    assert!(
        input::target(
            app.world(),
            description["session"].as_str().unwrap(),
            window.to_bits()
        )
        .is_err()
    );
}
#[test]
fn admitted_input_retirement_has_terminal_receipt_and_safe_retry() {
    let mut app = host();
    let session = app.world().resource::<AppSession>().id.clone();
    let window = app.world_mut().spawn(Window::default()).id();
    let id = uuid::Uuid::new_v4();
    let request =
        json!({"session":session,"id":id,"window":window.to_bits(),"width":800.,"height":600.});
    assert_eq!(
        call(&mut app, "app.resize", Some(request.clone())).unwrap()["state"],
        "pending"
    );
    app.world_mut().despawn(window);
    let retried = call(&mut app, "app.resize", Some(request.clone())).unwrap();
    assert_eq!(retried["state"], "failed");
    assert!(!app.world().resource::<input::InputQueue>().busy());
    let mut conflicting = request;
    conflicting["width"] = json!(900.);
    assert!(call(&mut app, "app.resize", Some(conflicting)).is_err());
}
