//! Static native model projection. Bevy owns entity/reference remapping and cloning.
use bevy::{
    camera::{primitives::Aabb, visibility::RenderLayers},
    ecs::entity::EntityHashMap,
    prelude::*,
    world_serialization::{DynamicWorldBuilder, WorldAsset, WorldFilter},
};

pub(super) fn mount(
    world: &mut World,
    parent: Entity,
    handle: &Handle<WorldAsset>,
    layer: usize,
    budget: usize,
) -> Result<usize, String> {
    let scene = world
        .resource::<Assets<WorldAsset>>()
        .get(handle)
        .ok_or("Model scene dependency is not materialized")?;
    let mut entities = Vec::new();
    for entity in scene.world.iter_entities() {
        // Bevy 0.20 stores resources as entities. Match WorldAsset's native
        // resource exclusion rather than cloning them into the model hierarchy.
        if entity.contains_id(bevy::ecs::resource::IS_RESOURCE) {
            continue;
        }
        if entity.contains::<bevy::mesh::skinning::SkinnedMesh>()
            || entity.contains::<bevy::mesh::morph::MorphWeights>()
            || entity.contains::<Camera>()
            || entity.contains::<DirectionalLight>()
            || entity.contains::<PointLight>()
            || entity.contains::<SpotLight>()
        {
            return Err(
                "Static model preview does not activate imported skins, morphs, lights or cameras"
                    .into(),
            );
        }
        if entity
            .get::<Transform>()
            .is_some_and(|t| !t.to_matrix().is_finite())
        {
            return Err("Model has a nonfinite transform".into());
        }
        if entity.contains::<Mesh3d>() != entity.contains::<MeshMaterial3d<StandardMaterial>>() {
            return Err("Model primitive lacks a supported standard material".into());
        }
        entities.push(entity.id());
    }
    if entities.len() > budget {
        return Err("Models exceed the aggregate 4096-node presentation budget".into());
    }
    let count = entities.len();
    if scene.world.iter_entities().any(|entity| {
        entity
            .get::<ChildOf>()
            .is_some_and(|p| !entities.contains(&p.parent()))
    }) {
        return Err("Model has an external parent".into());
    }
    let registry = world.resource::<AppTypeRegistry>().clone();
    let dynamic = DynamicWorldBuilder::from_world(&scene.world, &registry.read())
        .with_component_filter(
            WorldFilter::deny_all()
                .allow::<Name>()
                .allow::<Transform>()
                .allow::<ChildOf>()
                .allow::<Children>()
                .allow::<Mesh3d>()
                .allow::<Aabb>()
                .allow::<MeshMaterial3d<StandardMaterial>>(),
        )
        .with_resource_filter(WorldFilter::deny_all())
        .extract_entities(entities.iter().copied())
        .build();
    // Only audited visual data is projected; extras are inert metadata. Native
    // DynamicWorld owns cloning/mapping, not another bespoke scene graph importer.
    let mut mapping = EntityHashMap::default();
    for source in entities {
        mapping.insert(
            source,
            world
                .spawn((
                    Transform::IDENTITY,
                    Visibility::Inherited,
                    RenderLayers::layer(layer),
                ))
                .id(),
        );
    }
    if let Err(error) = dynamic.write_to_world_with(world, &mut mapping, &registry.read()) {
        for entity in mapping.values() {
            if let Ok(entity) = world.get_entity_mut(*entity) {
                entity.despawn();
            }
        }
        return Err(format!("Model materialization failed: {error}"));
    }
    // Attach only complete roots after native remapping. Pre-parenting every
    // allocation would leave stale relationship-target entries when reflected
    // ChildOf values are applied to already-present components.
    for entity in mapping.values() {
        if world.get::<ChildOf>(*entity).is_none() {
            world.entity_mut(*entity).insert(ChildOf(parent));
        }
    }
    Ok(count)
}
