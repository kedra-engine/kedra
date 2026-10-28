//! Native dispatch owns Enter/Escape; ordinary host values own acceptance.
use super::*;
use crate::fields::{LiveTextField, TextField, TextFieldEvent, TextFieldOutcome};
use kedra_text_input::TextSession;

#[derive(Resource, Default)]
struct Outcomes(Vec<TextFieldEvent>);
type TestResult = Result<(), String>;
fn field() -> Result<(App, Entity, Entity), String> {
    let mut app = app();
    app.init_resource::<Outcomes>().add_observer(
        |event: On<TextFieldEvent>, mut events: ResMut<Outcomes>| events.0.push(event.clone()),
    );
    let (window, root) = surface(&mut app, true);
    let text = EditableText::new("value");
    let session = TextSession::attach(&text, 32).map_err(|e| format!("{e:?}"))?;
    let input = app
        .world_mut()
        .spawn((
            TextInput,
            text,
            session,
            Node::default(),
            TabIndex(0),
            crate::authored::components::UiEditable {
                autofocus: false,
                select_all: false,
            },
            InheritedVisibility::VISIBLE,
            ChildOf(root),
        ))
        .id();
    app.update();
    TextField::attach(app.world_mut(), input, 1).map_err(|e| format!("{e:?}"))?;
    app.insert_resource(InputFocus::from_entity(input));
    Ok((app, window, input))
}
fn candidate(app: &App) -> Result<crate::fields::AcceptanceRequest, String> {
    app.world()
        .resource::<Outcomes>()
        .0
        .iter()
        .find_map(|event| match &event.outcome {
            TextFieldOutcome::Requested(request) => Some(request.clone()),
            _ => None,
        })
        .ok_or("Missing candidate".into())
}
#[test]
fn enter_captures_preceding_native_edits_and_freezes_until_host_response() -> TestResult {
    let (mut app, window, input) = field()?;
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("text")?
        .queue_edit(bevy::text::TextEdit::Insert("🙂".into()));
    key(&mut app, window, KeyCode::Enter, Key::Enter);
    let request = candidate(&app)?;
    assert_eq!(request.value(), "value🙂");
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(input));
    assert_eq!(
        app.world().get::<bevy::text::TextReadWriteMode>(input),
        Some(&bevy::text::TextReadWriteMode::ReadOnly)
    );
    TextField::refresh(app.world_mut(), input, "value", 1).map_err(|e| format!("{e:?}"))?;
    assert!(
        app.world()
            .get::<TextField>(input)
            .ok_or("field")?
            .awaiting_response()
    );
    key(&mut app, window, KeyCode::Backspace, Key::Backspace);
    assert_eq!(
        app.world()
            .get::<EditableText>(input)
            .ok_or("text")?
            .value(),
        "value🙂"
    );
    key(&mut app, window, KeyCode::Enter, Key::Enter);
    assert_eq!(
        app.world()
            .resource::<Outcomes>()
            .0
            .iter()
            .filter(|e| matches!(e.outcome, TextFieldOutcome::Requested(_)))
            .count(),
        1
    );
    assert!(TextField::is_current(app.world(), input, &request));
    TextField::accept(app.world_mut(), input, &request, 2).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        app.world()
            .get::<TextField>(input)
            .ok_or("field")?
            .accepted(),
        "value🙂"
    );
    assert_eq!(
        app.world().get::<bevy::text::TextReadWriteMode>(input),
        Some(&bevy::text::TextReadWriteMode::Editable)
    );
    Ok(())
}
#[test]
fn escape_revokes_a_pending_candidate_without_blurring_or_dismissing_its_owner() -> TestResult {
    let (mut app, window, input) = field()?;
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("text")?
        .queue_edit(bevy::text::TextEdit::Insert("draft".into()));
    key(&mut app, window, KeyCode::Enter, Key::Enter);
    let request = candidate(&app)?;
    key(&mut app, window, KeyCode::Escape, Key::Escape);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(input));
    assert_eq!(
        app.world()
            .get::<EditableText>(input)
            .ok_or("text")?
            .value(),
        "value"
    );
    assert!(!TextField::is_current(app.world(), input, &request));
    assert!(TextField::accept(app.world_mut(), input, &request, 2).is_err());
    assert!(
        app.world()
            .resource::<Outcomes>()
            .0
            .iter()
            .any(|e| matches!(e.outcome, TextFieldOutcome::Cancelled))
    );
    Ok(())
}
#[test]
fn same_batch_ime_commit_and_enter_or_escape_never_accepts_or_cancels_the_field() -> TestResult {
    for (code, logical) in [
        (KeyCode::Enter, Key::Enter),
        (KeyCode::Escape, Key::Escape),
        (KeyCode::Tab, Key::Tab),
    ] {
        let (mut app, window, input) = field()?;
        app.world_mut().write_message(bevy::window::Ime::Commit {
            window,
            value: "に".into(),
        });
        key(&mut app, window, code, logical);
        assert!(app.world().resource::<Outcomes>().0.is_empty());
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(input));
        assert_eq!(
            app.world()
                .get::<EditableText>(input)
                .ok_or("text")?
                .value(),
            "valueに"
        );
    }
    Ok(())
}
#[test]
fn foreign_window_and_lost_access_cannot_authorize_a_pending_consumer_value() -> TestResult {
    let (mut app, window, input) = field()?;
    let (foreign, _) = surface(&mut app, false);
    key(&mut app, foreign, KeyCode::Enter, Key::Enter);
    assert!(app.world().resource::<Outcomes>().0.is_empty());
    app.insert_resource(InputFocus::from_entity(input));
    key(&mut app, window, KeyCode::Enter, Key::Enter);
    let request = candidate(&app)?;
    app.world_mut()
        .entity_mut(input)
        .insert(crate::fields::FieldAccess::ReadOnly {
            reason: "Host access revoked".into(),
        });
    assert!(!TextField::is_current(app.world(), input, &request));
    assert!(TextField::accept(app.world_mut(), input, &request, 2).is_err());
    assert_eq!(
        app.world()
            .get::<TextField>(input)
            .ok_or("field")?
            .accepted(),
        "value"
    );
    Ok(())
}

#[test]
fn active_preedit_keeps_escape_and_enter_inside_composition_without_losing_focus() -> TestResult {
    let (mut app, window, input) = field()?;
    app.world_mut().write_message(bevy::window::Ime::Preedit {
        window,
        value: "に".into(),
        cursor: None,
    });
    app.update();
    assert!(
        app.world()
            .get::<EditableText>(input)
            .ok_or("text")?
            .is_composing()
    );
    for (code, logical) in [(KeyCode::Enter, Key::Enter), (KeyCode::Escape, Key::Escape)] {
        key(&mut app, window, code, logical);
        assert!(app.world().resource::<Outcomes>().0.is_empty());
        assert!(
            app.world()
                .get::<EditableText>(input)
                .ok_or("text")?
                .is_composing()
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(input));
        assert_eq!(
            app.world()
                .get::<TextField>(input)
                .ok_or("field")?
                .accepted(),
            "value"
        );
    }
    Ok(())
}

#[test]
fn replacing_a_binding_on_the_same_input_retires_older_native_notifications() -> TestResult {
    let (mut app, window, input) = field()?;
    key(&mut app, window, KeyCode::Enter, Key::Enter);
    let old_request = candidate(&app)?;
    let old_generation = app
        .world()
        .get::<TextSession>(input)
        .ok_or("session")?
        .generation();
    app.world_mut().entity_mut(input).remove::<TextField>();
    TextField::attach(app.world_mut(), input, 2).map_err(|e| format!("{e:?}"))?;
    app.world_mut().resource_mut::<Outcomes>().0.clear();
    app.world_mut().trigger(kedra_text_input::InputEvent {
        entity: input,
        generation: old_generation,
        outcome: kedra_text_input::Outcome::Read {
            request: 1,
            text: "value".into(),
        },
    });
    app.world_mut().flush();
    assert!(app.world().resource::<Outcomes>().0.is_empty());
    assert!(!TextField::is_current(app.world(), input, &old_request));
    key(&mut app, window, KeyCode::Enter, Key::Enter);
    let request = candidate(&app)?;
    assert_eq!(request.baseline(), 2);
    assert!(TextField::is_current(app.world(), input, &request));
    Ok(())
}

// Shared field scheduling owns live capture and immutable gesture stamps. These
// headless dispatch cases do not certify physical IME or editor source history.
#[test]
fn live_capture_keeps_the_original_gesture_and_rejection_waits_for_another_edit() -> TestResult {
    let (mut app, window, input) = field()?;
    app.world_mut()
        .entity_mut(input)
        .insert(LiveTextField::default());
    app.update();
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("text")?
        .queue_edit(bevy::text::TextEdit::Insert("1".into()));
    app.update();
    app.update();
    let request = candidate(&app)?;
    let gesture = app
        .world()
        .resource::<Outcomes>()
        .0
        .last()
        .ok_or("event")?
        .edit_session;
    assert!(gesture.is_some());
    TextField::reject(app.world_mut(), input, &request).map_err(|e| format!("{e:?}"))?;
    app.world_mut().resource_mut::<Outcomes>().0.clear();
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().resource::<Outcomes>().0.is_empty());
    assert_eq!(
        app.world()
            .get::<TextField>(input)
            .ok_or("field")?
            .accepted(),
        "value"
    );
    assert_eq!(
        app.world()
            .get::<EditableText>(input)
            .ok_or("text")?
            .value(),
        "value1"
    );
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("text")?
        .queue_edit(bevy::text::TextEdit::Insert("2".into()));
    // Enter completes the gesture before the queued ordered read is delivered.
    key(&mut app, window, KeyCode::Enter, Key::Enter);
    let request = candidate(&app)?;
    assert_eq!(request.value(), "value12");
    assert_eq!(
        app.world()
            .resource::<Outcomes>()
            .0
            .last()
            .ok_or("event")?
            .edit_session,
        gesture
    );
    TextField::accept(app.world_mut(), input, &request, 2).map_err(|e| format!("{e:?}"))?;
    app.world_mut().resource_mut::<Outcomes>().0.clear();
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("text")?
        .queue_edit(bevy::text::TextEdit::Insert("3".into()));
    app.update();
    app.update();
    assert_eq!(candidate(&app)?.value(), "value123");
    assert_ne!(
        app.world()
            .resource::<Outcomes>()
            .0
            .last()
            .ok_or("event")?
            .edit_session,
        gesture
    );
    Ok(())
}

#[test]
fn live_capture_waits_for_ime_and_cannot_publish_after_stale_refresh_or_access_loss() -> TestResult
{
    let (mut app, window, input) = field()?;
    app.world_mut()
        .entity_mut(input)
        .insert(LiveTextField::default());
    app.world_mut().write_message(bevy::window::Ime::Preedit {
        window,
        value: "に".into(),
        cursor: None,
    });
    app.update();
    app.update();
    assert!(app.world().resource::<Outcomes>().0.is_empty());
    assert_eq!(
        app.world()
            .get::<TextField>(input)
            .ok_or("field")?
            .accepted(),
        "value"
    );
    app.world_mut().write_message(bevy::window::Ime::Commit {
        window,
        value: "日本".into(),
    });
    app.update();
    app.update();
    let request = candidate(&app)?;
    assert_eq!(request.value(), "value日本");
    app.world_mut()
        .entity_mut(input)
        .insert(crate::fields::FieldAccess::ReadOnly {
            reason: "Revoked".into(),
        });
    assert!(TextField::accept(app.world_mut(), input, &request, 2).is_err());
    assert_eq!(
        app.world()
            .get::<TextField>(input)
            .ok_or("field")?
            .accepted(),
        "value"
    );
    app.world_mut()
        .entity_mut(input)
        .insert(crate::fields::FieldAccess::Editable);
    TextField::refresh(app.world_mut(), input, "external", 3).map_err(|e| format!("{e:?}"))?;
    app.world_mut().resource_mut::<Outcomes>().0.clear();
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("text")?
        .queue_edit(bevy::text::TextEdit::Insert("x".into()));
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().resource::<Outcomes>().0.is_empty());
    assert!(!TextField::is_current(app.world(), input, &request));
    assert_eq!(
        app.world()
            .get::<TextField>(input)
            .ok_or("field")?
            .accepted(),
        "external"
    );
    assert!(
        app.world()
            .get::<EditableText>(input)
            .ok_or("text")?
            .value()
            .to_string()
            .starts_with("value日本")
    );
    Ok(())
}

#[test]
fn settled_live_history_delegates_but_rejected_drafts_keep_local_undo() -> TestResult {
    let (mut app, window, input) = field()?;
    app.world_mut()
        .entity_mut(input)
        .insert(LiveTextField::default());
    app.update();
    let modifier = if cfg!(target_os = "macos") {
        Key::Super
    } else {
        Key::Control
    };
    // Insert after the input plugin's reset; dispatch itself reads this same state.
    app.add_systems(
        PreUpdate,
        (move |mut keys: ResMut<ButtonInput<Key>>| {
            keys.press(modifier.clone());
        })
        .after(bevy::input::InputSystems),
    );
    key(&mut app, window, KeyCode::KeyZ, Key::Character("z".into()));
    assert!(matches!(
        app.world()
            .resource::<Outcomes>()
            .0
            .last()
            .ok_or("event")?
            .outcome,
        TextFieldOutcome::HistoryRequested { redo: false }
    ));
    app.world_mut().resource_mut::<Outcomes>().0.clear();
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("text")?
        .queue_edit(bevy::text::TextEdit::Insert("bad".into()));
    app.update();
    app.update();
    let request = candidate(&app)?;
    TextField::reject(app.world_mut(), input, &request).map_err(|e| format!("{e:?}"))?;
    app.world_mut().resource_mut::<Outcomes>().0.clear();
    key(&mut app, window, KeyCode::KeyZ, Key::Character("z".into()));
    assert!(app.world().resource::<Outcomes>().0.is_empty());
    assert_eq!(
        app.world()
            .get::<EditableText>(input)
            .ok_or("text")?
            .value(),
        "value"
    );
    assert_eq!(
        app.world()
            .get::<TextField>(input)
            .ok_or("field")?
            .accepted(),
        "value"
    );
    Ok(())
}

#[test]
fn blur_captures_the_last_change_under_its_original_gesture_then_starts_another() -> TestResult {
    let (mut app, _, input) = field()?;
    app.world_mut()
        .entity_mut(input)
        .insert(LiveTextField::default());
    app.update();
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("text")?
        .queue_edit(bevy::text::TextEdit::Insert("1".into()));
    app.update();
    app.update();
    let first = candidate(&app)?;
    let gesture = app
        .world()
        .resource::<Outcomes>()
        .0
        .last()
        .ok_or("event")?
        .edit_session;
    TextField::accept(app.world_mut(), input, &first, 2).map_err(|e| format!("{e:?}"))?;
    app.world_mut().resource_mut::<Outcomes>().0.clear();
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("text")?
        .queue_edit(bevy::text::TextEdit::Insert("2".into()));
    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();
    app.update();
    let last = candidate(&app)?;
    assert_eq!(last.value(), "value12");
    assert_eq!(
        app.world()
            .resource::<Outcomes>()
            .0
            .last()
            .ok_or("event")?
            .edit_session,
        gesture
    );
    TextField::accept(app.world_mut(), input, &last, 3).map_err(|e| format!("{e:?}"))?;
    app.world_mut().resource_mut::<Outcomes>().0.clear();
    app.insert_resource(InputFocus::from_entity(input));
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("text")?
        .queue_edit(bevy::text::TextEdit::Insert("3".into()));
    app.update();
    app.update();
    assert_ne!(
        app.world()
            .resource::<Outcomes>()
            .0
            .last()
            .ok_or("event")?
            .edit_session,
        gesture
    );
    Ok(())
}
