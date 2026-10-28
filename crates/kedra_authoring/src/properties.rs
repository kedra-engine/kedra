//! Typed, revision-pinned property candidates, independent of widgets and gameplay.
//!
//! A session targets the explicitly opened document. Editing a nested prefab node
//! creates an override there; it never applies the edit to the prefab source.
//! Proposals use native Bevy values and paths. They do not publish source changes,
//! consume history, write files or acknowledge Save. Hosts must validate the complete
//! candidate catalog (including typed compilation) at both proposal and acceptance.
use crate::{
    failure,
    overrides::upsert,
    workspace::{EditGroup, WorkingSet},
};
use bevy::{
    ecs::reflect::ReflectComponent,
    reflect::{Access, ParsedPath, PartialReflect, ReflectPath, TypeRegistry},
};
use kedra_scene::{
    Result, composition::Catalog, document::*, instance::CompiledPrefab,
    reflection::ComponentPolicies,
};
use std::{collections::BTreeMap, sync::Arc};
mod lifting;

/// One explicit component destination in an opened source document.
/// Identity is persistent, not a UI label or a live entity handle.
#[derive(Clone, Debug)]
pub struct PropertyTarget {
    /// Document receiving local/instance field overrides, never an implicit Apply.
    pub document: DocumentId,
    /// Full composed source address within that document.
    pub node: NodeAddress,
    /// Canonical admitted component key.
    pub component: String,
}

/// One borrowed native field proposal. Values are encoded during the call;
/// neither reflection borrows nor UI/script handles are retained by the session.
pub struct PropertyEdit<'a> {
    /// Existing Bevy field path, not a display label or a new path language.
    pub path: ParsedPath,
    /// Exact represented native type of that field.
    pub value: &'a dyn PartialReflect,
}

#[derive(Clone)]
struct Proposal {
    path: ParsedPath,
    value: Box<ron::value::RawValue>,
    native: Arc<dyn PartialReflect>,
}

/// One cancellable candidate over an immutable source baseline.
///
/// The caller owns this value and its view/generation lifetime. Drop or `cancel`
/// discards only proposals. A successful `accept` closes the session; rejection
/// retains its candidate for inspection/correction. A closed session cannot write
/// again. No automatic rebasing or timer-based history coalescing occurs.
pub struct PropertySession {
    target: PropertyTarget,
    snapshot: Arc<CompiledPrefab>,
    expected: BTreeMap<DocumentId, u64>,
    version: u32,
    proposals: Vec<Proposal>,
    operations: Vec<Proposal>,
    closed: bool,
}

impl PropertySession {
    /// Begin against the exact current source compilation and dependency revisions.
    /// Missing/stale targets or policies fail without modifying the working set.
    pub fn begin(
        working: &WorkingSet,
        snapshot: Arc<CompiledPrefab>,
        target: PropertyTarget,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<Self> {
        if snapshot.graph().document != target.document {
            return Err(failure("property compilation belongs to another document"));
        }
        working.check_snapshot(&snapshot)?;
        let component = snapshot
            .graph()
            .nodes
            .get(&target.node)
            .and_then(|node| node.components.get(&target.component))
            .ok_or_else(|| failure("property component is unavailable"))?;
        let version = component.data.version;
        policies.policy(&target.component, version)?;
        component_value(&snapshot, &target, types, policies)?;
        let expected = snapshot
            .graph()
            .dependencies
            .iter()
            .chain(std::iter::once(&target.document))
            .map(|id| {
                working
                    .documents()
                    .get(id)
                    .map(|doc| (*id, doc.revision))
                    .ok_or_else(|| failure("property dependency is not open"))
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            target,
            snapshot,
            expected,
            version,
            proposals: Vec::new(),
            operations: Vec::new(),
            closed: false,
        })
    }

    /// Propose a native typed field value and validate the entire candidate.
    ///
    /// The exact represented type must match. Paths are Bevy paths; overlapping
    /// parent/child proposals are rejected rather than ordered implicitly. Repeated
    /// proposals for the same path replace the previous candidate. At most 32
    /// disjoint fields / 64 KiB of encoded proposals are retained. Unchanged values
    /// remove that field's proposal, preserving redo/history on no-op acceptance.
    ///
    /// Entity/asset identities require a separate symbolic picker contract: these
    /// proposals encode with no live entity mapping and cannot manufacture one.
    /// Indexed/compound edits are revision-pinned and lifted into a complete owning
    /// component; indices never become persistent source identities. Edits under an
    /// existing same-layer parent override preserve its effective siblings.
    /// An empty path replaces the complete component. Symbolic references are preserved
    /// when lifting, and source policy/catalog validation still precedes publication.
    /// Errors preserve the previously validated proposals and all working state.
    pub fn propose(
        &mut self,
        working: &WorkingSet,
        path: ParsedPath,
        value: &dyn PartialReflect,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        self.propose_batch(
            working,
            &[PropertyEdit { path, value }],
            types,
            policies,
            validate,
        )
    }

    /// Replace several field proposals together before validating the candidate.
    ///
    /// This is necessary for coupled constraints (for example, moving both ends
    /// of an interval). Duplicate/overlapping paths in a batch fail independently
    /// of order. No intermediate candidate is published or validated as an edit.
    /// A failed batch preserves the previous candidate, including other fields.
    pub fn propose_batch(
        &mut self,
        working: &WorkingSet,
        edits: &[PropertyEdit<'_>],
        types: &TypeRegistry,
        policies: &ComponentPolicies,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        self.check(working)?;
        if edits.len() > 32 {
            return Err(failure("property batch exceeds its field limit"));
        }
        let mut proposals = self.proposals.clone();
        let mut paths = Vec::new();
        for edit in edits {
            if edit.path.0.len() > 32 {
                return Err(failure(
                    "property edits require a bounded native field path",
                ));
            }
            let path = ParsedPath::from(
                edit.path
                    .0
                    .iter()
                    .map(|p| p.access.clone())
                    .collect::<Vec<_>>(),
            );
            if paths.iter().any(|other| overlaps(other, &path)) {
                return Err(failure("property batch paths overlap"));
            }
            paths.push(path.clone());
            let baseline = (&path)
                .reflect_element(component_value(
                    &self.snapshot,
                    &self.target,
                    types,
                    policies,
                )?)
                .map_err(|error| failure(error.to_string()))?;
            let ty = baseline
                .get_represented_type_info()
                .ok_or_else(|| failure("property has no represented type"))?;
            if types.get(ty.type_id()).is_none()
                || edit
                    .value
                    .get_represented_type_info()
                    .is_none_or(|info| info.type_id() != ty.type_id())
            {
                return Err(failure(
                    "property proposal has a different or unregistered type",
                ));
            }
            let encoded = policies.encode_value(edit.value, types, &BTreeMap::new())?;
            let unchanged = match baseline.reflect_partial_eq(edit.value) {
                Some(equal) => equal,
                None => policies.encode_value(baseline, types, &BTreeMap::new())? == encoded,
            };
            proposals.retain(|proposal| proposal.path != path);
            if !unchanged {
                if proposals
                    .iter()
                    .any(|proposal| overlaps(&proposal.path, &path))
                {
                    return Err(failure("property proposals overlap"));
                }
                proposals.push(Proposal {
                    path,
                    value: encoded,
                    native: Arc::from(
                        edit.value
                            .reflect_clone()
                            .map(|value| value.into_partial_reflect())
                            .or_else(|_| edit.value.to_dynamic())
                            .map_err(|error| failure(error.to_string()))?,
                    ),
                });
            }
        }
        if proposals.len() > 32
            || proposals
                .iter()
                .map(|p| p.value.get_ron().len())
                .sum::<usize>()
                > 65_536
        {
            return Err(failure("property candidate exceeds its field/byte limit"));
        }
        let mut candidate = working.clone();
        let operations = self.lift(working, &proposals, types, policies)?;
        if operations
            .iter()
            .map(|operation| operation.value.get_ron().len())
            .sum::<usize>()
            > 65_536
        {
            return Err(failure(
                "complete owning property values exceed the candidate byte limit",
            ));
        }
        self.apply(&mut candidate, &operations, None, validate)?;
        self.proposals = proposals;
        self.operations = operations;
        Ok(())
    }

    /// Build and validate an isolated source candidate for an optional preview.
    /// The returned catalog is disposable and is never a Save or history input.
    pub fn candidate(
        &self,
        working: &WorkingSet,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<Catalog> {
        self.check(working)?;
        let mut candidate = working.clone();
        self.apply(&mut candidate, &self.operations, None, validate)?;
        Ok(candidate.catalog())
    }

    /// Revalidate and atomically accept all proposed fields as one history operation.
    /// Stale transitive revisions, invalid sources and host rejection preserve both
    /// working state and this candidate. Successful no-op acceptance adds no history.
    pub fn accept(
        &mut self,
        working: &mut WorkingSet,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        self.check(working)?;
        self.apply(working, &self.operations, None, validate)?;
        self.closed = true;
        Ok(())
    }

    /// Accept this exact candidate as one update in a caller-owned live gesture.
    /// Revisions and full catalog validation remain mandatory. Group/history errors
    /// preserve both this candidate and the supplied group; no-op adds no Undo step.
    /// See [`WorkingSet::transact_grouped`] for gesture ownership and revocation.
    pub fn accept_grouped(
        &mut self,
        working: &mut WorkingSet,
        group: &mut EditGroup,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        self.check(working)?;
        self.apply(working, &self.operations, Some(group), validate)?;
        self.closed = true;
        Ok(())
    }

    /// Retire this session without changing sources, history, redo or Save baselines.
    pub fn cancel(&mut self) {
        self.proposals.clear();
        self.operations.clear();
        self.closed = true;
    }

    /// Whether an accepted source change is currently proposed, not document dirty.
    pub fn has_changes(&self) -> bool {
        !self.closed && !self.proposals.is_empty()
    }

    fn check(&self, working: &WorkingSet) -> Result<()> {
        if self.closed {
            return Err(failure("property session is closed"));
        }
        for (id, revision) in &self.expected {
            if working
                .documents()
                .get(id)
                .is_none_or(|doc| doc.revision != *revision)
            {
                return Err(failure("property session conflicts with a source revision"));
            }
        }
        working.check_snapshot(&self.snapshot)
    }

    fn apply(
        &self,
        working: &mut WorkingSet,
        proposals: &[Proposal],
        group: Option<&mut EditGroup>,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        let edit = |documents: &mut BTreeMap<DocumentId, PrefabDocument>| {
            let document = documents
                .get_mut(&self.target.document)
                .ok_or_else(|| failure("property document is unavailable"))?;
            for proposal in proposals {
                upsert(
                    &mut document.overrides,
                    if proposal.path.0.is_empty() {
                        Operation::SetComponent {
                            node: self.target.node.clone(),
                            component: self.target.component.clone(),
                            data: ComponentData {
                                version: self.version,
                                value: proposal.value.clone(),
                            },
                        }
                    } else {
                        Operation::SetField {
                            node: self.target.node.clone(),
                            component: self.target.component.clone(),
                            version: self.version,
                            path: source_path(&proposal.path),
                            value: proposal.value.clone(),
                        }
                    },
                );
            }
            Ok(())
        };
        match group {
            Some(group) => working.transact_grouped_named(
                group,
                "Edit properties",
                &self.expected,
                edit,
                validate,
            ),
            None => working.transact_named("Edit properties", &self.expected, edit, validate),
        }
    }
}

fn overlaps(a: &ParsedPath, b: &ParsedPath) -> bool {
    let common = a.0.len().min(b.0.len());
    a.0.iter()
        .zip(&b.0)
        .take(common)
        .all(|(a, b)| a.access == b.access)
}

fn source_path(path: &ParsedPath) -> String {
    let text = path.to_string();
    if matches!(
        path.0.first().map(|item| &item.access),
        Some(Access::Field(_))
    ) {
        text.strip_prefix('.').unwrap_or(&text).into()
    } else {
        text
    }
}

fn component_value<'a>(
    snapshot: &'a CompiledPrefab,
    target: &PropertyTarget,
    types: &'a TypeRegistry,
    policies: &ComponentPolicies,
) -> Result<&'a dyn PartialReflect> {
    let component = snapshot
        .graph()
        .nodes
        .get(&target.node)
        .and_then(|node| node.components.get(&target.component))
        .ok_or_else(|| failure("property component is unavailable"))?;
    let policy = policies.policy(&target.component, component.data.version)?;
    let reflect = types
        .get_type_data::<ReflectComponent>(policy.type_id)
        .ok_or_else(|| failure("property component lacks reflected access"))?;
    let entity = snapshot
        .source_entities()
        .get(&target.node)
        .ok_or_else(|| failure("property node is unavailable"))?;
    reflect
        .reflect(snapshot.authoring_world().entity(*entity))
        .map(|value| value.as_partial_reflect())
        .ok_or_else(|| failure("property value is unavailable"))
}
