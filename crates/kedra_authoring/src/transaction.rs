//! Explicit isolated authoring transactions for tools and authorized script hosts.
//! The candidate is not a second working source/history. Only commit publishes to
//! the original WorkingSet; dropped/cancelled candidates have no external effects.
use crate::{failure, workspace::WorkingSet};
use kedra_scene::{
    Result,
    composition::Catalog,
    document::{DocumentId, PrefabDocument},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// One revision/lineage-pinned, scoped source candidate. Not serializable or cloneable.
/// The host grants document identities and must provide full typed catalog validation
/// at commit. Mutating the candidate never runs gameplay, loads assets or writes files.
pub struct EditTransaction {
    label: String,
    lineage: Arc<()>,
    expected: BTreeMap<DocumentId, u64>,
    allowed: BTreeSet<DocumentId>,
    candidate: Catalog,
    closed: bool,
}
impl EditTransaction {
    /// Begin over explicitly granted existing documents (1..=128). Capture the
    /// complete working revision set, including composition dependencies. No rebase
    /// occurs: changes/admission/Save in this lineage invalidate outstanding work.
    pub fn begin(working: &WorkingSet, label: &str, allowed: BTreeSet<DocumentId>) -> Result<Self> {
        crate::workspace::validate_label(label)?;
        if allowed.is_empty()
            || allowed.len() > 128
            || allowed
                .iter()
                .any(|id| !working.documents().contains_key(id))
        {
            return Err(failure(
                "edit transaction requires 1..=128 admitted document grants",
            ));
        }
        Ok(Self {
            label: label.into(),
            lineage: working.lineage(),
            expected: working
                .documents()
                .iter()
                .map(|(id, document)| (*id, document.revision))
                .collect(),
            candidate: working.catalog(),
            allowed,
            closed: false,
        })
    }
    /// Current isolated catalog for exact typed compilation. Immutable references
    /// cannot mutate sources; access is rejected after successful commit/cancel.
    pub fn candidate(&self) -> Result<&Catalog> {
        self.open()?;
        Ok(&self.candidate)
    }
    /// Whether commit has succeeded or explicit cancellation has closed this lease.
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    /// Stage a node-name override in the explicitly granted receiving source.
    /// Validator failure preserves the candidate as well as published sources.
    pub fn rename(
        &mut self,
        id: DocumentId,
        node: kedra_scene::document::NodeAddress,
        name: String,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        self.operation(
            id,
            kedra_scene::document::Operation::RenameNode { node, name },
            validate,
        )
    }
    /// Stage one native scene operation in the receiving source. Composition and
    /// caller validation run before installation; errors leave the candidate intact.
    /// Instance addresses remain scoped to the receiver, never applied upstream.
    pub fn operation(
        &mut self,
        id: DocumentId,
        operation: kedra_scene::document::Operation,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        self.open()?;
        if !self.allowed.contains(&id) {
            return Err(failure("document is outside the edit grant"));
        }
        let mut candidate = self.candidate.clone();
        let document = candidate
            .documents
            .get_mut(&id)
            .ok_or_else(|| failure("edit document unavailable"))?;
        crate::overrides::upsert(&mut document.overrides, operation);
        for id in candidate.documents.keys() {
            candidate.resolve(*id)?;
        }
        validate(&candidate)?;
        self.stage(candidate)
    }
    /// Reject foreign/stale work before expensive host-side staging. Caller must
    /// still invoke commit with complete validation before publication.
    pub fn check(&self, working: &WorkingSet) -> Result<()> {
        self.open()?;
        if !Arc::ptr_eq(&self.lineage, &working.lineage())
            || self.expected
                != working
                    .documents()
                    .iter()
                    .map(|(id, document)| (*id, document.revision))
                    .collect()
        {
            return Err(failure(
                "edit transaction source scope or revisions changed",
            ));
        }
        Ok(())
    }
    /// Stage one document mutation. Failure preserves this candidate; identity
    /// changes and writes outside the explicit grant are refused. Coupled domain
    /// constraints are validated on the complete final catalog at commit.
    pub fn edit(
        &mut self,
        id: DocumentId,
        edit: impl FnOnce(&mut PrefabDocument) -> Result<()>,
    ) -> Result<()> {
        self.open()?;
        if !self.allowed.contains(&id) {
            return Err(failure("document is outside the edit grant"));
        }
        let mut candidate = self
            .candidate
            .documents
            .get(&id)
            .ok_or_else(|| failure("edit document unavailable"))?
            .clone();
        edit(&mut candidate)?;
        if candidate.id != id {
            return Err(failure("editing cannot change a document identity"));
        }
        self.candidate.documents.insert(id, candidate);
        Ok(())
    }
    /// Install a domain-validated candidate, for example a native PropertySession
    /// result. It must retain the exact inventory and all ungranted source content.
    /// This stages values only and does not consume any source history.
    pub fn stage(&mut self, catalog: Catalog) -> Result<()> {
        self.open()?;
        if self.candidate.documents.keys().ne(catalog.documents.keys())
            || catalog.documents.iter().any(|(id, value)| {
                value.id != *id
                    || (!self.allowed.contains(id)
                        && self.candidate.documents.get(id) != Some(value))
            })
        {
            return Err(failure(
                "candidate changes source inventory, identity or an ungranted document",
            ));
        }
        self.candidate = catalog;
        Ok(())
    }
    /// Revalidate and publish the entire candidate as one named operation in the
    /// existing working-set history. Errors preserve working state and this lease;
    /// successful no-op closes it without clearing Redo. Never acknowledges Save.
    pub fn commit(
        &mut self,
        working: &mut WorkingSet,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        self.check(working)?;
        working.transact_named(
            &self.label,
            &self.expected,
            |documents| {
                *documents = self.candidate.documents.clone();
                Ok(())
            },
            validate,
        )?;
        self.closed = true;
        Ok(())
    }
    /// Explicitly discard this candidate without changing working sources/history.
    /// Further staging/commit is refused. Dropping an open lease has the same effects.
    pub fn cancel(&mut self) {
        self.closed = true;
    }
    fn open(&self) -> Result<()> {
        if self.closed {
            return Err(failure("edit transaction is closed"));
        }
        Ok(())
    }
}
