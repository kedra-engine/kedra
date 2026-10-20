//! Bevy asset integration. Publishing a source revision never mutates live instances.
use crate::Result;
use crate::{
    composition::Catalog, document::*, failure, instance::CompiledPrefab,
    reflection::ComponentPolicies,
};
use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Asset, TypePath)]
/// A native asset declaring every document in one atomic source catalog.
pub struct PrefabCatalogAsset {
    /// Native dependency handles; readiness includes every declared document.
    #[dependency]
    pub documents: Vec<Handle<PrefabDocument>>,
}
#[derive(Default, TypePath)]
/// Bounded RON loader for scene/prefab sources, registered through native Bevy metadata.
pub struct DocumentLoader;
impl AssetLoader for DocumentLoader {
    type Asset = PrefabDocument;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &(),
        _: &mut LoadContext<'_>,
    ) -> std::io::Result<PrefabDocument> {
        let mut bytes = Vec::new();
        futures_lite::AsyncReadExt::read_to_end(
            &mut futures_lite::AsyncReadExt::take(reader, 65_537),
            &mut bytes,
        )
        .await?;
        let text = std::str::from_utf8(&bytes).map_err(std::io::Error::other)?;
        PrefabDocument::parse(text).map_err(std::io::Error::other)
    }
    fn extensions(&self) -> &[&str] {
        &["prefab.ron", "scene.ron"]
    }
}
#[derive(Default, TypePath)]
/// Bounded native catalog loader. The importer owns discovery; runtime loads explicit members.
pub struct CatalogLoader;
impl CatalogLoader {
    /// Decode the exact bounded native catalog source. Rejects empty/oversized
    /// catalogs and invalid or duplicate asset paths before initiating any loads.
    /// Authoring uses the same admission rules for proposed catalog write bytes.
    pub fn parse(text: &str) -> Result<Vec<String>> {
        if text.len() > 65_536 {
            return Err(failure("scene catalog exceeds 64 KiB"));
        }
        let paths: Vec<String> = ron::from_str(text)?;
        if paths.is_empty() || paths.len() > 128 {
            return Err(failure("scene catalog requires 1..=128 documents"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for path in &paths {
            if path.len() > 4096
                || !seen.insert(path)
                || bevy::asset::AssetPath::try_parse(path).is_err()
            {
                return Err(failure("invalid/duplicate scene catalog asset path"));
            }
        }
        Ok(paths)
    }
}
impl AssetLoader for CatalogLoader {
    type Asset = PrefabCatalogAsset;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &(),
        context: &mut LoadContext<'_>,
    ) -> std::io::Result<PrefabCatalogAsset> {
        let mut bytes = Vec::new();
        futures_lite::AsyncReadExt::read_to_end(
            &mut futures_lite::AsyncReadExt::take(reader, 65_537),
            &mut bytes,
        )
        .await?;
        let text = std::str::from_utf8(&bytes).map_err(std::io::Error::other)?;
        let paths = Self::parse(text).map_err(std::io::Error::other)?;
        Ok(PrefabCatalogAsset {
            documents: paths.into_iter().map(|path| context.load(path)).collect(),
        })
    }
    fn extensions(&self) -> &[&str] {
        &["prefabs.ron"]
    }
}

/// The indivisible source version used for publication and Play isolation.
#[derive(Default)]
pub struct SourceRevision {
    /// The complete source set validated before this publication.
    pub catalog: Catalog,
    /// Immutable compiled entries, keyed by document identity.
    pub compiled: BTreeMap<DocumentId, Arc<CompiledPrefab>>,
    /// Monotonic successful-publication revision, not a filesystem timestamp.
    pub revision: u64,
}
#[derive(Resource, Default)]
/// Published and optionally pinned source ownership. Failed candidates remain explicit errors.
pub struct PrefabSources {
    /// Host-requested top-level scene, if any.
    pub requested_scene: Option<DocumentId>,
    /// Host-confirmed active scene, separate from source readiness.
    pub active_scene: Option<DocumentId>,
    /// Canonical workspace snapshots own publication; raw file events cannot publish partial commits.
    pub managed: bool,
    /// Explicit native catalog asset, never an implicit filesystem discovery path.
    pub handle: Handle<PrefabCatalogAsset>,
    published: Arc<SourceRevision>,
    /// A candidate revision is still loading/compiling.
    pub pending: bool,
    /// Every dependency of the published compilation is loaded.
    pub dependencies_ready: bool,
    /// Latest publication/dependency failure; a last-valid snapshot does not hide it.
    pub error: Option<String>,
    /// A Play session keeps an immutable version, independent of source publication.
    pinned: Option<Arc<SourceRevision>>,
}
impl PrefabSources {
    /// Whether the explicit pinned session or current unpinned publication can instantiate.
    pub fn ready(&self) -> bool {
        self.is_pinned() || (!self.pending && self.dependencies_ready && self.error.is_none())
    }
    /// Pin one ready revision. A second pin requires an explicit unpin; it cannot replace a session.
    pub fn pin(&mut self) -> Result<()> {
        if self.pinned.is_some() {
            return Err(failure("source session is already pinned"));
        }
        if !self.ready() {
            return Err(failure("prefab sources are not ready"));
        }
        self.pinned = Some(self.published.clone());
        Ok(())
    }
    /// Borrow ownership of the explicitly pinned session, or the current unpinned revision.
    pub fn snapshot(&self) -> Arc<SourceRevision> {
        self.pinned.as_ref().unwrap_or(&self.published).clone()
    }
    /// Inspect the published revision independently of a session pin.
    pub fn published(&self) -> &Arc<SourceRevision> {
        &self.published
    }
    /// Whether a host-owned session explicitly holds an immutable revision.
    pub fn is_pinned(&self) -> bool {
        self.pinned.is_some()
    }
    /// End a pinned session. Existing instances retain their own snapshot/dependencies.
    pub fn unpin(&mut self) {
        self.pinned = None;
    }
    /// Publish an already validated whole candidate, without rewriting source files or instances.
    pub fn publish(
        &mut self,
        catalog: Catalog,
        compiled: BTreeMap<DocumentId, Arc<CompiledPrefab>>,
    ) -> Result<()> {
        let validation = (|| -> Result<()> {
            if catalog.documents.len() != compiled.len() {
                return Err(failure("publication has incomplete compiled sources"));
            }
            for id in catalog.documents.keys() {
                let entry = compiled
                    .get(id)
                    .ok_or_else(|| failure("publication is missing a compiled document"))?;
                if entry.graph() != &catalog.resolve(*id)? {
                    return Err(failure("compiled source differs from publication catalog"));
                }
            }
            Ok(())
        })();
        if let Err(error) = validation {
            self.error = Some(error.to_string());
            self.pending = false;
            return Err(error);
        }
        // Asset values can be visible before their Added/Modified message arrives.
        // A delayed notification is not another source revision or another snapshot.
        if self.published.revision > 0
            && self.error.is_none()
            && self.published.catalog.documents == catalog.documents
        {
            self.pending = false;
            return Ok(());
        }
        let dependencies_ready = compiled.values().all(|p| !p.has_dependencies());
        let revision = self
            .published
            .revision
            .checked_add(1)
            .ok_or_else(|| failure("source revision counter exhausted"))?;
        self.published = Arc::new(SourceRevision {
            catalog,
            compiled,
            revision,
        });
        self.error = None;
        self.pending = false;
        self.dependencies_ready = dependencies_ready;
        Ok(())
    }
}

/// Native scene asset integration. The caller must provide its catalog explicitly.
pub struct PrefabPlugin {
    /// Native source/UUID asset path; there is no default or filesystem fallback.
    pub catalog_path: String,
}
/// Native scene/catalog asset types and bounded loaders, without selecting a scene or catalog.
pub struct SceneAssetPlugin;
impl Plugin for SceneAssetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ComponentPolicies>()
            .init_asset::<PrefabDocument>()
            .init_asset::<PrefabCatalogAsset>()
            .init_asset::<crate::environment::Environment>()
            .init_asset_loader::<crate::environment::EnvironmentLoader>()
            .init_asset_loader::<DocumentLoader>()
            .init_asset_loader::<CatalogLoader>();
    }
}
impl Plugin for PrefabPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<SceneAssetPlugin>() {
            app.add_plugins(SceneAssetPlugin);
        }
        let handle = app
            .world()
            .resource::<AssetServer>()
            .load(self.catalog_path.clone());
        app.insert_resource(PrefabSources {
            handle,
            pending: true,
            ..default()
        })
        .add_systems(Update, (poll, record_failures).chain());
    }
}
/// Compile a complete catalog before publication; missing document links or any invalid graph
/// reject the whole candidate. No live world is mutated.
pub fn compile_catalog(
    catalog: &Catalog,
    types: &bevy::reflect::TypeRegistry,
    policies: &ComponentPolicies,
    loader: &mut dyn bevy::asset::LoadFromPath,
) -> Result<BTreeMap<DocumentId, Arc<CompiledPrefab>>> {
    if catalog.documents.len() > 128 {
        return Err(failure("scene catalog exceeds 128 documents"));
    }
    let compiled: BTreeMap<DocumentId, Arc<CompiledPrefab>> = catalog
        .documents
        .keys()
        .map(|id| {
            CompiledPrefab::compile(
                catalog.resolve(*id)?,
                types,
                policies,
                loader,
                &BTreeMap::new(),
            )
            .map(|compiled| (*id, Arc::new(compiled)))
        })
        .collect::<Result<_>>()?;
    if compiled
        .values()
        .map(|p| p.graph().nodes.len())
        .sum::<usize>()
        > 65_536
    {
        return Err(failure("compiled catalog exceeds 65536 total nodes"));
    }
    for prefab in compiled.values() {
        for target in prefab.document_links() {
            if !catalog.documents.contains_key(target) {
                return Err(failure(format!(
                    "document reference target missing: {target}"
                )));
            }
        }
    }
    Ok(compiled)
}

fn record_failures(
    mut failures: MessageReader<bevy::asset::UntypedAssetLoadFailedEvent>,
    indices: Res<Assets<PrefabCatalogAsset>>,
    mut sources: ResMut<PrefabSources>,
) {
    if sources.managed {
        failures.clear();
        return;
    }
    for event in failures.read() {
        let source = event.id == sources.handle.id().untyped()
            || indices.get(&sources.handle).is_some_and(|index| {
                index
                    .documents
                    .iter()
                    .any(|handle| handle.id().untyped() == event.id)
            });
        if source
            || sources
                .published
                .compiled
                .values()
                .any(|prefab| prefab.depends_on(event.id))
        {
            sources.error = Some(format!("{}: {}", event.path, event.error));
            sources.pending = false;
        }
    }
}
#[allow(
    clippy::too_many_arguments,
    reason = "explicit Bevy asset inputs and source publication"
)]
fn poll(
    mut events: MessageReader<AssetEvent<PrefabDocument>>,
    mut indices: MessageReader<AssetEvent<PrefabCatalogAsset>>,
    server: Res<AssetServer>,
    documents: Res<Assets<PrefabDocument>>,
    catalogs: Res<Assets<PrefabCatalogAsset>>,
    types: Res<AppTypeRegistry>,
    policies: Res<ComponentPolicies>,
    mut sources: ResMut<PrefabSources>,
) {
    if sources.managed {
        events.clear();
        indices.clear();
        sources.pending = false;
        let status = sources
            .published
            .compiled
            .values()
            .try_fold(true, |ready, prefab| {
                prefab.asset_status(&server).map(|value| ready && value)
            });
        match status {
            Ok(ready) => sources.dependencies_ready = ready,
            Err(error) => sources.error = Some(error.to_string()),
        }
        return;
    }
    let changed = events.read().fold(false, |changed, e| {
        changed
            | matches!(
                e,
                AssetEvent::Added { .. } | AssetEvent::Modified { .. } | AssetEvent::Removed { .. }
            )
    });
    let changed = indices.read().fold(changed, |changed, e| {
        changed | matches!(e, AssetEvent::Added { .. } | AssetEvent::Modified { .. })
    });
    if changed {
        sources.pending = true;
    }
    if !sources.pending {
        let status = sources
            .published
            .compiled
            .values()
            .try_fold(true, |ready, prefab| {
                prefab.asset_status(&server).map(|value| ready && value)
            });
        match status {
            Ok(ready) => sources.dependencies_ready = ready,
            Err(error) => sources.error = Some(error.to_string()),
        }
        return;
    }
    if let Some(bevy::asset::LoadState::Failed(error)) = server.get_load_state(sources.handle.id())
    {
        sources.error = Some(error.to_string());
        sources.pending = false;
        return;
    }
    let Some(index) = catalogs.get(&sources.handle) else {
        return;
    };
    let mut catalog = Catalog::default();
    for handle in &index.documents {
        if let Some(bevy::asset::LoadState::Failed(error)) = server.get_load_state(handle.id()) {
            sources.error = Some(error.to_string());
            sources.pending = false;
            return;
        }
        if !matches!(
            server.get_load_state(handle.id()),
            Some(bevy::asset::LoadState::Loaded)
        ) {
            return;
        }
        let Some(document) = documents.get(handle) else {
            return;
        };
        if let Err(error) = catalog.insert(document.clone()) {
            sources.error = Some(error.to_string());
            sources.pending = false;
            return;
        }
    }
    let mut loader = server.as_ref();
    match compile_catalog(&catalog, &types.read(), &policies, &mut loader) {
        Ok(compiled) => {
            if let Err(error) = sources.publish(catalog, compiled) {
                sources.error = Some(error.to_string());
                sources.pending = false;
            }
        }
        Err(error) => {
            sources.error = Some(error.to_string());
            sources.pending = false;
        }
    }
}
