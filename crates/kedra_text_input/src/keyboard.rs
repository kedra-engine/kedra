// Shortcut matching adapted from Bevy 0.20.0 bevy_ui_widgets/src/text_input.rs.
// Copyright Bevy contributors. SPDX-License-Identifier: MIT OR Apache-2.0.
use crate::{Command, TextSession};
use bevy::{
    input::keyboard::{Key, KeyboardInput},
    prelude::*,
    text::{EditableText, TextEdit},
    window::Ime,
};

/// Consume a managed input's local-history shortcut before native focused-input
/// dispatch. Call only after the host's window/focus/modal eligibility checks.
/// Returning true includes empty history, releases and IME ownership: do not
/// dispatch the same event again to a widget or document command handler.
/// Other inputs and ordinary editing keep the existing native event path.
pub fn handle_keyboard(world: &mut World, target: Entity, event: &KeyboardInput) -> bool {
    if world.get::<TextSession>(target).is_none() {
        return false;
    }
    let Some(keys) = world.get_resource::<ButtonInput<Key>>() else {
        return false;
    };
    let action = history_shortcut(keys, event);
    if let Some(action) = action {
        let ime_batch = world.get_resource::<Messages<Ime>>().is_some_and(|events| {
            events.iter_current_update_messages().any(|ime| match ime {
                Ime::Preedit { window, .. }
                | Ime::Commit { window, .. }
                | Ime::Enabled { window }
                | Ime::Disabled { window } => *window == event.window,
            })
        });
        let mut query = world.query::<(&mut TextSession, &mut EditableText)>();
        if let Ok((mut session, mut text)) = query.get_mut(world, target) {
            let composing = text.is_composing()
                || text.pending_edits.iter().any(|edit| {
                    matches!(
                        edit,
                        TextEdit::ImeSetCompose { .. } | TextEdit::ImeCommit { .. }
                    )
                });
            if event.state.is_pressed() && !event.repeat && !ime_batch && !composing {
                session.request(&mut text, action);
            }
        }
        return true;
    }
    false
}

/// Match the platform's Undo/Redo shortcut without queuing any native operation.
/// Hosts may route an already settled live field to consumer history instead.
/// Matching grants no focus/window/IME eligibility; the caller must guard those.
pub fn history_shortcut(keys: &ButtonInput<Key>, event: &KeyboardInput) -> Option<Command> {
    let command = if cfg!(target_os = "macos") {
        Key::Super
    } else {
        Key::Control
    };
    if !keys.pressed(command) || keys.pressed(Key::Alt) {
        return None;
    }
    let matches = |letter: &str, code| match &event.logical_key {
        Key::Character(value) if value.is_ascii() => value.eq_ignore_ascii_case(letter),
        Key::Character(_) => event.key_code == code,
        _ => false,
    };
    if matches("z", KeyCode::KeyZ) {
        Some(if keys.pressed(Key::Shift) {
            Command::Redo
        } else {
            Command::Undo
        })
    } else if !cfg!(target_os = "macos") && matches("y", KeyCode::KeyY) {
        Some(Command::Redo)
    } else {
        None
    }
}
