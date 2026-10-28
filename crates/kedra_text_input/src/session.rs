use crate::history::History;
use bevy::{
    prelude::*,
    text::{EditableText, TextEdit},
};
use bevy_clipboard::ClipboardRead;
use std::collections::VecDeque;

/// An ordered control operation within the same queue as native text edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Undo one local text mutation, never a source transaction.
    Undo,
    /// Redo one local text mutation, never a source transaction.
    Redo,
    /// Capture settled text exactly at this boundary. This is not acceptance:
    /// the consumer must validate the request, generation and binding separately.
    Read(u64),
    /// End the current local-history epoch after a consumer acknowledgment.
    /// This neither accepts a value nor writes sources.
    ResetHistory,
}

pub(crate) enum Operation {
    Edit(TextEdit),
    Control(Command),
}

/// Machine-readable refusal, without clipboard/source contents in diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The single-line profile excludes controls and Unicode line separators.
    ForbiddenCharacter,
    /// Replacement would exceed the admitted scalar limit.
    ScalarLimit,
    /// Input is read-only/disabled, or lost access while work was queued.
    ReadOnly,
    /// A read/history request cannot cross active IME composition.
    Composing,
    /// Native clipboard I/O failed; selection and buffer are preserved.
    Clipboard,
    /// Attach only outside active composition or an existing asynchronous paste.
    Unsettled,
    /// The scalar ceiling must be in 1..=4096.
    InvalidLimit,
    /// An external buffer replacement invalidated queued work and local history.
    Replaced,
    /// The local generation cannot advance; token wrapping/reuse is forbidden.
    GenerationExhausted,
}

/// Result of an operation, not a source-edit receipt or permission to Save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A particular request observed exactly the preceding settled text.
    Read {
        /// Consumer-supplied request identity.
        request: u64,
        /// Exact committed text at that queue boundary, excluding IME preedit.
        text: String,
    },
    /// An operation was refused atomically.
    Refused(Refusal),
}

/// Consumer notification with local mutation generation. Binding identity and
/// source revision are consumer-owned; a late event must not rebind a retired view.
#[derive(EntityEvent, Clone, Debug)]
pub struct InputEvent {
    /// Original managed input entity.
    pub entity: Entity,
    /// Generation at the queued operation boundary, not notification-delivery time.
    pub generation: u64,
    /// Bounded operation outcome.
    pub outcome: Outcome,
}

/// Opt-in single-line native operation controller with 32 local undo entries.
///
/// Attach using [`Self::attach`] before interaction. Keep both this component and
/// `EditableText` alive across host moves. Native queues transfer into this owner;
/// asynchronous clipboard work never belongs to a replacement binding. Retirement
/// drops that work and history. No accepted value, authoring target or filesystem
/// authority is stored here. A history step is a native mutation, not a frame.
#[derive(Component)]
pub struct TextSession {
    pub(crate) limit: usize,
    pub(crate) queue: VecDeque<Operation>,
    pub(crate) paste: Option<ClipboardRead>,
    pub(crate) history: History,
    pub(crate) composition: Option<crate::history::State>,
    pub(crate) rejection: Option<Refusal>,
    pub(crate) generation: u64,
    pub(crate) known: String,
}
impl TextSession {
    /// Admit an existing native buffer without changing text, selection or queues.
    /// Invalid source text remains untouched. No normalization or truncation occurs.
    pub fn attach(text: &EditableText, limit: usize) -> Result<Self, Refusal> {
        if !(1..=4096).contains(&limit) {
            return Err(Refusal::InvalidLimit);
        }
        if text.is_composing() || text.pending_paste.is_some() {
            return Err(Refusal::Unsettled);
        }
        let value = text.value().to_string();
        validate(&value, limit)?;
        Ok(Self {
            limit,
            queue: VecDeque::new(),
            paste: None,
            history: History::default(),
            composition: None,
            rejection: None,
            generation: 0,
            known: value,
        })
    }
    /// Queue a control operation after all native edits already requested, and
    /// before later ones. Queueing does not read or mutate the native buffer.
    pub fn request(&mut self, text: &mut EditableText, command: Command) {
        self.capture(text);
        self.queue.push_back(Operation::Control(command));
    }
    /// Whether ordered work or asynchronous clipboard I/O remains owned here.
    /// Native queues and composition must also be checked before host settlement.
    pub fn pending(&self) -> bool {
        !self.queue.is_empty() || self.paste.is_some()
    }
    /// Local mutation generation; no relation to document revisions.
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Validate a supplied value against this session's exact single-line limit,
    /// without changing its buffer, queues, history or generation.
    pub fn validate_value(&self, value: &str) -> Result<(), Refusal> {
        validate(value, self.limit)
    }
    /// Begin an acknowledged local-history epoch without changing native selection.
    /// Unsettled input and generation exhaustion leave all state unchanged. This is
    /// not a consumer/source acceptance; its owner must validate the candidate first.
    pub fn reset_history(&mut self, text: &EditableText) -> Result<(), Refusal> {
        if self.pending()
            || !text.pending_edits.is_empty()
            || text.pending_paste.is_some()
            || text.is_composing()
            || self.composition.is_some()
        {
            return Err(Refusal::Unsettled);
        }
        self.advance()?;
        self.history.clear();
        self.rejection = None;
        Ok(())
    }
    /// Explicit binding retirement/replacement. Discard only local queued work and
    /// history, including native paste/IME work; leave committed buffer text intact.
    /// The consumer must settle or explicitly discard its draft before calling this.
    pub fn discard(&mut self, text: &mut EditableText) -> Result<(), Refusal> {
        self.advance()?;
        self.queue.clear();
        self.paste = None;
        self.history.clear();
        self.rejection = None;
        if let Some(state) = self.composition.take() {
            text.editor = state;
        }
        text.pending_edits.clear();
        text.pending_paste = None;
        text.queue_edit(TextEdit::clear_ime_compose());
        self.known = text.value().to_string();
        Ok(())
    }
    /// Explicitly load/cancel to a consumer-supplied string, not accept a source
    /// edit. Validate before touching buffer/selection; revoke earlier queued work,
    /// advance generation and place the native caret at the end. Errors preserve
    /// the previous local state, including its queued work.
    pub fn replace(&mut self, text: &mut EditableText, value: &str) -> Result<(), Refusal> {
        validate(value, self.limit)?;
        self.discard(text)?;
        text.pending_edits.clear();
        text.editor_mut().set_text(value);
        text.queue_edit(TextEdit::TextEnd(false));
        self.known = value.into();
        Ok(())
    }
    pub(crate) fn advance(&mut self) -> Result<(), Refusal> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(Refusal::GenerationExhausted)?;
        Ok(())
    }
    pub(crate) fn capture(&mut self, text: &mut EditableText) {
        self.queue
            .extend(text.pending_edits.drain(..).map(Operation::Edit));
    }
}

pub(crate) fn validate(text: &str, limit: usize) -> Result<(), Refusal> {
    if text
        .chars()
        .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
    {
        return Err(Refusal::ForbiddenCharacter);
    }
    if text.chars().count() > limit {
        return Err(Refusal::ScalarLimit);
    }
    Ok(())
}
