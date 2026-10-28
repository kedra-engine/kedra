//! Native source compilation over admitted logical asset identities. No filesystem
//! access, UI dependencies, live asset loading or project execution is installed.
use crate::persistence::{CommitRequest, SourceSnapshot, Workspace};
use bevy::{
    asset::{AssetPath, LoadFromPath, UntypedHandle},
    reflect::TypeRegistry,
};
use kedra_scene::{
    document::{DocumentId, PrefabDocument},
    reflection::ComponentPolicies,
};
use std::{any::TypeId, collections::BTreeMap, sync::Arc};
use uuid::Uuid;
#[cfg(test)]
mod tests;

/// Exact immutable source compilations, indexed by persistent document identity.
pub type CompiledSources = BTreeMap<DocumentId, Arc<kedra_scene::instance::CompiledPrefab>>;
/// One admitted source workspace and its matching native compilations.
pub type Admission = (Workspace, CompiledSources);

struct SourceHandles<'a> {
    source: &'a SourceSnapshot,
    error: Option<String>,
    labels: BTreeMap<(TypeId, Uuid, String), Uuid>,
}
impl<'a> SourceHandles<'a> {
    fn new(source: &'a SourceSnapshot) -> Self {
        Self {
            source,
            error: None,
            labels: BTreeMap::new(),
        }
    }
}
impl LoadFromPath for SourceHandles<'_> {
    fn load_from_path_erased(
        &mut self,
        type_id: TypeId,
        path: AssetPath<'static>,
    ) -> UntypedHandle {
        let value = path.to_string();
        let resolved = value
            .split_once("://")
            .filter(|(namespace, _)| *namespace == self.source.namespace)
            .and_then(|(_, locator)| {
                let locator = locator.split('#').next()?;
                let identity = locator.split_once('/').and_then(|(id, leaf)| {
                    (leaf.starts_with("asset.") && leaf.len() > 6 && !leaf.contains('/'))
                        .then(|| id.parse::<Uuid>().ok())
                        .flatten()
                });
                self.source
                    .assets
                    .iter()
                    .find(|a| a.path == locator || (identity.is_some() && a.id == identity))
                    .and_then(|a| a.id)
                    .or_else(|| {
                        self.source
                            .documents
                            .iter()
                            .filter_map(|d| {
                                PrefabDocument::parse(&d.source).ok().map(|doc| (d, doc))
                            })
                            .find(|(d, doc)| d.path == locator || Some(doc.id.0) == identity)
                            .map(|(_, doc)| doc.id.0)
                    })
            });
        if resolved.is_none() {
            self.error = Some(format!(
                "Referenced asset is outside the source scope or has no metadata: {value}"
            ));
        }
        UntypedHandle::Uuid {
            type_id,
            uuid: match (resolved, path.label()) {
                (Some(id), Some(label)) => *self
                    .labels
                    .entry((type_id, id, label.to_owned()))
                    .or_insert_with(Uuid::new_v4),
                (Some(id), None) => id,
                (None, _) => Uuid::nil(),
            },
        }
    }
}

/// Admit one complete source snapshot with exact typed validation. Returns no
/// partially runnable workspace on failure; never reads or writes files.
pub fn admit(
    source: SourceSnapshot,
    types: &TypeRegistry,
    policies: &ComponentPolicies,
) -> Result<Admission, String> {
    let mut loader = SourceHandles::new(&source);
    let (workspace, compiled) =
        Workspace::open_compiled(source.clone(), types, policies, &mut loader);
    if let Some(error) = loader.error.or_else(|| workspace.diagnostic.clone()) {
        return Err(error);
    }
    Ok((workspace, compiled))
}
/// Compile accepted working content without changing its Save baseline or loading assets.
pub fn compile_working(
    workspace: &Workspace,
    types: &TypeRegistry,
    policies: &ComponentPolicies,
) -> Result<CompiledSources, String> {
    compile_catalog(
        &workspace.source,
        &workspace.working.catalog(),
        types,
        policies,
    )
}
/// Prepare a pinned existing-source Save using the same admitted symbolic loader.
/// Failure retains the original workspace and pending request; performs no I/O.
pub fn prepare_existing_save(
    workspace: &mut Workspace,
    selected: DocumentId,
    types: &TypeRegistry,
    policies: &ComponentPolicies,
) -> Result<CommitRequest, String> {
    let baseline = workspace.source.clone();
    let mut loader = SourceHandles::new(&baseline);
    let mut candidate = workspace.clone();
    let request = candidate
        .prepare_existing_save(selected, types, policies, &mut loader)
        .map_err(|error| error.to_string())?
        .clone();
    if let Some(error) = loader.error {
        return Err(error);
    }
    *workspace = candidate;
    Ok(request)
}
/// Compile a complete isolated catalog against its admitted resource inventory.
/// Unknown/untracked resources refuse; labeled handles retain exact native paths.
pub fn compile_catalog(
    source: &SourceSnapshot,
    catalog: &kedra_scene::composition::Catalog,
    types: &TypeRegistry,
    policies: &ComponentPolicies,
) -> Result<CompiledSources, String> {
    let mut loader = SourceHandles::new(source);
    let compiled = kedra_scene::assets::compile_catalog(catalog, types, policies, &mut loader)
        .map_err(|error| error.to_string())?;
    if let Some(error) = loader.error {
        return Err(error);
    }
    Ok(compiled)
}
