//! Minimal real input host and frame-ordered commands for input contracts.
#![allow(clippy::unwrap_used, reason = "Headless input fixture assertions")]
use super::*;
use crate::operations::{OperationState, Operations};

pub(super) fn host() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::input::InputPlugin,
        crate::KedraAppPlugin,
    ))
    .add_message::<WindowEvent>()
    .add_message::<CursorMoved>();
    app.update();
    let a = app.world_mut().spawn(Window::default()).id();
    let b = app.world_mut().spawn(Window::default()).id();
    (app, a, b)
}

pub(super) fn command(app: &mut App, window: Entity, input: Value) -> (Uuid, Value) {
    let id = Uuid::new_v4();
    let session = app.world().resource::<crate::AppSession>().id.clone();
    let value = json!({"session":session,"id":id,"window":window.to_bits(),"input":input});
    let (sender, receiver) = async_channel::bounded(1);
    app.world()
        .resource::<bevy_remote::BrpSender>()
        .try_send(bevy_remote::BrpMessage {
            method: "app.input".into(),
            params: Some(value.clone()),
            sender,
        })
        .unwrap();
    app.update();
    assert_eq!(receiver.try_recv().unwrap().unwrap()["state"], "pending");
    for _ in 0..16 {
        app.update();
        if !app.world().resource::<InputQueue>().busy() {
            break;
        }
    }
    assert_eq!(
        app.world().resource::<Operations>().get(id).unwrap().state,
        OperationState::Succeeded
    );
    (id, value)
}

pub(super) fn held(world: &World) -> (bool, bool, bool) {
    (
        world
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left),
        world
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::ShiftLeft),
        world.resource::<ButtonInput<Key>>().pressed(Key::Shift),
    )
}
