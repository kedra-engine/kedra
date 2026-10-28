//! Gesture-owned coalescing in the existing source history, never a second undo stack.
use super::*;
use std::sync::Arc;

/// An explicit live-edit history boundary owned by the caller.
///
/// Obtain from [`WorkingSet::edit_group`], keep for one editing gesture, and drop
/// on focus/target change or explicit completion. It grants no source authority:
/// every transaction still needs exact revisions and complete validation. Staged
/// clones of the same working set share its lineage; unrelated or deserialized
/// working sets do not. The capability itself is intentionally not serializable.
#[derive(Clone)]
pub struct EditGroup {
    scope: Arc<()>,
    clock: u64,
    depth: usize,
    started: bool,
}
impl WorkingSet {
    /// Capture the current history boundary without editing sources or consuming
    /// redo. No timers or automatic rebasing are involved. Intervening source edits,
    /// Undo/Redo, document admission and Save acknowledgement revoke this boundary.
    pub fn edit_group(&self) -> EditGroup {
        EditGroup {
            scope: self.group_scope.clone(),
            clock: self.clock,
            depth: self.undo.len(),
            started: false,
        }
    }

    /// Publish one fully validated live value through the ordinary transaction path.
    /// Consecutive accepted changes in `group` share one Undo entry, from the first
    /// baseline to the latest accepted value. Every changed value advances revisions;
    /// no-op values preserve redo and do not start a group. Validation, stale revision
    /// or group rejection leaves sources, history and the supplied group unchanged.
    ///
    /// The host must not reuse a group for another field/gesture. Drop it to complete
    /// a gesture; a new group starts a distinct Undo operation. Save remains separate.
    pub fn transact_grouped(
        &mut self,
        group: &mut EditGroup,
        expected: &BTreeMap<DocumentId, u64>,
        edit: impl FnOnce(&mut BTreeMap<DocumentId, PrefabDocument>) -> Result<()>,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        self.transact_grouped_named(group, "Edit sources", expected, edit, validate)
    }
    /// Named form of [`Self::transact_grouped`]. The first successful entry's
    /// identity and label survive later values in the same gesture.
    pub fn transact_grouped_named(
        &mut self,
        group: &mut EditGroup,
        label: &str,
        expected: &BTreeMap<DocumentId, u64>,
        edit: impl FnOnce(&mut BTreeMap<DocumentId, PrefabDocument>) -> Result<()>,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        if !Arc::ptr_eq(&self.group_scope, &group.scope)
            || self.clock != group.clock
            || self.undo.len() != group.depth
            || (group.started && self.undo.last().is_none_or(|entry| entry.mapping.is_some()))
        {
            return Err(failure("live edit history boundary is stale"));
        }
        let clock = self.clock;
        self.transact(label, expected, edit, validate, group.started)?;
        if self.clock != clock {
            group.started = true;
        }
        group.clock = self.clock;
        group.depth = self.undo.len();
        Ok(())
    }
}
