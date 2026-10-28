//! Acceptance authority over ordered native reads, independent of source ownership.
use bevy::{prelude::Entity, text::EditableText};
use kedra_text_input::{Command, InputEvent, Outcome, Refusal, TextSession};
use std::sync::Arc;

/// Immutable candidate for consumer validation. Clones retain the same request
/// identity; constructing another field with identical text never shares authority.
#[derive(Clone, Debug)]
pub struct AcceptanceRequest {
    owner: Arc<()>,
    sequence: u64,
    generation: u64,
    baseline: u64,
    value: String,
}
impl AcceptanceRequest {
    /// Exact settled candidate, without trimming or normalization.
    pub fn value(&self) -> &str {
        &self.value
    }
    /// Consumer-supplied baseline, to check before any authoritative publication.
    pub fn baseline(&self) -> u64 {
        self.baseline
    }
}

/// Protocol refusal. Errors never authorize a source transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptanceError {
    /// Another ordered read or acceptance already belongs to this field.
    Pending,
    /// Request, entity, generation or consumer baseline is no longer current.
    Stale,
    /// Native operation admission refused the request or replacement.
    Native(Refusal),
    /// Request identity cannot advance without wrapping.
    Exhausted,
}

enum Pending {
    Read(u64),
    Candidate(AcceptanceRequest),
}

/// One retained field binding's accepted value and acknowledgment protocol.
///
/// The native buffer remains exclusively in `EditableText`. The host owns domain
/// validation, access and publication; call `is_current` immediately before a
/// synchronous commit, then acknowledge its result. This is not an async document
/// capability. Retire this object when rebinding, even if the string is identical.
/// Keyboard, blur policy, pending-mode styling and focus are adapter responsibilities.
pub struct TextAcceptance {
    owner: Arc<()>,
    input: Entity,
    accepted: String,
    baseline: u64,
    sequence: u64,
    pending: Option<Pending>,
    stale: bool,
}
impl TextAcceptance {
    /// Bind an admitted, settled native input. No native text or history is changed.
    /// The caller retains the same `TextSession` and input for this binding's life.
    pub fn new(
        input: Entity,
        text: &EditableText,
        session: &TextSession,
        baseline: u64,
    ) -> Result<Self, AcceptanceError> {
        if !settled(text, session) {
            return Err(AcceptanceError::Native(Refusal::Unsettled));
        }
        session
            .validate_value(&text.value().to_string())
            .map_err(AcceptanceError::Native)?;
        Ok(Self {
            owner: Arc::new(()),
            input,
            accepted: text.value().to_string(),
            baseline,
            sequence: 0,
            pending: None,
            stale: false,
        })
    }
    /// Current consumer-acknowledged string, never a writable draft shadow.
    pub fn accepted(&self) -> &str {
        &self.accepted
    }
    /// Whether an ordered read or consumer response is outstanding.
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
    /// Whether an external consumer revision invalidated automatic acceptance.
    pub fn stale(&self) -> bool {
        self.stale
    }

    /// Queue a read after preceding native operations. Duplicate requests are
    /// refused until completion/cancellation; later operations are not its candidate.
    pub fn request(
        &mut self,
        text: &mut EditableText,
        session: &mut TextSession,
    ) -> Result<(), AcceptanceError> {
        if self.stale {
            return Err(AcceptanceError::Stale);
        }
        if self.pending() {
            return Err(AcceptanceError::Pending);
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(AcceptanceError::Exhausted)?;
        session.request(text, Command::Read(self.sequence));
        self.pending = Some(Pending::Read(self.sequence));
        Ok(())
    }

    /// Consume a matching native outcome. Unrelated/late notifications do nothing.
    /// A later edit invalidates capture rather than submitting the previous buffer.
    /// Native refusal releases the request and retains buffer, caret and history.
    pub fn capture(
        &mut self,
        event: &InputEvent,
        text: &EditableText,
        session: &TextSession,
    ) -> Result<Option<AcceptanceRequest>, AcceptanceError> {
        if event.entity != self.input {
            return Ok(None);
        }
        let Some(Pending::Read(sequence)) = self.pending else {
            return Ok(None);
        };
        let (request, value) = match &event.outcome {
            Outcome::Read { request, text } if *request == sequence => (*request, text),
            Outcome::Read { .. } => return Ok(None),
            // Refusals have no read identity: keep the read pending while an
            // earlier edit's notification arrives; only settle after queue drain.
            Outcome::Refused(error) => {
                if session.pending() {
                    return Ok(None);
                }
                self.pending = None;
                return Err(AcceptanceError::Native(*error));
            }
        };
        self.pending = None;
        if self.stale
            || event.generation != session.generation()
            || !settled(text, session)
            || text.value().to_string() != *value
        {
            return Err(AcceptanceError::Stale);
        }
        let candidate = AcceptanceRequest {
            owner: self.owner.clone(),
            sequence: request,
            generation: event.generation,
            baseline: self.baseline,
            value: value.clone(),
        };
        self.pending = Some(Pending::Candidate(candidate.clone()));
        Ok(Some(candidate))
    }

    /// Check identity, baseline and native state before consumer publication.
    /// Hosts must additionally enforce their own revision/access/lifetime guards.
    pub fn is_current(
        &self,
        request: &AcceptanceRequest,
        text: &EditableText,
        session: &TextSession,
    ) -> bool {
        !self.stale
            && Arc::ptr_eq(&self.owner, &request.owner)
            && self.baseline == request.baseline
            && matches!(&self.pending, Some(Pending::Candidate(current)) if current.sequence == request.sequence)
            && session.generation() == request.generation
            && settled(text, session)
            && text.value().to_string() == request.value
    }

    /// Acknowledge the exact candidate (including a no-op). Keep native caret and
    /// selection; start a new history epoch. Canonicalized replies are deliberately
    /// unsupported: consumers must explicitly refresh/cancel instead.
    pub fn accept(
        &mut self,
        request: &AcceptanceRequest,
        baseline: u64,
        text: &mut EditableText,
        session: &mut TextSession,
    ) -> Result<(), AcceptanceError> {
        if !self.is_current(request, text, session) {
            return Err(AcceptanceError::Stale);
        }
        session
            .reset_history(text)
            .map_err(AcceptanceError::Native)?;
        self.accepted.clone_from(&request.value);
        self.baseline = baseline;
        self.pending = None;
        Ok(())
    }
    /// Reject without modifying native text, selection, queues or local history.
    /// The host owns the bounded, user-facing diagnostic associated with rejection.
    pub fn reject(
        &mut self,
        request: &AcceptanceRequest,
        text: &EditableText,
        session: &TextSession,
    ) -> Result<(), AcceptanceError> {
        if !self.is_current(request, text, session) {
            return Err(AcceptanceError::Stale);
        }
        self.pending = None;
        Ok(())
    }
    /// Observe a new accepted consumer value. Idle fields refresh; drafts survive
    /// and become stale even when the new string happens to equal the old string.
    /// Invalid replacements leave all state unchanged under native admission.
    pub fn refresh(
        &mut self,
        value: &str,
        baseline: u64,
        text: &mut EditableText,
        session: &mut TextSession,
    ) -> Result<(), AcceptanceError> {
        if baseline == self.baseline && value == self.accepted {
            return Ok(());
        }
        session
            .validate_value(value)
            .map_err(AcceptanceError::Native)?;
        let dirty =
            self.pending() || !settled(text, session) || text.value().to_string() != self.accepted;
        if !dirty {
            if value == self.accepted {
                session
                    .reset_history(text)
                    .map_err(AcceptanceError::Native)?;
            } else {
                session
                    .replace(text, value)
                    .map_err(AcceptanceError::Native)?;
            }
        }
        self.accepted = value.into();
        self.baseline = baseline;
        self.pending = None;
        self.stale = dirty;
        Ok(())
    }
    /// Restore the latest consumer value, not an obsolete baseline. Revokes reads,
    /// paste and local history without writing a reverse consumer transaction.
    pub fn cancel(
        &mut self,
        text: &mut EditableText,
        session: &mut TextSession,
    ) -> Result<(), AcceptanceError> {
        session
            .replace(text, &self.accepted)
            .map_err(AcceptanceError::Native)?;
        self.pending = None;
        self.stale = false;
        Ok(())
    }
}
fn settled(text: &EditableText, session: &TextSession) -> bool {
    !session.pending()
        && text.pending_edits.is_empty()
        && text.pending_paste.is_none()
        && !text.is_composing()
}
