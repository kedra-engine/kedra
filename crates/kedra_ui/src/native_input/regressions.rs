//! Window/modal regressions use real Bevy messages and widget observers, not OS input.
use super::*;

#[test]
fn native_ui_01_modal_handoff_rejects_background_ime_and_clears_composition() {
    let mut app = app();
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(1),
    ));
    let (window, root) = surface(&mut app, true);
    let (other_window, _) = surface(&mut app, false);
    let field = app
        .world_mut()
        .spawn((
            TextInput,
            EditableText::new("日本語"),
            TabIndex(0),
            InheritedVisibility::VISIBLE,
            ChildOf(root),
        ))
        .id();
    app.update();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(field, bevy::input_focus::FocusCause::Pressed);
    app.world_mut().write_message(bevy::window::Ime::Preedit {
        window,
        value: "candidate".into(),
        cursor: Some((0, 0)),
    });
    app.update();
    assert!(
        app.world()
            .get::<EditableText>(field)
            .unwrap()
            .is_composing()
    );
    let modal = app
        .world_mut()
        .spawn((
            Node::default(),
            bevy::ui_widgets::ModalDialog,
            TabGroup::modal(),
            InheritedVisibility::VISIBLE,
            ChildOf(root),
        ))
        .id();
    // There is deliberately no autofocus. Test both the empty-modal case and
    // the native delayed handoff before its focusable child takes focus.
    for focusable in [false, true] {
        if focusable {
            button(&mut app, modal);
        }
        app.world_mut().write_message(bevy::window::Ime::Commit {
            window,
            value: "LEAK".into(),
        });
        app.world_mut().write_message(bevy::window::Ime::Preedit {
            window,
            value: "hidden".into(),
            cursor: Some((0, 0)),
        });
        app.update();
        let text = app.world().get::<EditableText>(field).unwrap();
        assert_eq!(text.value().to_string(), "日本語");
        assert!(!text.is_composing());
        assert!(!app.world().get::<Window>(window).unwrap().ime_enabled);
        assert!(!app.world().get::<Window>(other_window).unwrap().ime_enabled);
    }
    // Removing the modal must not replay drained commits; normal editing resumes.
    app.world_mut().despawn(modal);
    app.update();
    assert!(app.world().get::<Window>(window).unwrap().ime_enabled);
    app.world_mut().write_message(bevy::window::Ime::Commit {
        window: other_window,
        value: "wrong".into(),
    });
    app.world_mut().write_message(bevy::window::Ime::Commit {
        window,
        value: " valid".into(),
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<EditableText>(field)
            .unwrap()
            .value()
            .to_string(),
        "日本語 valid"
    );
}

#[test]
fn native_ui_02_wrapped_groups_keep_bevy_order_and_window_modal_scope() {
    let mut app = app();
    let (window, root) = surface(&mut app, true);
    let (other_window, other_root) = surface(&mut app, false);
    let foreign = button(&mut app, other_root);
    let first = button(&mut app, root);
    let wrapper = app
        .world_mut()
        .spawn((Node::default(), InheritedVisibility::VISIBLE, ChildOf(root)))
        .id();
    let nested = app
        .world_mut()
        .spawn((
            Node::default(),
            TabGroup::new(1),
            InheritedVisibility::VISIBLE,
            ChildOf(wrapper),
        ))
        .id();
    let second = button(&mut app, nested);
    let third = button(&mut app, nested);
    app.world_mut().entity_mut(second).insert(TabIndex(2));
    app.world_mut().entity_mut(third).insert(TabIndex(1));
    app.update();
    app.insert_resource(InputFocus::from_entity(first));
    for expected in [third, second, first] {
        key(&mut app, window, KeyCode::Tab, Key::Tab);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(expected));
        assert_eq!(app.world().get::<TabIndex>(foreign), Some(&TabIndex(0)));
    }
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::ShiftLeft,
        logical_key: Key::Shift,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    key(&mut app, window, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(second));
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::ShiftLeft,
        logical_key: Key::Shift,
        state: ButtonState::Released,
        text: None,
        repeat: false,
        window,
    });
    app.world_mut().get_mut::<Node>(wrapper).unwrap().display = Display::None;
    key(&mut app, window, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(first));
    app.world_mut().get_mut::<Node>(wrapper).unwrap().display = Display::Flex;
    let modal = app
        .world_mut()
        .spawn((
            Node::default(),
            TabGroup::modal(),
            InheritedVisibility::VISIBLE,
            ChildOf(wrapper),
        ))
        .id();
    let inside = button(&mut app, modal);
    for _ in 0..2 {
        key(&mut app, window, KeyCode::Tab, Key::Tab);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(inside));
    }
    key(&mut app, other_window, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(foreign));
}
