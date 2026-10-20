#![allow(
    clippy::unwrap_used,
    reason = "Native form event/schedule regression assertions"
)]
use super::*;
use bevy::{
    asset::AssetPlugin,
    camera::RenderTarget,
    input::{
        ButtonState, InputPlugin,
        keyboard::{Key, KeyboardInput, NativeKeyCode},
    },
    input_focus::{InputDispatchPlugin, InputFocusPlugin, tab_navigation::TabGroup},
    text::{EditableText, TextPlugin},
    ui_widgets::{TextInput, TextInputPlugin},
    window::{Ime, PrimaryWindow, WindowRef},
};

struct Form {
    app: App,
    window: Entity,
    modal: Entity,
    name: Entity,
    submit: Entity,
    _directory: tempfile::TempDir,
}
fn form() -> Form {
    let directory = tempfile::tempdir().unwrap();
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
    ))
    .add_plugins(kedra_ui::native_input::NativeInputPlugin)
    .init_resource::<ProjectManager>()
    .init_resource::<EntryForm>()
    .init_resource::<NextState<AppPhase>>()
    .init_resource::<bevy::picking::events::PointerState>()
    .init_resource::<UiScale>()
    .add_message::<bevy::picking::events::PointerRelease>()
    .add_observer(submit_key)
    .add_observer(activated)
    .add_systems(
        PostUpdate,
        native::apply.after(bevy::text::EditableTextSystems),
    );
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
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
            UiTargetCamera(camera),
            TabGroup::default(),
            InheritedVisibility::VISIBLE,
        ))
        .id();
    let modal = app
        .world_mut()
        .spawn((Node::default(), InheritedVisibility::VISIBLE, ChildOf(root)))
        .id();
    let name = app
        .world_mut()
        .spawn((
            EditableText::new("Valid"),
            TextInput,
            Field::Name,
            InheritedVisibility::VISIBLE,
            ChildOf(modal),
        ))
        .id();
    app.world_mut().spawn((
        EditableText::new(directory.path().to_str().unwrap()),
        TextInput,
        Field::Directory,
        InheritedVisibility::VISIBLE,
        ChildOf(modal),
    ));
    let submit = app.world_mut().spawn((Action::Submit, ChildOf(modal))).id();
    app.world_mut().resource_mut::<ProjectManager>().modal = Some(modal);
    app.world_mut().resource_mut::<EntryForm>().mode = Some(EntryMode::Create);
    app.insert_resource(InputFocus::from_entity(name));
    app.finish();
    app.cleanup();
    app.update();
    Form {
        app,
        window,
        modal,
        name,
        submit,
        _directory: directory,
    }
}
fn key(form: &mut Form, code: KeyCode, logical: Key, text: Option<&str>) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        form.app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state,
            text: if state.is_pressed() {
                text.map(Into::into)
            } else {
                None
            },
            repeat: false,
            window: form.window,
        });
    }
}
fn text(form: &mut Form, value: &str) {
    key(
        form,
        KeyCode::Unidentified(NativeKeyCode::Unidentified),
        Key::Character(value.into()),
        Some(value),
    );
}

#[test]
fn native_final_character_and_enter_capture_after_actual_text_edits() {
    let mut f = form();
    text(&mut f, "x");
    key(&mut f, KeyCode::Enter, Key::Enter, None);
    f.app.update();
    assert_eq!(f.app.world().resource::<EntryForm>().name, "Validx");
    assert!(f.app.world().contains_resource::<jobs::PendingJob>());
    assert_eq!(f.app.world().resource::<ProjectManager>().generation, 0);
}
#[test]
fn final_invalid_edit_cannot_submit_the_previous_valid_name() {
    let mut f = form();
    text(&mut f, "/");
    key(&mut f, KeyCode::Enter, Key::Enter, None);
    f.app.update();
    assert_eq!(f.app.world().resource::<EntryForm>().name, "Valid/");
    assert!(!f.app.world().contains_resource::<jobs::PendingJob>());
    assert!(!f.app.world().resource::<EntryForm>().error.is_empty());
}
#[test]
fn same_frame_native_button_activation_captures_the_applied_value() {
    let mut f = form();
    text(&mut f, "/");
    // Native activation and keyboard messages belong to the same input batch.
    f.app.world_mut().trigger(Activate { entity: f.submit });
    f.app.update();
    assert!(!f.app.world().contains_resource::<jobs::PendingJob>());
    assert_eq!(f.app.world().resource::<EntryForm>().name, "Valid/");
}
#[test]
fn composing_enter_and_escape_are_not_project_commands() {
    for (code, logical) in [(KeyCode::Enter, Key::Enter), (KeyCode::Escape, Key::Escape)] {
        let mut f = form();
        f.app.world_mut().write_message(Ime::Preedit {
            window: f.window,
            value: "candidate".into(),
            cursor: Some((0, 0)),
        });
        f.app.update();
        assert!(
            f.app
                .world()
                .get::<EditableText>(f.name)
                .unwrap()
                .is_composing()
        );
        key(&mut f, code, logical.clone(), None);
        f.app.update();
        assert_eq!(
            f.app.world().resource::<ProjectManager>().modal,
            Some(f.modal)
        );
        assert!(!f.app.world().contains_resource::<jobs::PendingJob>());
        f.app.world_mut().write_message(Ime::Preedit {
            window: f.window,
            value: String::new(),
            cursor: None,
        });
        f.app.update();
        key(&mut f, code, logical, None);
        f.app.update();
        if code == KeyCode::Enter {
            assert!(f.app.world().contains_resource::<jobs::PendingJob>());
        } else {
            assert!(f.app.world().resource::<ProjectManager>().modal.is_none());
        }
    }
}
#[test]
fn same_batch_preedit_and_submit_waits_then_rejects_composition() {
    for (code, logical) in [(KeyCode::Enter, Key::Enter), (KeyCode::Escape, Key::Escape)] {
        let mut f = form();
        f.app.world_mut().write_message(Ime::Preedit {
            window: f.window,
            value: "candidate".into(),
            cursor: Some((0, 0)),
        });
        key(&mut f, code, logical, None);
        f.app.update();
        assert!(!f.app.world().contains_resource::<jobs::PendingJob>());
        assert_eq!(
            f.app.world().resource::<ProjectManager>().modal,
            Some(f.modal)
        );
    }
}

#[test]
fn explicit_cancel_button_can_close_a_composing_form() {
    let mut f = form();
    f.app
        .world_mut()
        .entity_mut(f.submit)
        .insert(Action::Cancel);
    f.app.world_mut().write_message(Ime::Preedit {
        window: f.window,
        value: "candidate".into(),
        cursor: Some((0, 0)),
    });
    f.app.update();
    f.app.world_mut().trigger(Activate { entity: f.submit });
    f.app.update();
    assert!(f.app.world().resource::<ProjectManager>().modal.is_none());
}

#[test]
fn deferred_submit_is_bound_to_modal_and_model_generation() {
    for replace_modal in [false, true] {
        let mut f = form();
        f.app.world_mut().trigger(Activate { entity: f.submit });
        if replace_modal {
            f.app.world_mut().despawn(f.modal);
            let replacement = f.app.world_mut().spawn_empty().id();
            f.app.world_mut().resource_mut::<ProjectManager>().modal = Some(replacement);
        } else {
            f.app
                .world_mut()
                .resource_mut::<ProjectManager>()
                .generation += 1;
        }
        f.app.update();
        assert!(!f.app.world().contains_resource::<jobs::PendingJob>());
    }
}
#[test]
fn semantic_submit_refuses_pending_native_edits_instead_of_using_stale_draft() {
    let mut f = form();
    f.app
        .world_mut()
        .get_mut::<EditableText>(f.name)
        .unwrap()
        .queue_edit(bevy::text::TextEdit::Insert("/".into()));
    dispatch(f.app.world_mut(), Action::Submit, None);
    assert!(!f.app.world().contains_resource::<jobs::PendingJob>());
    assert_eq!(f.app.world().resource::<ProjectManager>().generation, 0);
    assert!(
        f.app
            .world()
            .resource::<EntryForm>()
            .error
            .contains("Wait for text")
    );
    f.app.update();
    dispatch(f.app.world_mut(), Action::Submit, None);
    assert_eq!(f.app.world().resource::<EntryForm>().name, "Valid/");
    assert!(!f.app.world().contains_resource::<jobs::PendingJob>());
}
