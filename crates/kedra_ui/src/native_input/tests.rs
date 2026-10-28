#![allow(
    clippy::unwrap_used,
    reason = "Native input schedule regression assertions"
)]
use super::*;
mod text_fields;
use bevy::{
    asset::AssetPlugin,
    camera::RenderTarget,
    input::{
        ButtonState, InputPlugin,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{
        FocusedInput, InputDispatchPlugin, InputFocus, InputFocusPlugin,
        tab_navigation::{TabGroup, TabIndex, TabNavigationPlugin},
    },
    text::{EditableText, TextPlugin},
    ui_widgets::{TextInput, TextInputPlugin},
    window::{PrimaryWindow, WindowRef},
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        WindowPlugin {
            primary_window: None,
            exit_condition: bevy::window::ExitCondition::DontExit,
            ..default()
        },
        InputPlugin,
        InputFocusPlugin,
        InputDispatchPlugin,
        TextPlugin,
        TextInputPlugin,
        TabNavigationPlugin,
        bevy::ui_widgets::ModalDialogPlugin,
    ))
    .init_resource::<bevy::picking::events::PointerState>()
    .init_resource::<UiScale>()
    .add_message::<bevy::picking::events::PointerRelease>()
    .add_message::<bevy::window::Ime>()
    .add_plugins(NativeInputPlugin);
    app.finish();
    app.cleanup();
    app
}
fn surface(app: &mut App, primary: bool) -> (Entity, Entity) {
    let window = app.world_mut().spawn(Window::default()).id();
    if primary {
        app.world_mut().entity_mut(window).insert(PrimaryWindow);
    }
    let camera = app
        .world_mut()
        .spawn((
            Camera::default(),
            RenderTarget::Window(WindowRef::Entity(window)),
        ))
        .id();
    let root = app
        .world_mut()
        .spawn((
            Node::default(),
            TabGroup::default(),
            UiTargetCamera(camera),
            InheritedVisibility::VISIBLE,
        ))
        .id();
    (window, root)
}
fn button(app: &mut App, parent: Entity) -> Entity {
    app.world_mut()
        .spawn((
            Node::default(),
            TabIndex(0),
            InheritedVisibility::VISIBLE,
            ChildOf(parent),
        ))
        .id()
}
fn key(app: &mut App, window: Entity, key: KeyCode, logical: Key) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: key,
            logical_key: logical.clone(),
            state,
            text: None,
            repeat: false,
            window,
        });
    }
    app.update();
}
#[test]
fn late_replacement_and_floating_windows_receive_tab_only_in_their_tree() {
    let mut app = app();
    let (initial, root) = surface(&mut app, true);
    let a = button(&mut app, root);
    let b = button(&mut app, root);
    app.update();
    app.insert_resource(InputFocus::from_entity(a));
    key(&mut app, initial, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(b));
    app.world_mut().despawn(initial);
    app.world_mut().despawn(root);
    let (main, root) = surface(&mut app, true);
    let c = button(&mut app, root);
    let d = button(&mut app, root);
    let (floating, float_root) = surface(&mut app, false);
    let e = button(&mut app, float_root);
    let f = button(&mut app, float_root);
    app.insert_resource(InputFocus::from_entity(main));
    key(&mut app, main, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(c));
    app.insert_resource(InputFocus::from_entity(c));
    key(&mut app, main, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(d));
    app.insert_resource(InputFocus::from_entity(e));
    key(&mut app, floating, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(f));
    key(&mut app, floating, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(e));
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::ShiftLeft,
        logical_key: Key::Shift,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: floating,
    });
    key(&mut app, floating, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(f));
}
#[test]
fn hidden_disabled_and_inactive_content_is_not_in_tab_order_and_modal_traps() {
    let mut app = app();
    let (window, root) = surface(&mut app, true);
    let a = button(&mut app, root);
    let stash = app
        .world_mut()
        .spawn((
            Node {
                display: Display::None,
                ..default()
            },
            InheritedVisibility::VISIBLE,
            ChildOf(root),
        ))
        .id();
    let _hidden = button(&mut app, stash);
    let disabled = button(&mut app, root);
    app.world_mut()
        .entity_mut(disabled)
        .insert(bevy::ui::InteractionDisabled);
    let b = button(&mut app, root);
    app.update();
    app.insert_resource(InputFocus::from_entity(a));
    key(&mut app, window, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(b));
    app.world_mut().get_mut::<Node>(stash).unwrap().display = Display::Flex;
    key(&mut app, window, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(a));
    key(&mut app, window, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(_hidden));
    let modal = app
        .world_mut()
        .spawn((
            Node::default(),
            TabGroup {
                modal: true,
                order: 1,
            },
            InheritedVisibility::VISIBLE,
            ChildOf(root),
        ))
        .id();
    let m = button(&mut app, modal);
    let n = button(&mut app, modal);
    app.insert_resource(InputFocus::from_entity(a));
    key(&mut app, window, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(m));
    key(&mut app, window, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(n));
    key(&mut app, window, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(m));
}
#[derive(Resource, Default)]
struct Bubbled(Vec<Entity>);
#[test]
fn keyboard_bubbles_to_owner_and_ime_tracks_transfer_readonly_and_retirement() {
    let mut app = app();
    let (main, root) = surface(&mut app, true);
    let (floating, float_root) = surface(&mut app, false);
    app.init_resource::<Bubbled>();
    for window in [main, floating] {
        app.world_mut().entity_mut(window).observe(
            move |ev: On<FocusedInput<KeyboardInput>>, mut seen: ResMut<Bubbled>| {
                if ev.input.state.is_pressed() && ev.input.key_code == KeyCode::F1 {
                    seen.0.push(window);
                }
            },
        );
    }
    let field = app
        .world_mut()
        .spawn((
            EditableText::new("日本語"),
            TextInput,
            TabIndex(0),
            InheritedVisibility::VISIBLE,
            ChildOf(float_root),
        ))
        .id();
    app.world_mut()
        .entity_mut(field)
        .insert(UiGlobalTransform::from(
            bevy::math::Affine2::from_translation(Vec2::new(120., 80.)),
        ));
    app.world_mut()
        .get_mut::<Window>(main)
        .unwrap()
        .ime_position = Vec2::new(9., 9.);
    app.insert_resource(InputFocus::from_entity(field));
    key(&mut app, floating, KeyCode::F1, Key::F1);
    assert_eq!(app.world().resource::<Bubbled>().0, vec![floating]);
    assert!(app.world().get::<Window>(floating).unwrap().ime_enabled);
    assert!(!app.world().get::<Window>(main).unwrap().ime_enabled);
    assert_ne!(
        app.world().get::<Window>(floating).unwrap().ime_position,
        Vec2::ZERO
    );
    assert_eq!(
        app.world().get::<Window>(main).unwrap().ime_position,
        Vec2::new(9., 9.)
    );
    app.world_mut().write_message(bevy::window::Ime::Commit {
        window: main,
        value: "wrong-window".into(),
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<EditableText>(field)
            .unwrap()
            .value()
            .to_string(),
        "日本語"
    );
    app.world_mut().write_message(bevy::window::Ime::Preedit {
        window: floating,
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
    key(&mut app, floating, KeyCode::Tab, Key::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(field));
    app.world_mut().write_message(bevy::window::Ime::Commit {
        window: floating,
        value: " correct".into(),
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<EditableText>(field)
            .unwrap()
            .value()
            .to_string(),
        "日本語 correct"
    );
    app.world_mut().entity_mut(field).insert(ChildOf(root));
    app.update();
    assert!(app.world().get::<Window>(main).unwrap().ime_enabled);
    assert!(!app.world().get::<Window>(floating).unwrap().ime_enabled);
    app.world_mut()
        .entity_mut(field)
        .insert(bevy::text::TextReadWriteMode::ReadOnly);
    app.update();
    assert!(!app.world().get::<Window>(main).unwrap().ime_enabled);
    app.world_mut().despawn(field);
    key(&mut app, floating, KeyCode::Tab, Key::Tab);
    assert_ne!(app.world().resource::<InputFocus>().get(), Some(field));
}

#[test]
fn another_windows_keyboard_cannot_edit_retained_focus_and_retired_events_are_drained() {
    let mut app = app();
    let (main, _) = surface(&mut app, true);
    let (floating, root) = surface(&mut app, false);
    let field = app
        .world_mut()
        .spawn((
            EditableText::new("Kept"),
            TextInput,
            InheritedVisibility::VISIBLE,
            ChildOf(root),
        ))
        .id();
    app.insert_resource(InputFocus::from_entity(field));
    app.update();
    app.world_mut()
        .write_message(bevy::input::mouse::MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Pixel,
            x: 0.,
            y: 20.,
            window: main,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(field));
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyX,
        logical_key: Key::Character("x".into()),
        state: ButtonState::Released,
        text: None,
        repeat: false,
        window: main,
    });
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(field));
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyX,
        logical_key: Key::Character("x".into()),
        state: ButtonState::Pressed,
        text: Some("x".into()),
        repeat: false,
        window: main,
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<EditableText>(field)
            .unwrap()
            .value()
            .to_string(),
        "Kept"
    );
    app.world_mut().despawn(floating);
    key(&mut app, floating, KeyCode::Tab, Key::Tab);
    assert!(!app.world().get::<Window>(main).unwrap().ime_enabled);
}

#[path = "regressions.rs"]
mod regressions;
