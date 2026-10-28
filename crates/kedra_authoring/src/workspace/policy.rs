//! Bounded changed-document history; source authority never depends on a panel cache.
use super::*;
/// Limits for retained Undo/Redo. Payload is encoded changed-document bytes, not a
/// claim about allocator overhead. One oversize operation refuses before publication.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HistoryPolicy {
    /// Maximum retained operations. The default is 256.
    pub max_entries: usize,
    /// Maximum encoded source payload across retained entries. Default: 64 MiB.
    pub max_payload_bytes: usize,
}
impl Default for HistoryPolicy {
    fn default() -> Self {
        Self {
            max_entries: 256,
            max_payload_bytes: 64 * 1024 * 1024,
        }
    }
}
impl History {
    fn payload_bytes(&self) -> usize {
        self.before
            .values()
            .chain(self.after.values())
            .fold(self.label.len(), |sum, doc| {
                sum.saturating_add(
                    doc.document
                        .serialize()
                        .map_or(usize::MAX, |text| text.len()),
                )
            })
    }
}
impl WorkingSet {
    /// Configure bounded history and evict oldest optional entries immediately.
    /// Does not change working sources, saved checkpoints or the revision clock.
    /// Policy needs 1..=4096 entries and 64 KiB..=1 GiB encoded payload.
    pub fn set_history_policy(&mut self, policy: HistoryPolicy) -> Result<()> {
        if !(1..=4096).contains(&policy.max_entries)
            || !(65_536..=1024 * 1024 * 1024).contains(&policy.max_payload_bytes)
        {
            return Err(failure("invalid authoring history policy"));
        }
        self.policy = policy;
        self.group_scope = std::sync::Arc::new(());
        self.trim_history();
        Ok(())
    }
    pub(super) fn check_history(&self, entry: &History) -> Result<()> {
        if entry.payload_bytes() > self.policy.max_payload_bytes {
            return Err(failure(
                "source operation exceeds the configured history payload budget",
            ));
        }
        Ok(())
    }
    pub(super) fn trim_history(&mut self) {
        while self.undo.len() + self.redo.len() > self.policy.max_entries
            || self
                .undo
                .iter()
                .chain(&self.redo)
                .fold(0usize, |sum, entry| {
                    sum.saturating_add(entry.payload_bytes())
                })
                > self.policy.max_payload_bytes
        {
            // Undo's front is the oldest past; Redo's front is the most distant future.
            if !self.undo.is_empty() {
                self.undo.remove(0);
            } else if !self.redo.is_empty() {
                self.redo.remove(0);
            } else {
                break;
            }
        }
    }
}
