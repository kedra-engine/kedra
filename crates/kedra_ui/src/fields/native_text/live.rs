//! Opt-in live candidate capture. Native text and the consumer retain all authority.
use super::*;
use bevy::input_focus::InputFocus;

/// Opt into candidates after settled native text changes, without requiring Enter.
///
/// Attach alongside a managed [`TextField`]. The consumer still validates and
/// accepts/rejects every immutable request. A gesture ends on Enter, Escape or
/// focus loss; `TextFieldEvent::edit_session` pins the gesture at request time.
/// Rejection is not retried without another native edit. IME/preedit, pending
/// clipboard work, hidden/disabled fields and stale bindings never auto-publish.
/// This component owns neither another text buffer nor a source Undo stack.
#[derive(Component)]
pub struct LiveTextField {
    pub(super) gesture: u64,
    focused: bool,
    attempted: Option<u64>,
    exhausted: bool,
}
impl Default for LiveTextField {
    fn default() -> Self {
        Self {
            gesture: 1,
            focused: false,
            attempted: None,
            exhausted: false,
        }
    }
}
impl LiveTextField {
    pub(super) fn finish(&mut self) {
        if let Some(next) = self.gesture.checked_add(1) {
            self.gesture = next;
        } else {
            self.exhausted = true;
        }
    }
}
pub(super) fn history(world: &mut World, input: Entity, event: &KeyboardInput) -> bool {
    if world.get::<LiveTextField>(input).is_none() || TextField::unsettled(world, input) {
        return false;
    }
    let Some(action) = world
        .get_resource::<ButtonInput<Key>>()
        .and_then(|keys| kedra_text_input::history_shortcut(keys, event))
    else {
        return false;
    };
    let ime_batch = world.get_resource::<Messages<Ime>>().is_some_and(|events| {
        events.iter_current_update_messages().any(|ime| match ime {
            Ime::Preedit { window, .. }
            | Ime::Commit { window, .. }
            | Ime::Enabled { window }
            | Ime::Disabled { window } => *window == event.window,
        })
    });
    if ime_batch {
        return false;
    }
    if event.state.is_pressed() && !event.repeat {
        if let Some(mut live) = world.get_mut::<LiveTextField>(input) {
            live.finish();
        }
        world.trigger(TextFieldEvent {
            entity: input,
            outcome: TextFieldOutcome::HistoryRequested {
                redo: action == kedra_text_input::Command::Redo,
            },
            edit_session: None,
        });
    }
    true
}
pub(super) fn poll(world: &mut World) {
    let focused = world.get_resource::<InputFocus>().and_then(InputFocus::get);
    let inputs: Vec<_> = world
        .query_filtered::<Entity, (With<LiveTextField>, With<TextField>)>()
        .iter(world)
        .collect();
    for input in inputs {
        // Queue the last settled change with the gesture that owned the edit,
        // then close that gesture if this frame also moved focus elsewhere.
        capture(world, input);
        if let Some(mut live) = world.get_mut::<LiveTextField>(input) {
            let now = focused == Some(input);
            if live.focused && !now {
                live.finish();
            }
            live.focused = now;
        }
    }
}
fn capture(world: &mut World, input: Entity) {
    if !writable(world, input) {
        return;
    }
    let candidate = world
        .get::<LiveTextField>(input)
        .zip(world.get::<TextField>(input))
        .zip(
            world
                .get::<EditableText>(input)
                .zip(world.get::<TextSession>(input)),
        )
        .and_then(|((live, field), (text, session))| {
            (!live.exhausted
                && !field.acceptance.pending()
                && !field.acceptance.stale()
                && !session.pending()
                && text.pending_edits.is_empty()
                && text.pending_paste.is_none()
                && !text.is_composing()
                && text.value() != field.accepted()
                && live.attempted != Some(session.generation()))
            .then_some(session.generation())
        });
    let Some(generation) = candidate else {
        return;
    };
    if let Some(mut live) = world.get_mut::<LiveTextField>(input) {
        live.attempted = Some(generation);
    }
    if let Err(error) = TextField::request(world, input) {
        world.trigger(TextFieldEvent {
            entity: input,
            outcome: TextFieldOutcome::Refused(error),
            edit_session: None,
        });
    }
}
