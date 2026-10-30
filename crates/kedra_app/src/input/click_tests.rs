//! Explicit button identity reaches native input without borrowing hardware holds.
use super::*;

#[derive(Resource, Default)]
struct Events(Vec<MouseButtonInput>);

#[test]
fn middle_hold_spans_operations_and_wrong_button_release_preserves_it() {
    let (mut app, window, foreign) = test_support::host();
    test_support::command(
        &mut app,
        window,
        json!({"kind":"button","button":"middle","pressed":true}),
    );
    test_support::command(&mut app, window, json!({"kind":"move","position":[40,50]}));
    // The default left-button release cannot consume a middle-button hold.
    test_support::command(&mut app, window, json!({"kind":"button","pressed":false}));
    assert!(
        app.world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Middle)
    );
    assert!(app.world().resource::<HeldInputs>().has_injected_holds());
    let session = app.world().resource::<crate::AppSession>().id.clone();
    assert!(
        request(
            In(Some(json!({"session":session,"id":Uuid::new_v4(),
        "window":foreign.to_bits(),"input":{"kind":"button","button":"right","pressed":true}}))),
            app.world_mut()
        )
        .is_err()
    );
    assert!(!app.world().resource::<InputQueue>().busy());
    assert!(
        app.world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Middle)
    );
    assert!(
        !app.world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Right)
    );
    test_support::command(
        &mut app,
        window,
        json!({"kind":"button","button":"middle","pressed":false}),
    );
    assert!(
        !app.world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Middle)
    );
    assert!(!app.world().resource::<HeldInputs>().has_injected_holds());
}

#[test]
fn secondary_and_middle_clicks_preserve_button_identity_and_release_their_holds() {
    let (mut app, window, _) = test_support::host();
    app.init_resource::<Events>().add_systems(
        Update,
        |mut input: MessageReader<MouseButtonInput>, mut events: ResMut<Events>| {
            events.0.extend(input.read().copied());
        },
    );
    for (name, button) in [
        ("right", MouseButton::Right),
        ("middle", MouseButton::Middle),
        ("left", MouseButton::Left),
    ] {
        app.world_mut().resource_mut::<Events>().0.clear();
        test_support::command(
            &mut app,
            window,
            json!({"kind":"click","position":[10,10],"button":name}),
        );
        let events = &app.world().resource::<Events>().0;
        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .all(|event| event.button == button && event.window == window)
        );
        assert_eq!(events[0].state, ButtonState::Pressed);
        assert_eq!(events[1].state, ButtonState::Released);
        assert!(
            !app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(button)
        );
        assert!(!app.world().resource::<HeldInputs>().has_injected_holds());
    }
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    let mut held = HeldInputs::default();
    assert!(
        held.button(app.world_mut(), window, MouseButton::Right, true)
            .is_err()
    );
    assert!(
        held.button(app.world_mut(), window, MouseButton::Right, false)
            .is_ok()
    );
    assert!(
        app.world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Right)
    );
    assert!(!held.has_injected_holds());
}
