use crate::support::*;
use bevy::text::{EditableText, PreeditCursor, TextEdit};
use kedra_text_input::{Command, Outcome, Refusal};

#[test]
fn preedit_is_not_history_and_committed_selection_replacement_is_one_atomic_step() -> TestResult {
    let (mut app, input) = app("original", 8)?;
    edit(&mut app, input, TextEdit::SelectAll)?;
    app.update();
    let before = selection(&app, input)?;
    for value in ["n", "ni", "日本語"] {
        edit(
            &mut app,
            input,
            TextEdit::ImeSetCompose {
                value: value.into(),
                cursor: None,
            },
        )?;
        app.update();
    }
    command(&mut app, input, Command::Read(5))?;
    app.update();
    assert_eq!(
        app.world()
            .resource::<Events>()
            .0
            .last()
            .ok_or("refusal")?
            .outcome,
        Outcome::Refused(Refusal::Composing)
    );
    edit(
        &mut app,
        input,
        TextEdit::ImeCommit {
            value: "日本語🙂".into(),
        },
    )?;
    app.update();
    assert_eq!(value(&app, input)?, "日本語🙂");
    command(&mut app, input, Command::Undo)?;
    app.update();
    assert_eq!(value(&app, input)?, "original");
    assert_eq!(selection(&app, input)?, before);
    command(&mut app, input, Command::Undo)?;
    app.update();
    assert_eq!(value(&app, input)?, "original");
    Ok(())
}

#[test]
fn rejected_commit_and_cancelled_composition_restore_the_original_native_selection() -> TestResult {
    let (mut app, input) = app("kept", 4)?;
    edit(&mut app, input, TextEdit::SelectAll)?;
    app.update();
    let before = selection(&app, input)?;
    for ending in [
        TextEdit::ImeCommit {
            value: "too long".into(),
        },
        TextEdit::clear_ime_compose(),
    ] {
        edit(
            &mut app,
            input,
            TextEdit::ImeSetCompose {
                value: "に".into(),
                cursor: Some(PreeditCursor {
                    anchor: 3,
                    focus: 3,
                }),
            },
        )?;
        app.update();
        edit(&mut app, input, ending)?;
        app.update();
        assert_eq!(value(&app, input)?, "kept");
        assert_eq!(selection(&app, input)?, before);
        assert!(
            !app.world()
                .get::<EditableText>(input)
                .ok_or("input")?
                .is_composing()
        );
    }
    Ok(())
}

#[test]
fn an_external_replacement_during_preedit_cannot_restore_the_old_composition_baseline() -> TestResult
{
    let (mut app, input) = app("old", 32)?;
    edit(&mut app, input, TextEdit::SelectAll)?;
    edit(
        &mut app,
        input,
        TextEdit::ImeSetCompose {
            value: "に".into(),
            cursor: None,
        },
    )?;
    app.update();
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("input")?
        .editor_mut()
        .set_text("replacement");
    edit(&mut app, input, TextEdit::clear_ime_compose())?;
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
    command(&mut app, input, Command::Undo)?;
    app.update();
    assert_eq!(value(&app, input)?, "replacement");
    Ok(())
}
