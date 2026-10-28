//! Opt-in retained native adapter. No source writes, second buffer or focus writer.
use super::{AcceptanceError, AcceptanceRequest, FieldAccess, TextAcceptance};
use bevy::{
    input::keyboard::{Key, KeyboardInput},
    prelude::*,
    text::{EditableText, TextEdit},
    ui::InteractionDisabled,
    window::Ime,
};
use kedra_text_input::{InputEvent, Outcome, TextSession};
mod live;
pub use live::LiveTextField;

/// One native input's retained acknowledgment binding. Attach only to a settled
/// managed `TextInput`; retiring/rebinding requires retiring this component too.
/// The host must guard its own access/revision before publication and respond via
/// [`Self::accept`] or [`Self::reject`]. Pending consumer responses freeze mutation
/// through the shared authored input-mode policy, retaining native selection/copy.
#[derive(Component)]
pub struct TextField {
    acceptance: TextAcceptance,
    responding: bool,
    epoch: u64,
    request_session: Option<u64>,
}

/// Native interaction outcome, not permission to publish a source transaction.
#[derive(Clone, Debug)]
pub enum TextFieldOutcome {
    /// Immutable candidate for synchronous host validation/revision checks.
    Requested(AcceptanceRequest),
    /// Local work was revoked and the latest accepted value restored.
    Cancelled,
    /// Input or candidate capture failed without accepting an older valid string.
    Refused(AcceptanceError),
    /// A settled live input delegates Undo/Redo to its consumer. Invalid or
    /// incomplete native drafts keep their own local history instead.
    HistoryRequested {
        /// Redo when true, otherwise Undo.
        redo: bool,
    },
}
/// Notification scoped to the original retained native input entity.
#[derive(EntityEvent, Clone, Debug)]
pub struct TextFieldEvent {
    /// Original input, never the currently focused replacement.
    pub entity: Entity,
    /// Candidate or local-only outcome.
    pub outcome: TextFieldOutcome,
    /// Original live gesture, when opted in. Hosts may coalesce accepted source
    /// changes for this exact input/session; it grants no publication authority.
    pub edit_session: Option<u64>,
}
impl TextField {
    /// Bind an existing native input and session. No presentation is reconstructed;
    /// initial pending native edits must have applied before attaching. Starting
    /// the binding revokes older read notifications and begins a fresh local epoch.
    pub fn attach(world: &mut World, input: Entity, baseline: u64) -> Result<(), AcceptanceError> {
        if world.get::<Self>(input).is_some() {
            return Err(AcceptanceError::Pending);
        }
        if world.get::<bevy::ui_widgets::TextInput>(input).is_none() {
            return Err(AcceptanceError::Stale);
        }
        if world
            .get::<crate::authored::components::UiEditable>(input)
            .is_none()
        {
            return Err(AcceptanceError::Stale);
        }
        let (acceptance, epoch) = {
            let mut query = world.query::<(&EditableText, &mut TextSession)>();
            let (text, mut session) = query
                .get_mut(world, input)
                .map_err(|_| AcceptanceError::Stale)?;
            let acceptance = TextAcceptance::new(input, text, &session, baseline)?;
            session
                .reset_history(text)
                .map_err(AcceptanceError::Native)?;
            (acceptance, session.generation())
        };
        world.entity_mut(input).insert(Self {
            acceptance,
            responding: false,
            epoch,
            request_session: None,
        });
        crate::authored::sync_editing(world, input);
        Ok(())
    }
    /// Accepted host value; the draft remains in native `EditableText` only.
    pub fn accepted(&self) -> &str {
        self.acceptance.accepted()
    }
    /// Whether the host owes a response. Used by the existing native-mode writer.
    pub fn awaiting_response(&self) -> bool {
        self.responding
    }
    /// Request an ordered native candidate without accepting or publishing it.
    /// Live hosts reuse the same request/acknowledgment protocol as Enter. Pending
    /// or stale bindings refuse without borrowing another field's authority.
    pub fn request(world: &mut World, input: Entity) -> Result<(), AcceptanceError> {
        if !writable(world, input) {
            return Err(AcceptanceError::Stale);
        }
        let gesture = world.get::<LiveTextField>(input).map(|live| live.gesture);
        with(world, input, |field, text, session| {
            field.acceptance.request(text, session)?;
            field.request_session = gesture;
            Ok(())
        })
    }
    /// Whether this retained binding has a draft, composition, queued native work,
    /// conflict or outstanding request which must be resolved before navigation.
    pub fn unsettled(world: &World, input: Entity) -> bool {
        let Some(field) = world.get::<Self>(input) else {
            return false;
        };
        let Some(text) = world.get::<EditableText>(input) else {
            return true;
        };
        let Some(session) = world.get::<TextSession>(input) else {
            return true;
        };
        field.acceptance.pending()
            || field.acceptance.stale()
            || session.pending()
            || !text.pending_edits.is_empty()
            || text.pending_paste.is_some()
            || text.is_composing()
            || text.value() != field.acceptance.accepted()
    }
    /// Verify field identity, native generation, settlement and baseline before
    /// host publication. This does not replace host revision/access checks.
    pub fn is_current(world: &World, input: Entity, request: &AcceptanceRequest) -> bool {
        let Some(field) = world.get::<Self>(input) else {
            return false;
        };
        let Some(text) = world.get::<EditableText>(input) else {
            return false;
        };
        let Some(session) = world.get::<TextSession>(input) else {
            return false;
        };
        writable(world, input) && field.acceptance.is_current(request, text, session)
    }
    /// Acknowledge a candidate and start a new local-history epoch, preserving the
    /// native caret. Call before changing a synchronous consumer value so a local
    /// failure cannot leave that consumer published without acknowledgment.
    pub fn accept(
        world: &mut World,
        input: Entity,
        request: &AcceptanceRequest,
        baseline: u64,
    ) -> Result<(), AcceptanceError> {
        if !Self::is_current(world, input, request) {
            return Err(AcceptanceError::Stale);
        }
        with(world, input, |field, text, session| {
            field.acceptance.accept(request, baseline, text, session)?;
            field.responding = false;
            Ok(())
        })
    }
    /// Reject without changing native draft, selection or local history. A bounded
    /// visible reason belongs to the surrounding host property presentation.
    pub fn reject(
        world: &mut World,
        input: Entity,
        request: &AcceptanceRequest,
    ) -> Result<(), AcceptanceError> {
        with(world, input, |field, text, session| {
            field.acceptance.reject(request, text, session)?;
            field.responding = false;
            Ok(())
        })
    }
    /// Refresh the accepted consumer value; active drafts survive and become
    /// stale. This is not a user edit and emits no acceptance request.
    pub fn refresh(
        world: &mut World,
        input: Entity,
        value: &str,
        baseline: u64,
    ) -> Result<(), AcceptanceError> {
        with(world, input, |field, text, session| {
            field.acceptance.refresh(value, baseline, text, session)?;
            field.responding &= field.acceptance.pending();
            Ok(())
        })
    }
    /// Explicit local cancellation. It revokes pending paste/candidates, never
    /// writes a reverse consumer transaction and never changes native focus.
    pub fn cancel(world: &mut World, input: Entity) -> Result<(), AcceptanceError> {
        with(world, input, |field, text, session| {
            field.acceptance.cancel(text, session)?;
            field.responding = false;
            Ok(())
        })?;
        world.trigger(TextFieldEvent {
            entity: input,
            outcome: TextFieldOutcome::Cancelled,
            edit_session: None,
        });
        Ok(())
    }
}
fn with<T>(
    world: &mut World,
    input: Entity,
    f: impl FnOnce(&mut TextField, &mut EditableText, &mut TextSession) -> Result<T, AcceptanceError>,
) -> Result<T, AcceptanceError> {
    let result = {
        let mut query = world.query::<(&mut TextField, &mut EditableText, &mut TextSession)>();
        let (mut field, mut text, mut session) = query
            .get_mut(world, input)
            .map_err(|_| AcceptanceError::Stale)?;
        f(&mut field, &mut text, &mut session)
    };
    crate::authored::sync_editing(world, input);
    result
}
fn writable(world: &World, input: Entity) -> bool {
    crate::native_input::eligible(world, input)
        && world
            .get::<FieldAccess>(input)
            .is_none_or(|access| !access.is_read_only())
        && world.get::<InteractionDisabled>(input).is_none()
}
pub(crate) fn install_text_fields(app: &mut App) {
    app.add_observer(outcome);
    app.add_systems(
        PostUpdate,
        live::poll.after(bevy::text::EditableTextSystems),
    );
}
fn outcome(event: On<InputEvent>, mut commands: Commands) {
    let event = event.clone();
    commands.queue(move |world: &mut World| {
        if world
            .get::<TextField>(event.entity)
            .is_none_or(|field| event.generation < field.epoch)
        {
            return;
        }
        let result = with(world, event.entity, |field, text, session| {
            let candidate = field.acceptance.capture(&event, text, session)?;
            if candidate.is_some() {
                field.responding = true;
            }
            Ok(candidate)
        });
        let notification = match result {
            Ok(Some(candidate)) => Some(TextFieldOutcome::Requested(candidate)),
            Err(error) => Some(TextFieldOutcome::Refused(error)),
            Ok(None) => match event.outcome {
                Outcome::Refused(error) => {
                    Some(TextFieldOutcome::Refused(AcceptanceError::Native(error)))
                }
                _ => None,
            },
        };
        if let Some(outcome) = notification {
            let edit_session = world
                .get::<TextField>(event.entity)
                .and_then(|field| field.request_session);
            world.trigger(TextFieldEvent {
                entity: event.entity,
                outcome,
                edit_session,
            });
        }
    });
}

// Called only after the shared window/modal/visibility gate, before Bevy's Escape
// observer can clear focus or collapse the original native selection.
pub(crate) fn field_keyboard(world: &mut World, input: Entity, event: &KeyboardInput) -> bool {
    if world.get::<TextField>(input).is_none() {
        return false;
    }
    if live::history(world, input, event) {
        return true;
    }
    if !matches!(event.logical_key, Key::Enter | Key::Escape | Key::Tab) {
        return false;
    }
    let composing = world.get::<EditableText>(input).is_some_and(|text| text.is_composing()
        || text.pending_edits.iter().any(|edit| matches!(edit, TextEdit::ImeSetCompose { .. } | TextEdit::ImeCommit { .. })))
        || world.get_resource::<Messages<Ime>>().is_some_and(|events| events.iter_current_update_messages().any(|ime|
            matches!(ime, Ime::Preedit { window, .. } | Ime::Commit { window, .. } if *window == event.window)));
    if composing {
        return true;
    }
    // Tab/blur settlement is intentionally not implemented by this Enter/Escape
    // increment; preserve native traversal, not an implicit commit.
    if event.logical_key == Key::Tab {
        return false;
    }
    if !event.state.is_pressed() || event.repeat {
        return true;
    }
    if !writable(world, input) {
        return true;
    }
    let result = if event.logical_key == Key::Escape {
        TextField::cancel(world, input)
    } else {
        TextField::request(world, input)
    };
    if let Some(mut live) = world.get_mut::<LiveTextField>(input) {
        live.finish();
    }
    if let Err(error) = result
        && error != AcceptanceError::Pending
    {
        world.trigger(TextFieldEvent {
            entity: input,
            outcome: TextFieldOutcome::Refused(error),
            edit_session: None,
        });
    }
    true
}
