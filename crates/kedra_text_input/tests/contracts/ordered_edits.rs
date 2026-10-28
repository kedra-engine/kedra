use crate::support::*;
use bevy::text::{EditableText, TextEdit, TextReadWriteMode};
use kedra_text_input::{Command, Outcome, Refusal, TextSession};

#[test]
fn queued_read_observes_preceding_edits_but_not_later_input() -> TestResult {
    let (mut app, input) = app("A", 32)?;
    edit(&mut app, input, TextEdit::Insert("🙂".into()))?;
    command(&mut app, input, Command::Read(7))?;
    edit(&mut app, input, TextEdit::Insert("Z".into()))?;
    app.update();
    assert_eq!(value(&app, input)?, "A🙂Z");
    let event = app.world().resource::<Events>().0.last().ok_or("read")?;
    assert_eq!(
        event.outcome,
        Outcome::Read {
            request: 7,
            text: "A🙂".into()
        }
    );
    assert!(
        event.generation
            < app
                .world()
                .get::<TextSession>(input)
                .ok_or("session")?
                .generation()
    );
    Ok(())
}

#[test]
fn rejected_replacement_preserves_native_selection_and_cannot_submit_the_old_value() -> TestResult {
    let (mut app, input) = app("e\u{301}🙂", 4)?;
    edit(&mut app, input, TextEdit::SelectAll)?;
    app.update();
    let before = selection(&app, input)?;
    for (text, reason) in [
        ("12345", Refusal::ScalarLimit),
        ("x\ny", Refusal::ForbiddenCharacter),
        ("x\u{2028}y", Refusal::ForbiddenCharacter),
    ] {
        edit(&mut app, input, TextEdit::Insert(text.into()))?;
        command(&mut app, input, Command::Read(1))?;
        app.update();
        assert_eq!(value(&app, input)?, "e\u{301}🙂");
        assert_eq!(selection(&app, input)?, before);
        assert_eq!(
            app.world()
                .resource::<Events>()
                .0
                .last()
                .ok_or("refusal")?
                .outcome,
            Outcome::Refused(reason)
        );
    }
    edit(&mut app, input, TextEdit::Insert(" 你好 ".into()))?;
    app.update();
    assert_eq!(value(&app, input)?, " 你好 ");
    Ok(())
}

#[test]
fn access_loss_refuses_queued_mutation_and_read_without_touching_the_buffer() -> TestResult {
    let (mut app, input) = app("source", 32)?;
    edit(&mut app, input, TextEdit::Insert("draft".into()))?;
    command(&mut app, input, Command::Read(2))?;
    app.world_mut()
        .entity_mut(input)
        .insert(TextReadWriteMode::ReadOnly);
    app.update();
    assert_eq!(value(&app, input)?, "source");
    assert!(
        app.world()
            .resource::<Events>()
            .0
            .iter()
            .all(|event| event.outcome == Outcome::Refused(Refusal::ReadOnly))
    );
    Ok(())
}

#[test]
fn external_replacement_revokes_pending_work_instead_of_appending_to_a_new_binding() -> TestResult {
    let (mut app, input) = app("old", 32)?;
    edit(&mut app, input, TextEdit::Insert("stale".into()))?;
    command(&mut app, input, Command::Read(3))?;
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("input")?
        .editor_mut()
        .set_text("replacement");
    app.update();
    assert_eq!(value(&app, input)?, "replacement");
    assert_eq!(
        app.world()
            .resource::<Events>()
            .0
            .last()
            .ok_or("refusal")?
            .outcome,
        Outcome::Refused(Refusal::Replaced)
    );
    assert!(
        !app.world()
            .get::<TextSession>(input)
            .ok_or("session")?
            .pending()
    );
    Ok(())
}

#[test]
fn paste_uses_the_owned_clipboard_once_and_applies_the_same_atomic_guard() -> TestResult {
    let (mut app, input) = app("kept", 8)?;
    edit(&mut app, input, TextEdit::SelectAll)?;
    app.update();
    let before = selection(&app, input)?;
    app.world_mut()
        .resource_mut::<bevy_clipboard::Clipboard>()
        .set_text("not\nsingle-line")?;
    edit(&mut app, input, TextEdit::Paste)?;
    command(&mut app, input, Command::Read(4))?;
    app.update();
    assert_eq!(value(&app, input)?, "kept");
    assert_eq!(selection(&app, input)?, before);
    assert_eq!(
        app.world()
            .resource::<Events>()
            .0
            .last()
            .ok_or("refusal")?
            .outcome,
        Outcome::Refused(Refusal::ForbiddenCharacter)
    );
    app.world_mut()
        .resource_mut::<bevy_clipboard::Clipboard>()
        .set_text("日本語🙂")?;
    edit(&mut app, input, TextEdit::Paste)?;
    app.update();
    assert_eq!(value(&app, input)?, "日本語🙂");
    command(&mut app, input, Command::Undo)?;
    app.update();
    assert_eq!(value(&app, input)?, "kept");
    assert_eq!(selection(&app, input)?, before);
    Ok(())
}

#[test]
fn explicit_replacement_validates_before_revoking_work_and_starts_a_new_history_epoch() -> TestResult
{
    let (mut app, input) = app("A", 8)?;
    edit(&mut app, input, TextEdit::Insert("B".into()))?;
    command(&mut app, input, Command::Read(11))?;
    {
        let world = app.world_mut();
        let mut query = world.query::<(&mut TextSession, &mut EditableText)>();
        let (mut session, mut text) = query.get_mut(world, input)?;
        assert_eq!(
            session.replace(&mut text, "bad\nvalue"),
            Err(Refusal::ForbiddenCharacter)
        );
        assert!(session.pending());
    }
    app.update();
    assert_eq!(value(&app, input)?, "AB");
    command(&mut app, input, Command::Read(12))?;
    {
        let world = app.world_mut();
        let mut query = world.query::<(&mut TextSession, &mut EditableText)>();
        let (mut session, mut text) = query.get_mut(world, input)?;
        let before = session.generation();
        session
            .replace(&mut text, "C")
            .map_err(|error| format!("{error:?}"))?;
        assert!(session.generation() > before);
    }
    app.update();
    assert_eq!(value(&app, input)?, "C");
    assert!(
        !app.world()
            .resource::<Events>()
            .0
            .iter()
            .any(|event| matches!(event.outcome, Outcome::Read { request: 12, .. }))
    );
    command(&mut app, input, Command::Undo)?;
    app.update();
    assert_eq!(value(&app, input)?, "C");
    Ok(())
}
