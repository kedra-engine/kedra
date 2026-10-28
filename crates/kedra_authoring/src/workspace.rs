//! One mutable working document per identity. Commands are atomic across documents.
use crate::failure;
use kedra_scene::{Result, composition::Catalog, document::*};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
mod groups;
pub use groups::EditGroup;
mod history;
pub(crate) use history::validate_label;
pub use history::{HistoryId, HistoryInfo};
mod policy;
pub use policy::HistoryPolicy;

#[derive(Clone, Serialize, Deserialize)]
/// One source working copy and its monotonic current/saved revisions.
pub struct WorkingDocument {
    /// Mutable source authority, never a captured gameplay world.
    pub document: PrefabDocument,
    /// Current successful edit revision.
    pub revision: u64,
    /// Last revision acknowledged by the save protocol.
    pub saved_revision: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct History {
    id: HistoryId,
    label: String,
    before: BTreeMap<DocumentId, WorkingDocument>,
    after: BTreeMap<DocumentId, WorkingDocument>,
    pub(crate) mapping: Option<(DocumentId, BTreeMap<NodeAddress, NodeAddress>)>,
}
#[derive(Default, Clone, Serialize, Deserialize)]
/// Atomic multi-document edits and their revision-guarded history.
pub struct WorkingSet {
    #[serde(skip)]
    group_scope: std::sync::Arc<()>,
    pub(crate) documents: BTreeMap<DocumentId, WorkingDocument>,
    pub(crate) saved: BTreeMap<DocumentId, PrefabDocument>,
    policy: HistoryPolicy,
    pub(crate) undo: Vec<History>,
    redo: Vec<History>,
    coupled: Vec<BTreeSet<DocumentId>>,
    pub(crate) clock: u64,
    /// Most recent explicit unpack/history source mapping for selection restoration.
    pub last_mapping: Option<(DocumentId, BTreeMap<NodeAddress, NodeAddress>)>,
}
impl WorkingSet {
    pub(crate) fn check_snapshot(
        &self,
        snapshot: &kedra_scene::instance::CompiledPrefab,
    ) -> Result<()> {
        if self.catalog().resolve(snapshot.graph().document)? != *snapshot.graph() {
            return Err(failure(
                "compiled snapshot is stale for the current composition sources",
            ));
        }
        Ok(())
    }
    /// Read working revisions without bypassing transaction guards.
    pub fn documents(&self) -> &BTreeMap<DocumentId, WorkingDocument> {
        &self.documents
    }
    /// Whether an undo candidate exists; undo still validates its preconditions.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    /// Whether a redo candidate exists; redo still validates its preconditions.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// Admit an existing source once. Duplicate identities cannot replace a working copy.
    pub fn open(&mut self, document: PrefabDocument) -> Result<()> {
        if self.documents.contains_key(&document.id) {
            return Err(failure("document already open; share the working document"));
        }
        self.saved.insert(document.id, document.clone());
        self.documents.insert(
            document.id,
            WorkingDocument {
                document,
                revision: 1,
                saved_revision: 1,
            },
        );
        self.group_scope = std::sync::Arc::new(());
        Ok(())
    }
    /// Build an independent candidate catalog without publishing or writing it.
    pub fn catalog(&self) -> Catalog {
        Catalog {
            documents: self
                .documents
                .iter()
                .map(|(id, d)| (*id, d.document.clone()))
                .collect(),
        }
    }
    /// Apply one revision-guarded source transaction after graph and caller validation.
    /// Any error preserves working copies, history and saved revisions.
    pub fn transact_checked(
        &mut self,
        expected: &BTreeMap<DocumentId, u64>,
        edit: impl FnOnce(&mut BTreeMap<DocumentId, PrefabDocument>) -> Result<()>,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        self.transact_named("Edit sources", expected, edit, validate)
    }
    /// Atomically publish one named source operation after full caller validation.
    /// Invalid labels, stale revisions and errors preserve sources/history/Redo.
    /// A no-op creates no entry. This is working-source commit, never filesystem Save.
    pub fn transact_named(
        &mut self,
        label: &str,
        expected: &BTreeMap<DocumentId, u64>,
        edit: impl FnOnce(&mut BTreeMap<DocumentId, PrefabDocument>) -> Result<()>,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        self.transact(label, expected, edit, validate, false)
    }
    fn transact(
        &mut self,
        label: &str,
        expected: &BTreeMap<DocumentId, u64>,
        edit: impl FnOnce(&mut BTreeMap<DocumentId, PrefabDocument>) -> Result<()>,
        validate: impl FnOnce(&Catalog) -> Result<()>,
        merge: bool,
    ) -> Result<()> {
        history::validate_label(label)?;
        for (id, revision) in expected {
            if self
                .documents
                .get(id)
                .is_none_or(|d| d.revision != *revision)
            {
                return Err(failure("stale document revision"));
            }
        }
        let before = self.documents.clone();
        let mut candidate = self.catalog().documents;
        edit(&mut candidate)?;
        let catalog = Catalog {
            documents: candidate.clone(),
        };
        for id in catalog.documents.keys() {
            catalog.resolve(*id)?;
        }
        validate(&catalog)?;
        let changed: BTreeSet<_> = candidate
            .iter()
            .filter(|(id, d)| before.get(id).is_none_or(|old| old.document != **d))
            .map(|(id, _)| *id)
            .chain(
                before
                    .keys()
                    .filter(|id| !candidate.contains_key(id))
                    .copied(),
            )
            .collect();
        if changed
            .iter()
            .any(|id| !expected.contains_key(id) && before.contains_key(id))
        {
            return Err(failure(
                "transaction edited a document without revision precondition",
            ));
        }
        if changed.is_empty() {
            return Ok(());
        }
        let clock = self
            .clock
            .max(
                self.documents
                    .values()
                    .map(|d| d.revision)
                    .max()
                    .unwrap_or(0),
            )
            .checked_add(1)
            .ok_or_else(|| failure("authoring revision counter exhausted"))?;
        let documents: BTreeMap<_, _> = candidate
            .into_iter()
            .map(|(id, document)| {
                let previous = before.get(&id);
                let revision = if changed.contains(&id) {
                    clock
                } else {
                    previous.map_or(clock, |d| d.revision)
                };
                (
                    id,
                    WorkingDocument {
                        document,
                        revision,
                        saved_revision: previous.map_or(0, |d| d.saved_revision),
                    },
                )
            })
            .collect();
        let latest = History {
            id: HistoryId(clock),
            label: label.into(),
            before: before
                .into_iter()
                .filter(|(id, _)| changed.contains(id))
                .collect(),
            after: documents
                .iter()
                .filter(|(id, _)| changed.contains(id))
                .map(|(id, doc)| (*id, doc.clone()))
                .collect(),
            mapping: None,
        };
        let history = if merge {
            let mut previous = self
                .undo
                .last()
                .cloned()
                .ok_or_else(|| failure("live history entry missing"))?;
            previous.merge(latest);
            previous
        } else {
            latest
        };
        self.check_history(&history)?;
        let mut coupled: BTreeSet<_> = history
            .before
            .keys()
            .chain(history.after.keys())
            .copied()
            .collect();
        self.documents = documents;
        self.clock = clock;
        if coupled.len() > 1 {
            loop {
                let count = coupled.len();
                self.coupled.retain(|group| {
                    if group.is_disjoint(&coupled) {
                        true
                    } else {
                        coupled.extend(group);
                        false
                    }
                });
                if coupled.len() == count {
                    break;
                }
            }
            self.coupled.push(coupled);
        }
        if merge {
            self.undo.pop();
        }
        self.undo.push(history);
        self.redo.clear();
        self.trim_history();
        self.last_mapping = None;
        Ok(())
    }
    /// Undo atomically against unchanged affected source content and a caller validator.
    pub fn undo_checked(&mut self, validate: impl FnOnce(&Catalog) -> Result<()>) -> Result<()> {
        let item = self
            .undo
            .last()
            .cloned()
            .ok_or_else(|| failure("nothing to undo"))?;
        self.restore(&item.after, &item.before, validate)?;
        self.last_mapping = item.mapping.as_ref().map(|(id, mapping)| {
            (
                *id,
                mapping
                    .iter()
                    .map(|(a, b)| (b.clone(), a.clone()))
                    .collect(),
            )
        });
        self.undo.pop();
        self.redo.push(item);
        Ok(())
    }
    /// Redo atomically; conflicting external source changes fail without consuming history.
    pub fn redo_checked(&mut self, validate: impl FnOnce(&Catalog) -> Result<()>) -> Result<()> {
        let item = self
            .redo
            .last()
            .cloned()
            .ok_or_else(|| failure("nothing to redo"))?;
        self.restore(&item.before, &item.after, validate)?;
        self.last_mapping = item.mapping.clone();
        self.redo.pop();
        self.undo.push(item);
        Ok(())
    }
    fn restore(
        &mut self,
        from: &BTreeMap<DocumentId, WorkingDocument>,
        to: &BTreeMap<DocumentId, WorkingDocument>,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        let affected: BTreeSet<_> = from
            .keys()
            .chain(to.keys())
            .filter(|id| from.get(id).map(|d| &d.document) != to.get(id).map(|d| &d.document))
            .copied()
            .collect();
        let mut candidate = self.documents.clone();
        let revision = self
            .clock
            .max(
                self.documents
                    .values()
                    .map(|d| d.revision)
                    .max()
                    .unwrap_or(0),
            )
            .checked_add(1)
            .ok_or_else(|| failure("authoring revision counter exhausted"))?;
        for id in &affected {
            if candidate.get(id).map(|d| &d.document) != from.get(id).map(|d| &d.document) {
                return Err(failure(
                    "history conflicts with externally changed document",
                ));
            }
            if let Some(old) = to.get(id) {
                let saved_revision = candidate.get(id).map_or(0, |d| d.saved_revision);
                candidate.insert(
                    *id,
                    WorkingDocument {
                        document: old.document.clone(),
                        revision,
                        saved_revision,
                    },
                );
            } else {
                candidate.remove(id);
            }
        }
        let catalog = Catalog {
            documents: candidate
                .iter()
                .map(|(id, d)| (*id, d.document.clone()))
                .collect(),
        };
        for id in catalog.documents.keys() {
            catalog.resolve(*id)?;
        }
        validate(&catalog)?;
        self.documents = candidate;
        self.clock = revision;
        Ok(())
    }
    /// Whether current authored content differs from the acknowledged source content.
    /// Revisions still advance monotonically; returning to saved content is clean.
    pub fn dirty(&self, id: DocumentId) -> bool {
        self.documents
            .get(&id)
            .is_some_and(|d| self.saved.get(&id) != Some(&d.document))
    }
    /// Compute the transaction-coupled document set that must be saved together.
    pub fn save_group(&self, id: DocumentId) -> BTreeSet<DocumentId> {
        let mut result = BTreeSet::from([id]);
        loop {
            let before = result.len();
            for group in &self.coupled {
                if !group.is_disjoint(&result) {
                    result.extend(group);
                }
            }
            if before == result.len() {
                // Keep history edges for redo, but an undone, never-saved creation is
                // not a live Save member. Compute the full closure before filtering.
                result.retain(|id| self.documents.contains_key(id));
                return result;
            }
        }
    }
    pub(crate) fn acknowledge(
        &mut self,
        revisions: &BTreeMap<DocumentId, (u64, Option<PrefabDocument>)>,
    ) {
        // A Save boundary must not be absorbed by a continuing live edit.
        self.group_scope = std::sync::Arc::new(());
        for (id, (revision, source)) in revisions {
            match source {
                Some(source) => {
                    self.saved.insert(*id, source.clone());
                }
                None => {
                    self.saved.remove(id);
                }
            }
            if let Some(document) = self.documents.get_mut(id) {
                document.saved_revision = *revision;
            }
        }
    }
}
