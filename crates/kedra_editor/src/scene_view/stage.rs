//! The verified editor delivery owns the ordinary authored inspection stage.
use bevy::{asset::LoadState, prelude::*};
use kedra_scene::{
    composition::Catalog, document::PrefabDocument, instance::CompiledPrefab,
    reflection::ComponentPolicies,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Resource)]
struct Stage {
    handle: Handle<PrefabDocument>,
    compiled: Option<Arc<CompiledPrefab>>,
}
pub(super) fn ready(world: &mut World) -> Result<Option<Arc<CompiledPrefab>>, String> {
    if !world.contains_resource::<Stage>() {
        let handle = world
            .resource::<AssetServer>()
            .load("editor://scenes/model-preview.scene.ron");
        world.insert_resource(Stage {
            handle,
            compiled: None,
        });
    }
    world.resource_scope(|world, mut stage: Mut<Stage>| {
        let server = world.resource::<AssetServer>().clone();
        if let Some(LoadState::Failed(error)) = server.get_load_state(stage.handle.id()) {
            return Err(format!("Editor preview stage failed: {error}"));
        }
        if stage.compiled.is_none() {
            let Some(source) = world
                .resource::<Assets<PrefabDocument>>()
                .get(&stage.handle)
                .cloned()
            else {
                return Ok(None);
            };
            let id = source.id;
            let mut catalog = Catalog::default();
            catalog.insert(source).map_err(|e| e.to_string())?;
            let graph = catalog.resolve(id).map_err(|e| e.to_string())?;
            super::projection::supported(&graph)?;
            if graph.nodes.values().any(|node| {
                node.components.keys().any(|key| {
                    !matches!(
                        key.as_str(),
                        "scene.transform" | "scene.sun" | "scene.environment"
                    )
                })
            }) {
                return Err(
                    "Editor lighting stage supports only environment and light nodes".into(),
                );
            }
            let registry = world.resource::<AppTypeRegistry>().clone();
            stage.compiled = Some(Arc::new(
                CompiledPrefab::compile(
                    graph,
                    &registry.read(),
                    world.resource::<ComponentPolicies>(),
                    &mut server.clone(),
                    &BTreeMap::new(),
                )
                .map_err(|e| e.to_string())?,
            ));
        }
        let compiled = stage
            .compiled
            .as_ref()
            .ok_or("Editor stage compilation unavailable")?;
        if !compiled.asset_status(&server).map_err(|e| e.to_string())? {
            return Ok(None);
        }
        Ok(Some(compiled.clone()))
    })
}
