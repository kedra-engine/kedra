//! Explicit one-shot entry scene loading over the existing published source catalog.
use crate::{
    Result,
    assets::{PrefabCatalogAsset, PrefabSources},
    document::{DocumentId, DocumentKind, PrefabDocument},
    failure,
    instance::CompiledPrefab,
};
use bevy::{
    asset::{AssetPath, LoadState},
    prelude::*,
};
use std::{collections::BTreeMap, sync::Arc};

/// Host-selected native hydration, not a scene-selected plugin or script.
/// On success the returned live root owns the complete instance. On error the
/// adapter must leave no partial instance. It must not execute project scripts.
pub type MountScene = fn(&mut World, &Arc<CompiledPrefab>) -> Result<Entity>;

/// Entry-scene lifecycle. Mounted means source instantiation, not GPU readiness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartupState {
    /// Waiting for the explicit catalog, scene and their dependencies.
    Loading,
    /// The host owns the live root; despawning it retires startup without reloading.
    Mounted {
        /// Source document identity, resolved from the selected asset path.
        document: DocumentId,
        /// Complete scene instance root.
        root: Entity,
    },
    /// Loading or hydration failed permanently; no fallback scene is selected.
    Failed(String),
    /// The mounted root was removed. There is no implicit restart or hot reload.
    Retired,
}

/// One explicitly selected startup scene. Does not create a window or a renderer.
#[derive(Resource)]
pub struct StartupScene {
    path: String,
    handle: Option<Handle<PrefabDocument>>,
    mount: MountScene,
    state: StartupState,
}
impl StartupScene {
    /// The explicit Bevy asset path, not a native filesystem display path.
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Current source/instance lifecycle; frames and input require separate evidence.
    pub fn state(&self) -> &StartupState {
        &self.state
    }
}

/// Select one Scene member of an already installed PrefabPlugin catalog by path.
/// Install component policies and asset sources first. The default mount uses
/// ordinary CompiledPrefab instantiation; UI hosts supply their native hydrator.
/// This grants no script execution, project discovery, Save or Play permission.
pub struct StartupScenePlugin {
    /// Explicit asset path of a Scene in the host's catalog.
    pub path: String,
    /// Trusted host's native adapter; never chosen by authored data.
    pub mount: MountScene,
}
impl StartupScenePlugin {
    /// Select a scene using ordinary source instantiation, without presentation hooks.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            mount: instantiate,
        }
    }
}
impl Plugin for StartupScenePlugin {
    fn build(&self, app: &mut App) {
        assert!(
            app.world().contains_resource::<PrefabSources>(),
            "Install PrefabPlugin before StartupScenePlugin"
        );
        let valid = !self.path.is_empty()
            && self.path.len() <= 4096
            && AssetPath::try_parse(self.path.as_str()).is_ok();
        let (handle, state) = if valid {
            (
                Some(
                    app.world()
                        .resource::<AssetServer>()
                        .load(self.path.clone()),
                ),
                StartupState::Loading,
            )
        } else {
            (
                None,
                StartupState::Failed("Invalid startup scene asset path".into()),
            )
        };
        app.insert_resource(StartupScene {
            path: self.path.clone(),
            handle,
            mount: self.mount,
            state,
        })
        .add_systems(PreUpdate, poll);
    }
}

fn instantiate(world: &mut World, scene: &Arc<CompiledPrefab>) -> Result<Entity> {
    scene.instantiate(world, None, &BTreeMap::new())
}

fn resolve(
    world: &World,
    handle: &Handle<PrefabDocument>,
) -> Result<Option<(DocumentId, Arc<CompiledPrefab>)>> {
    let sources = world.resource::<PrefabSources>();
    if let Some(error) = &sources.error {
        return Err(failure(error));
    }
    if let Some(LoadState::Failed(error)) =
        world.resource::<AssetServer>().get_load_state(handle.id())
    {
        return Err(failure(error.to_string()));
    }
    if !sources.ready() {
        return Ok(None);
    }
    let catalog = world
        .resource::<Assets<PrefabCatalogAsset>>()
        .get(&sources.handle)
        .ok_or_else(|| failure("Startup requires the loaded native source catalog"))?;
    if !catalog
        .documents
        .iter()
        .any(|member| member.id() == handle.id())
    {
        return Err(failure(
            "Startup scene is not a member of the host's source catalog",
        ));
    }
    let document = world
        .resource::<Assets<PrefabDocument>>()
        .get(handle)
        .ok_or_else(|| failure("Startup scene is not materialized"))?;
    if document.kind != DocumentKind::Scene {
        return Err(failure("Startup requires a Scene, not a Prefab"));
    }
    let snapshot = sources.snapshot();
    // Native asset storage can update before the catalog's publication system.
    // Never combine a new path/document value with an older compiled revision.
    if snapshot.catalog.documents.get(&document.id) != Some(document) {
        return Ok(None);
    }
    let compiled = snapshot
        .compiled
        .get(&document.id)
        .cloned()
        .ok_or_else(|| failure("Startup scene is not in the published source revision"))?;
    Ok(Some((document.id, compiled)))
}

fn poll(world: &mut World) {
    let startup = world.resource::<StartupScene>();
    match startup.state.clone() {
        StartupState::Mounted { document, root } if world.get_entity(root).is_err() => {
            world.resource_mut::<StartupScene>().state = StartupState::Retired;
            let mut sources = world.resource_mut::<PrefabSources>();
            if sources.active_scene == Some(document) {
                sources.active_scene = None;
            }
            return;
        }
        StartupState::Loading => (),
        _ => return,
    }
    let Some(handle) = startup.handle.clone() else {
        return;
    };
    let mount = startup.mount;
    let result = resolve(world, &handle).and_then(|candidate| {
        let Some((document, compiled)) = candidate else {
            return Ok(None);
        };
        let root = mount(world, &compiled)?;
        if world.get_entity(root).is_err() {
            return Err(failure("Startup scene adapter returned a retired root"));
        }
        Ok(Some((document, root)))
    });
    match result {
        Ok(None) => (),
        Ok(Some((document, root))) => {
            let mut sources = world.resource_mut::<PrefabSources>();
            sources.requested_scene = Some(document);
            sources.active_scene = Some(document);
            world.resource_mut::<StartupScene>().state = StartupState::Mounted { document, root };
        }
        Err(error) => {
            world.resource_mut::<StartupScene>().state = StartupState::Failed(error.to_string());
        }
    }
}
