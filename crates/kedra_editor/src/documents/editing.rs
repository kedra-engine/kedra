//! Staged typed property/history changes. Publication has no fallible native step.
use super::*;
use bevy::reflect::TypeRegistry;
use kedra_authoring::{
    properties::{PropertyEdit, PropertyTarget},
    workspace::{EditGroup, WorkingSet},
};
use kedra_scene::instance::CompiledPrefab;
use std::sync::Arc;

/// Isolated, fully validated working state. The host acknowledges native input
/// before publishing it synchronously; dropping it leaves documents/history intact.
pub(crate) struct PreparedChange {
    pub(super) working: WorkingSet,
    pub(super) compiled: BTreeMap<DocumentId, Arc<CompiledPrefab>>,
    pub(super) expected: BTreeMap<DocumentId, u64>,
    pub(super) reset_scope: bool,
}

/// Exact source destination and revision guard for one validated property batch.
pub(crate) struct PropertyChange<'a> {
    pub target: &'a inspection::Target,
    pub expected: &'a BTreeMap<DocumentId, u64>,
    pub component: &'a str,
    pub edits: &'a [PropertyEdit<'a>],
}

#[cfg(test)]
mod tests;
impl Documents {
    pub(crate) fn history_id(&self, redo: bool) -> Option<kedra_authoring::workspace::HistoryId> {
        let working = &self.scope.as_ref()?.working;
        if redo {
            working.redo_entry()
        } else {
            working.undo_entry()
        }
        .map(|entry| entry.id)
    }
    pub(crate) fn history_ids(
        &self,
    ) -> std::collections::BTreeSet<kedra_authoring::workspace::HistoryId> {
        self.scope
            .as_ref()
            .map(|scope| scope.working.history_ids())
            .unwrap_or_default()
    }
    pub(crate) fn revisions(&self) -> BTreeMap<DocumentId, u64> {
        self.scope
            .as_ref()
            .map(|scope| {
                scope
                    .working
                    .documents()
                    .iter()
                    .map(|(id, doc)| (*id, doc.revision))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub(crate) fn prepare_property(
        &self,
        target: &inspection::Target,
        expected: &BTreeMap<DocumentId, u64>,
        component: &str,
        edits: &[PropertyEdit<'_>],
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<PreparedChange, String> {
        self.prepare_property_change(
            PropertyChange {
                target,
                expected,
                component,
                edits,
            },
            None,
            types,
            policies,
        )
    }
    pub(crate) fn edit_group(&self) -> Result<EditGroup, String> {
        Ok(self
            .scope
            .as_ref()
            .ok_or("Source workspace unavailable")?
            .working
            .edit_group())
    }
    /// A caller stages a cloned gesture and publishes it only after native ack.
    pub(crate) fn prepare_property_change(
        &self,
        request: PropertyChange<'_>,
        group: Option<&mut EditGroup>,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<PreparedChange, String> {
        let PropertyChange {
            target,
            expected,
            component,
            edits,
        } = request;
        self.inspect(target)?;
        if &self.revisions() != expected {
            return Err("Source revision changed. Press Escape before editing again.".into());
        }
        let scope = self.scope.as_ref().ok_or("Source workspace unavailable")?;
        let prepared = kedra_authoring::changes::property(
            scope,
            kedra_authoring::changes::PropertyRequest {
                snapshot: self
                    .compiled
                    .get(&target.document)
                    .ok_or("Source compilation unavailable")?
                    .clone(),
                target: PropertyTarget {
                    document: target.document,
                    node: target.node.clone(),
                    component: component.into(),
                },
                expected,
                edits,
            },
            group,
            types,
            policies,
        )?;
        Ok(PreparedChange {
            working: prepared.working,
            compiled: prepared.compiled,
            expected: expected.clone(),
            reset_scope: false,
        })
    }
    pub(crate) fn prepare_history(
        &self,
        redo: bool,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<PreparedChange, String> {
        let scope = self.scope.as_ref().ok_or("No source workspace is open")?;
        let prepared = kedra_authoring::changes::history(scope, redo, types, policies)?;
        Ok(PreparedChange {
            working: prepared.working,
            compiled: prepared.compiled,
            expected: self.revisions(),
            reset_scope: false,
        })
    }
    pub(crate) fn can_history(&self, redo: bool) -> bool {
        self.scope.as_ref().is_some_and(|scope| {
            if redo {
                scope.working.can_redo()
            } else {
                scope.working.can_undo()
            }
        })
    }
    pub(crate) fn prepare_discard(
        &self,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<PreparedChange, String> {
        let scope = self.scope.as_ref().ok_or("Source workspace unavailable")?;
        if scope.pending().is_some() {
            return Err("Resolve the pending Save before discarding sources.".into());
        }
        let (baseline, compiled) = source::admit(scope.source.clone(), types, policies)?;
        Ok(PreparedChange {
            working: baseline.working,
            compiled,
            expected: self.revisions(),
            reset_scope: true,
        })
    }
    /// Check before native acknowledgment; no systems run between this and publish.
    pub(crate) fn change_current(&self, change: &PreparedChange) -> bool {
        self.revisions() == change.expected
    }
    pub(crate) fn publish_change(&mut self, change: PreparedChange) {
        let Some(scope) = &mut self.scope else {
            return;
        };
        scope.working = change.working;
        if change.reset_scope {
            self.scope_identity = default();
        }
        self.compiled = change.compiled;
        self.compiled_revisions = self.revisions();
        if change.reset_scope {
            self.renew_selection_after_discard();
        } else {
            self.reconcile_selection();
        }
        for (id, opened) in &mut self.opened {
            if let Some(compiled) = self.compiled.get(id) {
                opened.graph = Arc::new(compiled.graph().clone());
            }
        }
    }
}
