//! Controlled native clipboard readiness: queue ownership, refusal and retirement.
use super::*;
use bevy::{
    platform::sync::{Arc, Mutex},
    text::TextLayout,
};
use bevy_clipboard::{ClipboardError, ClipboardRead};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[derive(Resource, Default)]
struct Events(Vec<Outcome>);
fn host() -> Result<(App, Entity), String> {
    let mut app = App::new();
    app.init_resource::<FontCx>()
        .init_resource::<LayoutCx>()
        .init_resource::<Clipboard>()
        .init_resource::<Events>()
        .add_systems(Update, apply)
        .add_observer(|event: On<InputEvent>, mut events: ResMut<Events>| {
            events.0.push(event.outcome.clone())
        });
    let text = EditableText::new("A");
    let session = TextSession::attach(&text, 32).map_err(|error| format!("{error:?}"))?;
    let input = app
        .world_mut()
        .spawn((text, session, TextLayout::default()))
        .id();
    app.update();
    Ok((app, input))
}

#[test]
fn unresolved_paste_holds_later_native_edits_and_read_at_the_same_owned_barrier() -> TestResult {
    let (mut app, input) = host()?;
    let shared = Arc::new(Mutex::new(None));
    {
        let mut session = app
            .world_mut()
            .get_mut::<TextSession>(input)
            .ok_or("session")?;
        session.paste = Some(ClipboardRead::Pending(shared.clone()));
        session
            .queue
            .push_back(Operation::Edit(TextEdit::Insert("Z".into())));
        session
            .queue
            .push_back(Operation::Control(Command::Read(9)));
    }
    app.update();
    assert_eq!(
        app.world()
            .get::<EditableText>(input)
            .ok_or("input")?
            .value(),
        "A"
    );
    assert!(
        app.world()
            .get::<TextSession>(input)
            .ok_or("session")?
            .pending()
    );
    assert!(app.world().resource::<Events>().0.is_empty());
    *shared.lock().map_err(|_| "clipboard lock")? = Some(Ok("🙂".into()));
    app.update();
    assert_eq!(
        app.world()
            .get::<EditableText>(input)
            .ok_or("input")?
            .value(),
        "A🙂Z"
    );
    assert_eq!(
        app.world().resource::<Events>().0,
        [Outcome::Read {
            request: 9,
            text: "A🙂Z".into()
        }]
    );
    Ok(())
}

#[test]
fn failed_paste_preserves_the_buffer_and_rejects_a_read_of_its_old_value() -> TestResult {
    let (mut app, input) = host()?;
    {
        let mut session = app
            .world_mut()
            .get_mut::<TextSession>(input)
            .ok_or("session")?;
        session.paste = Some(ClipboardRead::Ready(Err(
            ClipboardError::ContentNotAvailable,
        )));
        session
            .queue
            .push_back(Operation::Control(Command::Read(10)));
    }
    app.update();
    assert_eq!(
        app.world()
            .get::<EditableText>(input)
            .ok_or("input")?
            .value(),
        "A"
    );
    assert_eq!(
        app.world().resource::<Events>().0,
        [
            Outcome::Refused(Refusal::Clipboard),
            Outcome::Refused(Refusal::Clipboard)
        ]
    );
    Ok(())
}

#[test]
fn binding_discard_revokes_an_unresolved_paste_without_polling_it_again() -> TestResult {
    let (mut app, input) = host()?;
    let shared = Arc::new(Mutex::new(None));
    app.world_mut()
        .get_mut::<TextSession>(input)
        .ok_or("session")?
        .paste = Some(ClipboardRead::Pending(shared.clone()));
    app.update();
    {
        let world = app.world_mut();
        let mut query = world.query::<(&mut TextSession, &mut EditableText)>();
        let (mut session, mut text) = query.get_mut(world, input)?;
        session
            .discard(&mut text)
            .map_err(|error| format!("{error:?}"))?;
        text.editor_mut().set_text("replacement");
    }
    *shared.lock().map_err(|_| "clipboard lock")? = Some(Ok("stale paste".into()));
    app.update();
    assert_eq!(
        app.world()
            .get::<EditableText>(input)
            .ok_or("input")?
            .value(),
        "replacement"
    );
    assert!(
        shared.lock().map_err(|_| "clipboard lock")?.is_some(),
        "Retired paste must not be polled by a new binding"
    );
    Ok(())
}

#[test]
fn generation_exhaustion_refuses_mutation_without_reusing_a_read_token() -> TestResult {
    let (mut app, input) = host()?;
    {
        let mut session = app
            .world_mut()
            .get_mut::<TextSession>(input)
            .ok_or("session")?;
        session.generation = u64::MAX;
        session
            .queue
            .push_back(Operation::Edit(TextEdit::Insert("new".into())));
        session
            .queue
            .push_back(Operation::Control(Command::Read(13)));
    }
    app.update();
    assert_eq!(
        app.world()
            .get::<EditableText>(input)
            .ok_or("input")?
            .value(),
        "A"
    );
    assert_eq!(
        app.world().resource::<Events>().0,
        [
            Outcome::Refused(Refusal::GenerationExhausted),
            Outcome::Refused(Refusal::GenerationExhausted)
        ]
    );
    assert_eq!(
        app.world()
            .get::<TextSession>(input)
            .ok_or("session")?
            .generation(),
        u64::MAX
    );
    Ok(())
}
