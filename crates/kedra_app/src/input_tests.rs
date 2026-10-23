#![allow(clippy::unwrap_used, reason = "Headless input contract assertions")]
use super::*;
use crate::operations::{OperationState, Operations};

#[derive(Resource, Default)]
struct RecordedKeyboard(Vec<KeyboardInput>);

fn host() -> (App, Entity, Entity) {
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
fn command(app: &mut App, window: Entity, input: Value) -> (Uuid, Value) {
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
fn held(world: &World) -> (bool, bool, bool) {
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

fn unowned_release_is_a_receipted_noop(input: Value, external: bool) {
    let (mut app, target, hardware_window) = host();
    let modifiers = [
        (KeyCode::ShiftLeft, Key::Shift),
        (KeyCode::ControlLeft, Key::Control),
        (KeyCode::AltLeft, Key::Alt),
        (KeyCode::SuperLeft, Key::Super),
    ];
    if external {
        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: hardware_window,
        });
        for (key_code, logical_key) in &modifiers {
            app.world_mut().write_message(KeyboardInput {
                key_code: *key_code,
                logical_key: logical_key.clone(),
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: hardware_window,
            });
        }
        app.update();
    }
    let mut mouse = app
        .world()
        .resource::<Messages<MouseButtonInput>>()
        .get_cursor_current();
    let mut keys = app
        .world()
        .resource::<Messages<KeyboardInput>>()
        .get_cursor_current();
    let mut windows = app
        .world()
        .resource::<Messages<WindowEvent>>()
        .get_cursor_current();
    // Repeat with fresh IDs as cleanup does, as well as retrying a terminal ID.
    for _ in 0..2 {
        let id = Uuid::new_v4();
        let session = app.world().resource::<crate::AppSession>().id.clone();
        let value = json!({"session":session,"id":id,"window":target.to_bits(),"input":input});
        let (sender, receiver) = async_channel::bounded(1);
        app.world()
            .resource::<bevy_remote::BrpSender>()
            .try_send(bevy_remote::BrpMessage {
                method: "app.input".into(),
                params: Some(value.clone()),
                sender,
            })
            .unwrap();
        for _ in 0..3 {
            app.update();
            // Inspect every frame, so an erroneous release cannot age out of the
            // retained message window before this regression observes it.
            assert_eq!(
                mouse
                    .read(app.world().resource::<Messages<MouseButtonInput>>())
                    .count(),
                0
            );
            assert_eq!(
                keys.read(app.world().resource::<Messages<KeyboardInput>>())
                    .count(),
                0
            );
            assert_eq!(
                windows
                    .read(app.world().resource::<Messages<WindowEvent>>())
                    .count(),
                0
            );
        }
        assert_eq!(receiver.try_recv().unwrap().unwrap()["state"], "pending");
        assert_eq!(
            app.world().resource::<Operations>().get(id).unwrap().state,
            OperationState::Succeeded
        );
        assert_eq!(
            request(In(Some(value)), app.world_mut()).unwrap()["state"],
            "succeeded"
        );
        assert_eq!(
            app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left),
            external
        );
        for (code, key) in &modifiers {
            assert_eq!(
                app.world()
                    .resource::<ButtonInput<KeyCode>>()
                    .pressed(*code),
                external
            );
            assert_eq!(
                app.world()
                    .resource::<ButtonInput<Key>>()
                    .pressed(key.clone()),
                external
            );
        }
    }
    // The native fixture's unconditional button-up cleanup is followed by F12.
    // A no-op cleanup must not prevent the next real input transaction.
    command(&mut app, target, json!({"kind":"key","key":"f12"}));
    assert_eq!(
        app.world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left),
        external
    );
}

#[test]
fn cleanup_button_release_without_ownership_succeeds_without_input_events() {
    for external in [false, true] {
        unowned_release_is_a_receipted_noop(json!({"kind":"button","pressed":false}), external);
    }
}

#[test]
fn cleanup_modifier_releases_without_ownership_succeed_without_input_events() {
    for external in [false, true] {
        for modifier in ["shift", "control", "alt", "super"] {
            unowned_release_is_a_receipted_noop(
                json!({"kind":"modifier","modifier":modifier,"pressed":false}),
                external,
            );
        }
    }
}

#[test]
fn cross_host_drag_moves_and_releases_in_the_destination_window() {
    let (mut app, floating, primary) = host();
    app.world_mut()
        .entity_mut(primary)
        .insert(bevy::window::PrimaryWindow);
    command(
        &mut app,
        floating,
        json!({"kind":"move","position":[100,100]}),
    );
    let (press, _) = command(&mut app, floating, json!({"kind":"button","pressed":true}));
    assert!(held(app.world()).0);
    for input in [
        json!({"kind":"move","position":[415,525]}),
        json!({"kind":"wheel","delta":[0,20]}),
        json!({"kind":"button","pressed":false}),
    ] {
        let mut cursor = app
            .world()
            .resource::<Messages<CursorMoved>>()
            .get_cursor_current();
        let mut button = app
            .world()
            .resource::<Messages<MouseButtonInput>>()
            .get_cursor_current();
        let mut wheel = app
            .world()
            .resource::<Messages<MouseWheel>>()
            .get_cursor_current();
        let mut windows = app
            .world()
            .resource::<Messages<WindowEvent>>()
            .get_cursor_current();
        let id = Uuid::new_v4();
        let session = app.world().resource::<crate::AppSession>().id.clone();
        let (sender, receiver) = async_channel::bounded(1);
        app.world()
            .resource::<bevy_remote::BrpSender>()
            .try_send(bevy_remote::BrpMessage {
                method: "app.input".into(),
                params: Some(
                    json!({"session":session,"id":id,"window":primary.to_bits(),"input":input}),
                ),
                sender,
            })
            .unwrap();
        app.update();
        assert_eq!(receiver.try_recv().unwrap().unwrap()["state"], "pending");
        app.update();
        let window_events: Vec<_> = windows
            .read(app.world().resource::<Messages<WindowEvent>>())
            .collect();
        match input["kind"].as_str().unwrap() {
            "move" => {
                let event = cursor
                    .read(app.world().resource::<Messages<CursorMoved>>())
                    .next()
                    .unwrap();
                assert_eq!(
                    (event.window, event.position),
                    (primary, Vec2::new(415., 525.))
                );
                assert!(window_events.iter().any(|e| matches!(e, WindowEvent::CursorMoved(e) if e.window == primary && e.position == Vec2::new(415.,525.))));
                assert_eq!(
                    app.world()
                        .get::<Window>(floating)
                        .unwrap()
                        .cursor_position(),
                    Some(Vec2::new(100., 100.))
                );
                assert!(held(app.world()).0);
            }
            "wheel" => {
                assert_eq!(
                    wheel
                        .read(app.world().resource::<Messages<MouseWheel>>())
                        .next()
                        .unwrap()
                        .window,
                    primary
                );
                assert!(
                    window_events
                        .iter()
                        .any(|e| matches!(e, WindowEvent::MouseWheel(e) if e.window == primary))
                );
                assert!(held(app.world()).0);
            }
            "button" => {
                let event = button
                    .read(app.world().resource::<Messages<MouseButtonInput>>())
                    .next()
                    .unwrap();
                assert_eq!(event.window, primary);
                assert_eq!(event.state, ButtonState::Released);
                assert!(window_events.iter().any(|e| matches!(e, WindowEvent::MouseButtonInput(e) if e.window == primary && e.state == ButtonState::Released)));
                assert!(!held(app.world()).0);
            }
            _ => unreachable!(),
        }
        app.update();
        assert_eq!(
            app.world().resource::<Operations>().get(id).unwrap().state,
            OperationState::Succeeded
        );
    }
    assert_eq!(
        app.world()
            .resource::<Operations>()
            .get(press)
            .unwrap()
            .state,
        OperationState::Succeeded
    );
    // Source retirement after destination release must not generate another up.
    let mut mouse = app
        .world()
        .resource::<Messages<MouseButtonInput>>()
        .get_cursor_current();
    app.world_mut().despawn(floating);
    app.update();
    assert_eq!(
        mouse
            .read(app.world().resource::<Messages<MouseButtonInput>>())
            .count(),
        0
    );
    assert!(!held(app.world()).0);
}

#[test]
fn cross_host_pointer_release_preserves_hardware_and_rejects_overlapping_holds() {
    let (mut app, source, destination) = host();
    command(&mut app, source, json!({"kind":"button","pressed":true}));
    let session = app.world().resource::<crate::AppSession>().id.clone();
    for input in [
        json!({"kind":"button","pressed":true}),
        json!({"kind":"modifier","modifier":"shift","pressed":true}),
        json!({"kind":"key","key":"escape"}),
        json!({"kind":"text","text":"x"}),
        json!({"kind":"click","position":[1,1]}),
        json!({"kind":"drag","from":[1,1],"to":[2,2]}),
    ] {
        let id = Uuid::new_v4();
        let error = request(
            In(Some(
                json!({"session":session,"id":id,"window":destination.to_bits(),"input":input}),
            )),
            app.world_mut(),
        )
        .unwrap_err();
        assert!(error.message.contains("another window"));
        assert!(app.world().resource::<Operations>().get(id).is_none());
        assert!(held(app.world()).0);
    }
    // Releasing an unowned modifier does not affect the pointer's source lease.
    command(
        &mut app,
        destination,
        json!({"kind":"modifier","modifier":"shift","pressed":false}),
    );
    assert!(held(app.world()).0);
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Pressed,
        window: destination,
    });
    app.update();
    command(
        &mut app,
        destination,
        json!({"kind":"button","pressed":false}),
    );
    assert!(held(app.world()).0); // Hardware remains down; the adapter lease is gone.
    app.world_mut().despawn(source);
    app.update();
    assert!(held(app.world()).0);
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Released,
        window: destination,
    });
    app.update();
    assert!(!held(app.world()).0);
}

#[test]
fn cross_host_movement_does_not_transfer_pointer_retirement_ownership() {
    let (mut app, source, destination) = host();
    let (press, _) = command(&mut app, source, json!({"kind":"button","pressed":true}));
    command(
        &mut app,
        destination,
        json!({"kind":"move","position":[415,525]}),
    );
    let mut events = app
        .world()
        .resource::<Messages<MouseButtonInput>>()
        .get_cursor_current();
    app.world_mut().despawn(source);
    app.update();
    assert!(!held(app.world()).0);
    let release = events
        .read(app.world().resource::<Messages<MouseButtonInput>>())
        .next()
        .unwrap();
    assert_eq!(
        (release.window, release.state),
        (source, ButtonState::Released)
    );
    assert_eq!(
        app.world()
            .resource::<Operations>()
            .get(press)
            .unwrap()
            .state,
        OperationState::Succeeded
    );
    assert!(app.world().get::<Window>(destination).is_some());
}

#[test]
fn completed_holds_retire_independently_of_receipts_and_another_queued_operation() {
    let (mut app, a, b) = host();
    let (mouse, retry_mouse) = command(&mut app, a, json!({"kind":"button","pressed":true}));
    let (shift, _) = command(
        &mut app,
        a,
        json!({"kind":"modifier","modifier":"shift","pressed":true}),
    );
    assert_eq!(held(app.world()), (true, true, true));
    let session = app.world().resource::<crate::AppSession>().id.clone();
    let resize_id = Uuid::new_v4();
    resize(
        In(Some(
            json!({"session":session,"id":resize_id,"window":b.to_bits(),"width":800,"height":600}),
        )),
        app.world_mut(),
    )
    .unwrap();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    app.world_mut()
        .resource_mut::<ButtonInput<Key>>()
        .press(Key::Character("b".into()));
    app.world_mut().despawn(a);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(held(app.world()), (false, false, false));
    assert!(
        app.world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Right)
    );
    assert!(
        app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::KeyB)
    );
    assert!(
        app.world()
            .resource::<ButtonInput<Key>>()
            .pressed(Key::Character("b".into()))
    );
    for id in [mouse, shift, resize_id] {
        assert_eq!(
            app.world().resource::<Operations>().get(id).unwrap().state,
            OperationState::Succeeded
        );
    }
    assert_eq!(
        request(In(Some(retry_mouse)), app.world_mut()).unwrap()["state"],
        "succeeded"
    );
}

#[test]
fn normal_release_and_chords_preserve_explicit_modifiers_and_reject_second_window_holds() {
    let (mut app, a, b) = host();
    command(
        &mut app,
        a,
        json!({"kind":"modifier","modifier":"shift","pressed":true}),
    );
    command(&mut app, a, json!({"kind":"key","key":"shift_tab"}));
    assert_eq!(held(app.world()), (false, true, true));
    assert!(
        !app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::Tab)
    );
    let id = Uuid::new_v4();
    let session = app.world().resource::<crate::AppSession>().id.clone();
    let error = request(In(Some(json!({"session":session,"id":id,"window":b.to_bits(),"input":{"kind":"button","pressed":true}}))), app.world_mut()).unwrap_err();
    assert!(error.message.contains("another window"));
    assert!(app.world().resource::<Operations>().get(id).is_none());
    command(
        &mut app,
        a,
        json!({"kind":"modifier","modifier":"shift","pressed":false}),
    );
    command(&mut app, b, json!({"kind":"button","pressed":true}));
    command(&mut app, b, json!({"kind":"button","pressed":false}));
    assert_eq!(held(app.world()), (false, false, false));
    app.world_mut().despawn(a);
    app.update();
    assert_eq!(held(app.world()), (false, false, false));
}

#[test]
fn retirement_preserves_hardware_overlapping_logical_and_physical_identities() {
    for (code, logical) in [
        (KeyCode::ShiftLeft, Key::Shift),
        (KeyCode::ShiftRight, Key::Shift),
        (KeyCode::ShiftLeft, Key::Character("external".into())),
    ] {
        let (mut app, a, b) = host();
        command(&mut app, a, json!({"kind":"button","pressed":true}));
        command(
            &mut app,
            a,
            json!({"kind":"modifier","modifier":"shift","pressed":true}),
        );
        // Hardware/live-host events arrive through the real Bevy message surfaces.
        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: b,
        });
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: b,
        });
        app.update();
        let mut events = app
            .world()
            .resource::<Messages<WindowEvent>>()
            .get_cursor_current();
        app.world_mut().despawn(a);
        app.update();
        assert!(
            app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );
        assert!(app.world().resource::<ButtonInput<KeyCode>>().pressed(code));
        assert!(
            app.world()
                .resource::<ButtonInput<Key>>()
                .pressed(logical.clone())
        );
        if logical != Key::Shift {
            assert!(
                !app.world()
                    .resource::<ButtonInput<Key>>()
                    .pressed(Key::Shift)
            );
        }
        if code == KeyCode::ShiftRight {
            assert!(
                !app.world()
                    .resource::<ButtonInput<KeyCode>>()
                    .pressed(KeyCode::ShiftLeft)
            );
        }
        assert!(!events.read(app.world().resource::<Messages<WindowEvent>>()).any(|event| matches!(event, WindowEvent::MouseButtonInput(e) if e.state == ButtonState::Released) || matches!(event, WindowEvent::KeyboardInput(e) if e.state == ButtonState::Released)));
        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Released,
            window: b,
        });
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical,
            state: ButtonState::Released,
            text: None,
            repeat: false,
            window: b,
        });
        app.update();
        assert_eq!(held(app.world()), (false, false, false));
    }
}

#[test]
fn active_retirement_does_not_clear_unrelated_hardware_or_terminal_receipts() {
    let (mut app, a, _) = host();
    let id = Uuid::new_v4();
    let session = app.world().resource::<crate::AppSession>().id.clone();
    request(In(Some(json!({"session":session,"id":id,"window":a.to_bits(),"input":{"kind":"drag","from":[1,1],"to":[20,20]}}))), app.world_mut()).unwrap();
    app.update();
    app.update();
    assert!(held(app.world()).0);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    app.world_mut().despawn(a);
    app.update();
    assert!(!held(app.world()).0);
    assert!(
        app.world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Right)
    );
    assert!(
        app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::KeyB)
    );
    assert_eq!(
        app.world().resource::<Operations>().get(id).unwrap().state,
        OperationState::Failed
    );
    app.update();
    assert_eq!(
        app.world().resource::<Operations>().get(id).unwrap().state,
        OperationState::Failed
    );
}

#[test]
fn input_validation_rejects_invalid_effects_without_admitting_receipts() {
    let (mut app, window, _) = host();
    for input in [
        json!({"kind":"click"}),
        json!({"kind":"click","element":"x","position":[1,1]}),
        json!({"kind":"move","position":[-1,0]}),
        json!({"kind":"drag","from":[0,0],"to":[99999,0]}),
        json!({"kind":"wheel","delta":[10001,0]}),
        json!({"kind":"text","text":"x".repeat(16385)}),
        json!({"kind":"key","key":"unsupported"}),
        json!({"kind":"move","position":[0,0],"unknown":true}),
    ] {
        let id = Uuid::new_v4();
        let session = app.world().resource::<crate::AppSession>().id.clone();
        assert!(
            request(
                In(Some(
                    json!({"session":session,"id":id,"window":window.to_bits(),"input":input})
                )),
                app.world_mut()
            )
            .is_err()
        );
        assert!(app.world().resource::<Operations>().get(id).is_none());
    }
    for (width, height, dpr) in [(319., 600., 1.), (800., 3841., 1.), (800., 600., 0.5)] {
        let id = Uuid::new_v4();
        let session = app.world().resource::<crate::AppSession>().id.clone();
        assert!(resize(In(Some(json!({"session":session,"id":id,"window":window.to_bits(),"width":width,"height":height,"dpr":dpr}))), app.world_mut()).is_err());
        assert!(app.world().resource::<Operations>().get(id).is_none());
    }
}

#[test]
fn cleanup_noops_still_validate_sessions_targets_and_operation_ids() {
    let (mut app, window, _) = host();
    let session = app.world().resource::<crate::AppSession>().id.clone();
    for input in [
        json!({"kind":"button","pressed":false}),
        json!({"kind":"modifier","modifier":"shift","pressed":false}),
    ] {
        for value in [
            json!({"session":"stale","id":Uuid::new_v4(),"window":window.to_bits(),"input":input}),
            json!({"session":session,"id":Uuid::new_v4(),"window":Entity::PLACEHOLDER.to_bits(),"input":input}),
            json!({"session":session,"id":Uuid::nil(),"window":window.to_bits(),"input":input}),
        ] {
            let id: Uuid = serde_json::from_value(value["id"].clone()).unwrap();
            assert!(request(In(Some(value)), app.world_mut()).is_err());
            assert!(app.world().resource::<Operations>().get(id).is_none());
        }
    }
}

#[test]
fn real_input_pipeline_delivers_moves_drag_text_chords_and_idempotent_receipts() {
    let (mut app, a, _) = host();
    // Exercise normal message expiry deterministically, regardless of CPU load.
    // A reader must observe delivery each frame, not after the receipt: the
    // press can age out while release and completion traverse later frames.
    app.insert_resource(bevy::time::TimeUpdateStrategy::FixedTimesteps(1));
    app.init_resource::<RecordedKeyboard>().add_systems(
        Update,
        |mut events: MessageReader<KeyboardInput>, mut recorded: ResMut<RecordedKeyboard>| {
            recorded.0.extend(events.read().cloned());
        },
    );
    command(&mut app, a, json!({"kind":"move","position":[4,5]}));
    assert_eq!(
        app.world().get::<Window>(a).unwrap().cursor_position(),
        Some(Vec2::new(4., 5.))
    );
    command(
        &mut app,
        a,
        json!({"kind":"drag","from":[4,5],"to":[40,50]}),
    );
    assert_eq!(
        app.world().get::<Window>(a).unwrap().cursor_position(),
        Some(Vec2::new(40., 50.))
    );
    assert!(!held(app.world()).0);
    let (id, value) = command(&mut app, a, json!({"kind":"text","text":"Кедра 日本語"}));
    let events = &app.world().resource::<RecordedKeyboard>().0;
    assert_eq!(events.len(), 2);
    for event in events {
        assert_eq!(event.window, a);
        assert_eq!(event.logical_key, Key::Character("Кедра 日本語".into()));
        assert_eq!(
            event.key_code,
            KeyCode::Unidentified(NativeKeyCode::Unidentified)
        );
        assert!(!event.repeat);
    }
    assert_eq!(events[0].state, ButtonState::Pressed);
    assert_eq!(events[0].text.as_deref(), Some("Кедра 日本語"));
    assert_eq!(events[1].state, ButtonState::Released);
    assert_eq!(events[1].text, None);
    assert_eq!(
        request(In(Some(value.clone())), app.world_mut()).unwrap()["state"],
        "succeeded"
    );
    let mut conflict = value;
    conflict["input"]["text"] = json!("different");
    assert!(request(In(Some(conflict)), app.world_mut()).is_err());
    assert_eq!(
        app.world().resource::<Operations>().get(id).unwrap().state,
        OperationState::Succeeded
    );
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<RecordedKeyboard>().0.len(), 2);
    for chord in [
        "enter",
        "escape",
        "tab",
        "shift_tab",
        "space",
        "backspace",
        "delete",
        "left",
        "right",
        "up",
        "down",
        "home",
        "end",
        "f1",
        "f2",
        "f3",
        "f4",
        "f5",
        "f6",
        "f7",
        "f8",
        "f12",
        "select_all",
        "copy",
        "paste",
        "cut",
    ] {
        command(&mut app, a, json!({"kind":"key","key":chord}));
        assert_eq!(
            app.world()
                .resource::<ButtonInput<KeyCode>>()
                .get_pressed()
                .count(),
            0
        );
        assert_eq!(
            app.world()
                .resource::<ButtonInput<Key>>()
                .get_pressed()
                .count(),
            0
        );
    }
    for modifier in ["shift", "control", "alt", "super"] {
        command(
            &mut app,
            a,
            json!({"kind":"modifier","modifier":modifier,"pressed":true}),
        );
        command(
            &mut app,
            a,
            json!({"kind":"modifier","modifier":modifier,"pressed":false}),
        );
    }
}

#[test]
fn adapter_never_borrows_hardware_holds_and_busy_admission_has_no_effects() {
    let (mut app, a, _) = host();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    let id = Uuid::new_v4();
    let session = app.world().resource::<crate::AppSession>().id.clone();
    let value = json!({"session":session,"id":id,"window":a.to_bits(),"input":{"kind":"button","pressed":true}});
    request(In(Some(value.clone())), app.world_mut()).unwrap();
    let busy_id = Uuid::new_v4();
    let mut busy = value;
    busy["id"] = json!(busy_id);
    assert!(
        request(In(Some(busy)), app.world_mut())
            .unwrap_err()
            .message
            .contains("busy")
    );
    app.update();
    assert_eq!(
        app.world().resource::<Operations>().get(id).unwrap().state,
        OperationState::Failed
    );
    assert!(app.world().resource::<Operations>().get(busy_id).is_none());
    app.world_mut().despawn(a);
    app.update();
    assert!(held(app.world()).0);
}

#[test]
fn normal_release_does_not_release_overlapping_live_host_hardware() {
    let (mut app, a, b) = host();
    command(&mut app, a, json!({"kind":"button","pressed":true}));
    command(
        &mut app,
        a,
        json!({"kind":"modifier","modifier":"shift","pressed":true}),
    );
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Pressed,
        window: b,
    });
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::ShiftRight,
        logical_key: Key::Shift,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: b,
    });
    app.update();
    command(&mut app, a, json!({"kind":"button","pressed":false}));
    command(
        &mut app,
        a,
        json!({"kind":"modifier","modifier":"shift","pressed":false}),
    );
    assert_eq!(held(app.world()), (true, false, true));
    assert!(
        app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::ShiftRight)
    );
    app.world_mut().despawn(a);
    app.update();
    assert_eq!(held(app.world()), (true, false, true));
}

#[test]
fn same_batch_hardware_collision_fails_atomically_without_borrowing_keys() {
    let (mut app, a, b) = host();
    let id = Uuid::new_v4();
    let session = app.world().resource::<crate::AppSession>().id.clone();
    request(In(Some(json!({"session":session,"id":id,"window":a.to_bits(),"input":{"kind":"key","key":"shift_tab"}}))), app.world_mut()).unwrap();
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Tab,
        logical_key: Key::Tab,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: b,
    });
    app.update();
    assert_eq!(
        app.world().resource::<Operations>().get(id).unwrap().state,
        OperationState::Failed
    );
    assert!(
        !app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::ShiftLeft)
    );
    assert!(
        app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::Tab)
    );
    app.world_mut().despawn(a);
    app.update();
    assert!(
        app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::Tab)
    );
}

#[test]
fn completed_holds_retire_with_an_empty_queue_and_release_both_message_surfaces() {
    let (mut app, a, _) = host();
    command(&mut app, a, json!({"kind":"button","pressed":true}));
    command(
        &mut app,
        a,
        json!({"kind":"modifier","modifier":"shift","pressed":true}),
    );
    let mut mouse = app
        .world()
        .resource::<Messages<MouseButtonInput>>()
        .get_cursor_current();
    let mut keys = app
        .world()
        .resource::<Messages<KeyboardInput>>()
        .get_cursor_current();
    let mut windows = app
        .world()
        .resource::<Messages<WindowEvent>>()
        .get_cursor_current();
    app.world_mut().despawn(a);
    app.update();
    assert_eq!(held(app.world()), (false, false, false));
    assert_eq!(
        mouse
            .read(app.world().resource::<Messages<MouseButtonInput>>())
            .filter(|e| e.window == a && e.state == ButtonState::Released)
            .count(),
        1
    );
    assert_eq!(
        keys.read(app.world().resource::<Messages<KeyboardInput>>())
            .filter(|e| e.window == a && e.state == ButtonState::Released)
            .count(),
        1
    );
    assert_eq!(windows.read(app.world().resource::<Messages<WindowEvent>>()).filter(|e| matches!(e, WindowEvent::MouseButtonInput(e) if e.window == a && e.state == ButtonState::Released) || matches!(e, WindowEvent::KeyboardInput(e) if e.window == a && e.state == ButtonState::Released)).count(), 2);
}
