// Clipboard sequencing and cursor reveal adapted from Bevy 0.20.0
// crates/bevy_text/src/{editing,text_edit}.rs. Copyright Bevy contributors.
// SPDX-License-Identifier: MIT OR Apache-2.0. See ../README.md and ../LICENSE-MIT.
use crate::{
    Command, InputEvent, Outcome, Refusal, TextSession, history, mutation, session::Operation,
};
use bevy::{
    prelude::*,
    text::{
        EditableText, EditableTextGeneration, FontCx, LayoutCx, TextEdit, TextLayout,
        TextReadWriteMode,
    },
};
use bevy_clipboard::Clipboard;

type Inputs<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut TextSession,
        &'static mut EditableText,
        &'static TextLayout,
        &'static TextReadWriteMode,
        &'static EditableTextGeneration,
        Has<bevy::ui::InteractionDisabled>,
    ),
>;

pub(crate) fn apply(
    mut inputs: Inputs<'_, '_>,
    mut font: ResMut<FontCx>,
    mut layout: ResMut<LayoutCx>,
    mut clipboard: ResMut<Clipboard>,
    mut commands: Commands,
) {
    for (entity, mut session, mut text, text_layout, mode, rendered, disabled) in &mut inputs {
        session.capture(&mut text);
        if !session.pending() {
            continue;
        }
        let mut context = Context {
            font: &mut font,
            layout: &mut layout,
            clipboard: &mut clipboard,
            entity,
            commands: &mut commands,
            text_layout,
            editable: *mode == TextReadWriteMode::Editable && !disabled,
        };
        // A programmatic replacement is a new local-history epoch. Native preedit
        // can temporarily remove the selected committed text, so it is not an echo.
        if (session.composition.is_none() && session.known != text.value().to_string())
            || (session.composition.is_some() && !text.is_composing())
        {
            session.history.clear();
            session.composition = None;
            session.queue.clear();
            session.paste = None;
            session.rejection = None;
            session.known = text.value().to_string();
            let error = session.advance().err().unwrap_or(Refusal::Replaced);
            session.rejection = Some(error);
            context.refuse(&session, error);
            continue;
        }
        if session.paste.is_some() && !context.paste(&mut session, &mut text) {
            continue;
        }
        while let Some(operation) = session.queue.pop_front() {
            match operation {
                Operation::Control(command) => context.control(&mut session, &mut text, command),
                Operation::Edit(TextEdit::Paste) => {
                    if !context.editable {
                        context.refuse(&session, Refusal::ReadOnly);
                        continue;
                    }
                    session.paste = Some(context.clipboard.fetch_text());
                    if !context.paste(&mut session, &mut text) {
                        break;
                    }
                }
                Operation::Edit(edit) => context.edit(&mut session, &mut text, edit),
            }
        }
        // A restored native snapshot can reuse an old Parley layout generation.
        // Resolve its layout, then invalidate once more if that number aliases
        // the last rendered state. Otherwise correct buffer text could retain
        // obsolete glyphs/selection geometry in Bevy's TextLayoutInfo.
        context.reveal(&mut text);
        if text.editor.generation() == **rendered {
            text.editor.edit_styles();
            context.reveal(&mut text);
        }
    }
}

#[cfg(test)]
mod tests;

struct Context<'a, 'w, 's> {
    font: &'a mut FontCx,
    layout: &'a mut LayoutCx,
    clipboard: &'a mut Clipboard,
    entity: Entity,
    commands: &'a mut Commands<'w, 's>,
    text_layout: &'a TextLayout,
    editable: bool,
}
impl Context<'_, '_, '_> {
    fn notify(&mut self, session: &TextSession, outcome: Outcome) {
        self.commands.trigger(InputEvent {
            entity: self.entity,
            generation: session.generation,
            outcome,
        });
    }
    fn refuse(&mut self, session: &TextSession, refusal: Refusal) {
        self.notify(session, Outcome::Refused(refusal));
    }
    fn changed(
        &mut self,
        session: &mut TextSession,
        text: &mut EditableText,
        before: history::State,
    ) {
        let value = text.value().to_string();
        if value != before.text().to_string() {
            if let Err(error) = session.advance() {
                history::restore(text, &before, self.text_layout);
                session.rejection = Some(error);
                self.refuse(session, error);
                return;
            }
            session.history.record(before, text.editor.clone());
        }
        session.rejection = None;
        session.known = value;
    }
    fn reveal(&mut self, text: &mut EditableText) {
        let mut driver = text.editor.driver(self.font, &mut self.layout.0);
        driver.refresh_layout();
        if let Some(rect) = bevy::text::cursor_reveal_rect(&mut driver) {
            let layout = driver.layout();
            text.viewport.reveal_caret(
                rect,
                Vec2::new(layout.full_width(), layout.height()),
                text.cursor_margin,
                layout
                    .lines()
                    .map(|line| bevy::text::TextLineYBounds::from_line(&line)),
            );
        }
    }
    fn paste(&mut self, session: &mut TextSession, text: &mut EditableText) -> bool {
        if !self.editable {
            session.paste = None;
            self.refuse(session, Refusal::ReadOnly);
            return true;
        }
        let Some(result) = session.paste.as_mut().and_then(|read| read.poll_result()) else {
            return false;
        };
        session.paste = None;
        match result {
            Ok(value) => self.insert(session, text, &value),
            Err(_) => {
                session.rejection = Some(Refusal::Clipboard);
                self.refuse(session, Refusal::Clipboard);
            }
        }
        true
    }
    fn insert(&mut self, session: &mut TextSession, text: &mut EditableText, value: &str) {
        if text.is_composing() {
            self.refuse(session, Refusal::Composing);
            return;
        }
        let before = text.editor.clone();
        let result = mutation::insert(
            &mut text.editor.driver(self.font, &mut self.layout.0),
            value,
            session.limit,
        );
        match result {
            Ok(()) => {
                self.changed(session, text, before);
                self.reveal(text);
            }
            Err(error) => {
                session.rejection = Some(error);
                self.refuse(session, error);
            }
        }
    }
    fn control(&mut self, session: &mut TextSession, text: &mut EditableText, command: Command) {
        if text.is_composing() || session.composition.is_some() {
            self.refuse(session, Refusal::Composing);
            return;
        }
        if !self.editable {
            self.refuse(session, Refusal::ReadOnly);
            return;
        }
        match command {
            Command::Read(request) => {
                if let Some(error) = session.rejection {
                    self.refuse(session, error);
                } else {
                    self.notify(
                        session,
                        Outcome::Read {
                            request,
                            text: text.value().to_string(),
                        },
                    );
                }
            }
            Command::ResetHistory => {
                if let Err(error) = session.advance() {
                    self.refuse(session, error);
                    return;
                }
                session.history.clear();
                session.rejection = None;
            }
            Command::Undo | Command::Redo => {
                if let Err(error) = session.advance() {
                    self.refuse(session, error);
                    return;
                }
                if command == Command::Undo {
                    session.history.undo(text, self.text_layout);
                } else {
                    session.history.redo(text, self.text_layout);
                }
                session.known = text.value().to_string();
                session.rejection = None;
                self.reveal(text);
            }
        }
    }
    fn edit(&mut self, session: &mut TextSession, text: &mut EditableText, edit: TextEdit) {
        if edit.is_destructive() && !self.editable {
            // Native focus loss must be able to cancel a preedit after access loss.
            if matches!(&edit, TextEdit::ImeSetCompose { value, .. } if value.is_empty()) {
                if let Some(before) = session.composition.take() {
                    history::restore(text, &before, self.text_layout);
                }
            } else {
                self.refuse(session, Refusal::ReadOnly);
            }
            return;
        }
        match edit {
            TextEdit::Insert(value) => self.insert(session, text, &value),
            TextEdit::Copy | TextEdit::Cut => {
                let before = text.editor.clone();
                if let Some(value) = text.editor.selected_text() {
                    if self.clipboard.set_text(value).is_err() {
                        self.refuse(session, Refusal::Clipboard);
                        return;
                    }
                    if edit == TextEdit::Cut {
                        text.editor
                            .driver(self.font, &mut self.layout.0)
                            .delete_selection();
                    }
                }
                if edit == TextEdit::Cut {
                    self.changed(session, text, before);
                }
                self.reveal(text);
            }
            TextEdit::ImeSetCompose { value, cursor } => {
                if value.is_empty() {
                    if let Some(before) = session.composition.take() {
                        history::restore(text, &before, self.text_layout);
                    }
                    self.reveal(text);
                    return;
                }
                let baseline = session.composition.as_ref().unwrap_or(&text.editor);
                if let Err(error) = mutation::check(baseline, &value, session.limit) {
                    session.rejection = Some(error);
                    self.refuse(session, error);
                    return;
                }
                if cursor.is_some_and(|c| {
                    c.anchor > value.len()
                        || c.focus > value.len()
                        || !value.is_char_boundary(c.anchor)
                        || !value.is_char_boundary(c.focus)
                }) {
                    self.refuse(session, Refusal::ForbiddenCharacter);
                    return;
                }
                if session.composition.is_none() {
                    session.composition = Some(text.editor.clone());
                }
                text.editor
                    .driver(self.font, &mut self.layout.0)
                    .set_compose(&value, cursor.map(|c| (c.anchor, c.focus)));
                self.reveal(text);
            }
            TextEdit::ImeCommit { value } => {
                // Restore the native baseline before validation so rejected commits
                // preserve the original selection and text, including replacement.
                let baseline = session.composition.take();
                if let Some(before) = &baseline {
                    history::restore(text, before, self.text_layout);
                }
                self.insert(session, text, &value);
            }
            other => {
                if text.is_composing() {
                    self.refuse(session, Refusal::Composing);
                    return;
                }
                let before = text.editor.clone();
                let destructive = other.is_destructive();
                other.apply(
                    &mut text.editor.driver(self.font, &mut self.layout.0),
                    &mut text.viewport,
                    text.cursor_margin,
                    self.clipboard,
                    Some(session.limit),
                    |_| true,
                );
                if destructive {
                    self.changed(session, text, before);
                }
            }
        }
    }
}
