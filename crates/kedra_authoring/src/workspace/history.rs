//! Stable source-operation metadata shared by every authoring client.
use super::*;

/// Monotonic operation identity within one working-set lineage, not a document
/// revision or persistent asset identity. Grouped edits keep their first identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HistoryId(pub u64);

/// Read-only description of an Undo/Redo operation. It grants no mutation rights.
pub struct HistoryInfo<'a> {
    /// Stable identity in this working-set lineage.
    pub id: HistoryId,
    /// User-facing action name, independent of the originating panel/script.
    pub label: &'a str,
    /// Exact affected document identities, including creations/deletions.
    pub documents: BTreeSet<DocumentId>,
}
pub(crate) fn validate_label(label: &str) -> Result<()> {
    if label.trim().is_empty()
        || label.len() > 512
        || label.chars().count() > 128
        || label.chars().any(char::is_control)
    {
        return Err(failure(
            "history label must contain 1..=128 characters without controls (at most 512 bytes)",
        ));
    }
    Ok(())
}
impl History {
    pub(super) fn merge(&mut self, latest: Self) {
        let known: BTreeSet<_> = self
            .before
            .keys()
            .chain(self.after.keys())
            .copied()
            .collect();
        for (id, value) in &latest.before {
            if !known.contains(id) {
                self.before.insert(*id, value.clone());
            }
            if !latest.after.contains_key(id) {
                self.after.remove(id);
            }
        }
        self.after.extend(latest.after);
    }
    fn info(&self) -> HistoryInfo<'_> {
        HistoryInfo {
            id: self.id,
            label: &self.label,
            documents: self
                .before
                .keys()
                .chain(self.after.keys())
                .filter(|id| {
                    self.before.get(id).map(|value| &value.document)
                        != self.after.get(id).map(|value| &value.document)
                })
                .copied()
                .collect(),
        }
    }
}
impl WorkingSet {
    /// Identities retained by source history, for pruning optional host attachments.
    /// Attachments never keep an otherwise evicted operation alive.
    pub fn history_ids(&self) -> BTreeSet<HistoryId> {
        self.undo
            .iter()
            .chain(&self.redo)
            .map(|entry| entry.id)
            .collect()
    }
    /// Describe the next source Undo without consuming it or invoking validation.
    pub fn undo_entry(&self) -> Option<HistoryInfo<'_>> {
        self.undo.last().map(History::info)
    }
    /// Describe the next source Redo without consuming it or invoking validation.
    pub fn redo_entry(&self) -> Option<HistoryInfo<'_>> {
        self.redo.last().map(History::info)
    }
    pub(crate) fn lineage(&self) -> std::sync::Arc<()> {
        self.group_scope.clone()
    }
}
