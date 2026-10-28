//! Prepared native source changes. Hosts settle UI input before publishing these
//! matching working/compiled pairs synchronously; this module owns no widgets.
use crate::{
    persistence::Workspace,
    properties::{PropertyEdit, PropertySession, PropertyTarget},
    source::{self, CompiledSources},
    workspace::{EditGroup, WorkingSet},
};
use bevy::reflect::TypeRegistry;
use kedra_scene::{document::DocumentId, reflection::ComponentPolicies};
use std::collections::BTreeMap;

/// A validated candidate and its exact compilation, never an acknowledged Save.
/// Preparation leaves the source workspace untouched. Hosts must recheck their
/// original scope/revisions before synchronous publication after UI acknowledgment.
pub struct PreparedSources {
    /// Isolated candidate working set, including the same source history lineage.
    pub working: WorkingSet,
    /// Exact compilations for that candidate, not for a gameplay World.
    pub compiled: CompiledSources,
}
/// Native property batch, independent of script transport and input widgets.
pub struct PropertyRequest<'a> {
    /// Immutable compilation matching the target's current composition sources.
    pub snapshot: std::sync::Arc<kedra_scene::instance::CompiledPrefab>,
    /// Persistent receiving node/component, including full instance scope.
    pub target: PropertyTarget,
    /// Exact complete working revision inventory captured by the host.
    pub expected: &'a BTreeMap<DocumentId, u64>,
    /// Native typed field proposals, preserving units/handles/symbolic references.
    pub edits: &'a [PropertyEdit<'a>],
}
/// Prepare one fully compiled property operation or update an explicit gesture.
/// Any error preserves workspace sources/history and leaves publication to the host.
pub fn property(
    workspace: &Workspace,
    request: PropertyRequest<'_>,
    group: Option<&mut EditGroup>,
    types: &TypeRegistry,
    policies: &ComponentPolicies,
) -> Result<PreparedSources, String> {
    let revisions: BTreeMap<_, _> = workspace
        .working
        .documents()
        .iter()
        .map(|(id, document)| (*id, document.revision))
        .collect();
    if revisions != *request.expected {
        return Err("Source revision changed before property preparation".into());
    }
    let mut working = workspace.working.clone();
    let mut session =
        PropertySession::begin(&working, request.snapshot, request.target, types, policies)
            .map_err(|error| error.to_string())?;
    session
        .propose_batch(&working, request.edits, types, policies, |catalog| {
            source::compile_catalog(&workspace.source, catalog, types, policies)
                .map(|_| ())
                .map_err(kedra_scene::Error)
        })
        .map_err(|error| error.to_string())?;
    let mut compiled = None;
    let validate = |catalog: &kedra_scene::composition::Catalog| {
        compiled = Some(
            source::compile_catalog(&workspace.source, catalog, types, policies)
                .map_err(kedra_scene::Error)?,
        );
        Ok(())
    };
    match group {
        Some(group) => session.accept_grouped(&mut working, group, validate),
        None => session.accept(&mut working, validate),
    }
    .map_err(|error| error.to_string())?;
    Ok(PreparedSources {
        working,
        compiled: compiled.ok_or("Candidate compilation unavailable")?,
    })
}
/// Prepare common Undo/Redo with typed compilation, without changing the workspace.
pub fn history(
    workspace: &Workspace,
    redo: bool,
    types: &TypeRegistry,
    policies: &ComponentPolicies,
) -> Result<PreparedSources, String> {
    let mut working = workspace.working.clone();
    let mut compiled = None;
    let validate = |catalog: &kedra_scene::composition::Catalog| {
        compiled = Some(
            source::compile_catalog(&workspace.source, catalog, types, policies)
                .map_err(kedra_scene::Error)?,
        );
        Ok(())
    };
    if redo {
        working.redo_checked(validate)
    } else {
        working.undo_checked(validate)
    }
    .map_err(|error| error.to_string())?;
    Ok(PreparedSources {
        working,
        compiled: compiled.ok_or("History compilation unavailable")?,
    })
}
