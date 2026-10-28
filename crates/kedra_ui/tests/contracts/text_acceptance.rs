//! Field acknowledgment contracts over real ordered native edits, without a renderer.
use bevy::{
    asset::AssetPlugin,
    input::InputPlugin,
    input_focus::{InputDispatchPlugin, InputFocusPlugin},
    prelude::*,
    text::{EditableText, TextEdit, TextPlugin},
    ui_widgets::{TextInput, TextInputPlugin},
};
use kedra_ui::{
    fields::{AcceptanceError, AcceptanceRequest, TextAcceptance},
    text_input::{Command, InputEvent, Refusal, TextSession, TextSessionPlugin},
};
type TestResult = Result<(), String>;
#[derive(Resource, Default)]
struct Events(Vec<InputEvent>);
struct Field {
    app: App,
    input: Entity,
    acceptance: TextAcceptance,
}
impl Field {
    fn new() -> Result<Self, String> {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            WindowPlugin {
                primary_window: None,
                ..default()
            },
            InputPlugin,
            InputFocusPlugin,
            InputDispatchPlugin,
            TextPlugin,
            TextInputPlugin,
            TextSessionPlugin,
        ))
        .init_resource::<bevy::picking::events::PointerState>()
        .init_resource::<UiScale>()
        .init_resource::<Events>()
        .add_message::<bevy::picking::events::PointerRelease>()
        .add_observer(|event: On<InputEvent>, mut events: ResMut<Events>| {
            events.0.push(event.clone())
        });
        let text = EditableText::new("original");
        let session = TextSession::attach(&text, 16).map_err(|e| format!("{e:?}"))?;
        let input = app.world_mut().spawn_empty().id();
        app.world_mut()
            .entity_mut(input)
            .insert((TextInput, text, session));
        app.finish();
        app.cleanup();
        app.update();
        let text = app
            .world()
            .get::<EditableText>(input)
            .ok_or("Missing text")?;
        let session = app
            .world()
            .get::<TextSession>(input)
            .ok_or("Missing session")?;
        let acceptance =
            TextAcceptance::new(input, text, session, 1).map_err(|e| format!("{e:?}"))?;
        Ok(Self {
            app,
            input,
            acceptance,
        })
    }
    fn with<T>(
        &mut self,
        f: impl FnOnce(&mut TextAcceptance, &mut EditableText, &mut TextSession) -> T,
    ) -> Result<T, String> {
        let world = self.app.world_mut();
        let mut query = world.query::<(&mut EditableText, &mut TextSession)>();
        let (mut text, mut session) = query
            .get_mut(world, self.input)
            .map_err(|e| e.to_string())?;
        Ok(f(&mut self.acceptance, &mut text, &mut session))
    }
    fn edit(&mut self, value: &str) -> TestResult {
        self.with(|_, text, _| text.queue_edit(TextEdit::Insert(value.into())))?;
        Ok(())
    }
    fn request(&mut self) -> TestResult {
        self.with(|a, text, session| a.request(text, session))?
            .map_err(|e| format!("{e:?}"))
    }
    fn capture(&mut self) -> Result<AcceptanceRequest, String> {
        self.app.update();
        let events = std::mem::take(&mut self.app.world_mut().resource_mut::<Events>().0);
        for event in events {
            if let Some(request) = self
                .with(|a, text, session| a.capture(&event, text, session))?
                .map_err(|e| format!("{e:?}"))?
            {
                return Ok(request);
            }
        }
        Err("No acceptance candidate".into())
    }
    fn value(&self) -> Result<String, String> {
        self.app
            .world()
            .get::<EditableText>(self.input)
            .map(|text| text.value().to_string())
            .ok_or("Missing input".into())
    }
}

#[test]
fn rejection_retains_native_draft_and_local_undo_while_acceptance_starts_a_new_epoch() -> TestResult
{
    let mut field = Field::new()?;
    field.edit("🙂")?;
    field.request()?;
    let request = field.capture()?;
    assert_eq!(request.value(), "original🙂");
    assert_eq!(request.baseline(), 1);
    let selection = field.with(|_, text, _| *text.editor.raw_selection())?;
    field
        .with(|a, text, session| a.reject(&request, text, session))?
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        field.with(|_, text, _| *text.editor.raw_selection())?,
        selection
    );
    assert_eq!(field.acceptance.accepted(), "original");
    field.with(|_, text, session| session.request(text, Command::Undo))?;
    field.app.update();
    assert_eq!(field.value()?, "original");
    field.edit(" ok")?;
    field.request()?;
    let request = field.capture()?;
    // Independent ordinary-string consumer: domain validation/publication is not
    // performed by the field and has no project/source filesystem authority.
    let mut consumer = String::from("original");
    field
        .with(|a, text, session| {
            assert!(a.is_current(&request, text, session));
            consumer = request.value().into();
            a.accept(&request, 2, text, session)
        })?
        .map_err(|e| format!("{e:?}"))?;
    field.with(|_, text, session| session.request(text, Command::Undo))?;
    field.app.update();
    assert_eq!(field.value()?, consumer);
    assert_eq!(field.acceptance.accepted(), consumer);
    Ok(())
}

#[test]
fn cancellation_restores_latest_consumer_value_and_retires_old_acknowledgments() -> TestResult {
    let mut field = Field::new()?;
    field.edit("draft")?;
    field.request()?;
    let request = field.capture()?;
    field
        .with(|a, text, session| a.refresh("external", 2, text, session))?
        .map_err(|e| format!("{e:?}"))?;
    assert!(field.acceptance.stale());
    assert_eq!(field.value()?, "originaldraft");
    assert_eq!(
        field.with(|a, text, session| a.accept(&request, 3, text, session))?,
        Err(AcceptanceError::Stale)
    );
    field
        .with(|a, text, session| a.cancel(text, session))?
        .map_err(|e| format!("{e:?}"))?;
    field.app.update();
    assert_eq!(field.value()?, "external");
    assert!(!field.acceptance.stale());
    assert!(!field.acceptance.pending());
    assert_eq!(
        field.with(|a, text, session| a.reject(&request, text, session))?,
        Err(AcceptanceError::Stale)
    );
    Ok(())
}

#[test]
fn later_native_edits_cannot_silently_join_an_earlier_acceptance_boundary() -> TestResult {
    let mut field = Field::new()?;
    field.edit("A")?;
    field.request()?;
    field.edit("B")?;
    assert!(field.capture().is_err());
    assert_eq!(field.value()?, "originalAB");
    assert_eq!(field.acceptance.accepted(), "original");
    assert!(!field.acceptance.pending());
    Ok(())
}

#[test]
fn duplicate_requests_and_foreign_binding_responses_have_no_authority() -> TestResult {
    let mut field = Field::new()?;
    field.request()?;
    assert_eq!(
        field.with(|a, text, session| a.request(text, session))?,
        Err(AcceptanceError::Pending)
    );
    let request = field.capture()?;
    let mut other = Field::new()?;
    other.request()?;
    let _other_request = other.capture()?;
    assert_eq!(
        other.with(|a, text, session| a.accept(&request, 2, text, session))?,
        Err(AcceptanceError::Stale)
    );
    assert!(other.acceptance.pending());
    // No-op still completes its local epoch, but the consumer need not advance
    // its revision or create an undo entry.
    field
        .with(|a, text, session| a.accept(&request, 1, text, session))?
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(field.acceptance.accepted(), "original");
    Ok(())
}

#[test]
fn invalid_refresh_and_failed_native_insertion_preserve_accepted_value_and_draft() -> TestResult {
    let mut field = Field::new()?;
    assert_eq!(
        field.with(|a, text, session| a.refresh("seventeen letters!", 2, text, session))?,
        Err(AcceptanceError::Native(Refusal::ScalarLimit))
    );
    assert_eq!(field.acceptance.accepted(), "original");
    assert_eq!(field.value()?, "original");
    field.edit("\n")?;
    field.request()?;
    assert!(field.capture().is_err());
    assert!(!field.acceptance.pending());
    assert_eq!(field.value()?, "original");
    assert_eq!(field.acceptance.accepted(), "original");
    Ok(())
}

#[test]
fn idle_refresh_is_not_a_user_edit_but_same_text_with_a_new_revision_invalidates_a_candidate()
-> TestResult {
    let mut field = Field::new()?;
    field
        .with(|a, text, session| a.refresh("new", 2, text, session))?
        .map_err(|e| format!("{e:?}"))?;
    field.app.update();
    assert_eq!(field.value()?, "new");
    assert!(!field.acceptance.pending());
    assert!(!field.acceptance.stale());
    field.request()?;
    let request = field.capture()?;
    field
        .with(|a, text, session| a.refresh("new", 3, text, session))?
        .map_err(|e| format!("{e:?}"))?;
    assert!(field.acceptance.stale());
    assert_eq!(
        field.with(|a, text, session| a.accept(&request, 4, text, session))?,
        Err(AcceptanceError::Stale)
    );
    assert_eq!(field.value()?, "new");
    Ok(())
}

#[test]
fn unsettled_history_reset_is_atomic_and_does_not_discard_queued_edits() -> TestResult {
    let mut field = Field::new()?;
    field.edit("A")?;
    let generation = field.with(|_, _, session| session.generation())?;
    assert_eq!(
        field.with(|_, text, session| session.reset_history(text))?,
        Err(Refusal::Unsettled)
    );
    assert_eq!(
        field.with(|_, _, session| session.generation())?,
        generation
    );
    field.app.update();
    assert_eq!(field.value()?, "originalA");
    field.with(|_, text, session| session.request(text, Command::Undo))?;
    field.app.update();
    assert_eq!(field.value()?, "original");
    Ok(())
}
