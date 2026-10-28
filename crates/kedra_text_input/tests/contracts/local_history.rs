use crate::support::*;
use bevy::{
    input::ButtonState,
    input::keyboard::{Key, KeyboardInput},
    input_focus::FocusedInput,
    prelude::*,
    text::{EditableText, TextEdit},
};
use kedra_text_input::{Command, TextSession};

#[test]
fn multiple_mutations_in_one_frame_have_distinct_local_undo_boundaries() -> TestResult {
    let (mut app, input) = app("", 32)?;
    for value in ["a", "e\u{301}", "🙂"] {
        edit(&mut app, input, TextEdit::Insert(value.into()))?;
    }
    app.update();
    for expected in ["ae\u{301}", "a", ""] {
        command(&mut app, input, Command::Undo)?;
        app.update();
        assert_eq!(value(&app, input)?, expected);
    }
    command(&mut app, input, Command::Redo)?;
    app.update();
    assert_eq!(value(&app, input)?, "a");
    edit(&mut app, input, TextEdit::Insert("new".into()))?;
    app.update();
    command(&mut app, input, Command::Redo)?;
    app.update();
    assert_eq!(value(&app, input)?, "anew");
    Ok(())
}

#[test]
fn selection_and_host_geometry_are_preserved_without_reconstructing_byte_carets() -> TestResult {
    let (mut app, input) = app("שלום e\u{301}🙂", 64)?;
    edit(&mut app, input, TextEdit::TextStart(false))?;
    edit(&mut app, input, TextEdit::TextEnd(true))?;
    app.update();
    let before = selection(&app, input)?;
    edit(&mut app, input, TextEdit::Insert("replacement".into()))?;
    app.update();
    app.world_mut()
        .get_mut::<EditableText>(input)
        .ok_or("input")?
        .viewport
        .size = Vec2::new(480., 22.);
    command(&mut app, input, Command::Undo)?;
    app.update();
    assert_eq!(value(&app, input)?, "שלום e\u{301}🙂");
    assert_eq!(selection(&app, input)?, before);
    assert_eq!(
        app.world()
            .get::<EditableText>(input)
            .ok_or("input")?
            .viewport
            .size
            .x,
        480.
    );
    command(&mut app, input, Command::ResetHistory)?;
    app.update();
    command(&mut app, input, Command::Redo)?;
    app.update();
    assert_eq!(value(&app, input)?, "שלום e\u{301}🙂");
    Ok(())
}

#[test]
fn local_shortcut_is_consumed_even_with_empty_history() -> TestResult {
    #[derive(Resource, Default)]
    struct Escaped(bool);
    let (mut app, input) = app("kept", 32)?;
    let parent = app.world_mut().spawn_empty().id();
    let window = app.world_mut().spawn(Window::default()).id();
    app.world_mut().entity_mut(input).insert(ChildOf(parent));
    app.init_resource::<Escaped>();
    app.world_mut().entity_mut(parent).observe(
        |_: On<FocusedInput<KeyboardInput>>, mut escaped: ResMut<Escaped>| escaped.0 = true,
    );
    app.world_mut()
        .resource_mut::<ButtonInput<Key>>()
        .press(if cfg!(target_os = "macos") {
            Key::Super
        } else {
            Key::Control
        });
    let event = KeyboardInput {
        key_code: KeyCode::KeyZ,
        logical_key: Key::Character("я".into()),
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    };
    let consumed = kedra_text_input::handle_keyboard(app.world_mut(), input, &event);
    if !consumed {
        app.world_mut()
            .trigger(FocusedInput::new(input, event, window));
    }
    app.update();
    assert!(consumed);
    assert!(!app.world().resource::<Escaped>().0);
    assert_eq!(value(&app, input)?, "kept");
    assert!(
        !app.world()
            .get::<TextSession>(input)
            .ok_or("session")?
            .pending()
    );
    Ok(())
}

#[test]
fn undo_invalidates_native_glyph_layout_even_when_snapshot_generations_alias() -> TestResult {
    let (mut app, input) = app("A", 32)?;
    edit(&mut app, input, TextEdit::Insert("B".into()))?;
    app.update();
    let displayed = app
        .world()
        .get::<EditableText>(input)
        .ok_or("input")?
        .editor
        .generation();
    **app
        .world_mut()
        .get_mut::<bevy::text::EditableTextGeneration>(input)
        .ok_or("render generation")? = displayed;
    command(&mut app, input, Command::Undo)?;
    app.update();
    assert_eq!(value(&app, input)?, "A");
    assert_ne!(
        app.world()
            .get::<EditableText>(input)
            .ok_or("input")?
            .editor
            .generation(),
        displayed,
        "Restored text must invalidate the renderer's previous glyph layout"
    );
    Ok(())
}
