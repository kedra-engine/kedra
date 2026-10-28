//! Static native model projection. Bevy owns entity/reference remapping and cloning.
use bevy::{
    camera::{primitives::Aabb, visibility::RenderLayers},
    ecs::entity::EntityHashMap,
    prelude::*,
    world_serialization::{DynamicWorldBuilder, WorldAsset, WorldFilter},
};

/// Preflight the same audited materialization/bounds route used by Scene View,
/// without publishing a source or retaining temporary presentation entities.
pub fn validate(world: &mut World, handle: &Handle<WorldAsset>) -> Result<(), String> {
    let owner = world.spawn((Transform::IDENTITY, Visibility::Hidden)).id();
    let result = mount(world, owner, handle, 0, 4095)
        .and_then(|_| super::bounds::measure(world, owner).map(|_| ()));
    if let Ok(owner) = world.get_entity_mut(owner) {
        owner.despawn();
    }
    result
}

/// One explicitly owned static forest. Retire explicitly; Drop does not mutate World.
pub struct Instance {
    nodes: Vec<Entity>,
    roots: Vec<Entity>,
}
impl Instance {
    #[cfg(any(debug_assertions, test))]
    /// Observe material colors for native acceptance without changing assets.
    pub fn material_colors(&self, world: &World) -> Vec<[f32; 4]> {
        self.nodes
            .iter()
            .filter_map(|entity| {
                let material = world.get::<MeshMaterial3d<StandardMaterial>>(*entity)?;
                let color = world
                    .resource::<Assets<StandardMaterial>>()
                    .get(&material.0)?
                    .base_color
                    .to_srgba();
                Some([color.red, color.green, color.blue, color.alpha])
            })
            .collect()
    }
    /// Number of owned entities for the host's aggregate budget.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    /// Whether this static asset materialized no native entities.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    /// Check all recorded relationships before replacement of the serving forest.
    pub fn owned_by(&self, world: &World, parent: Entity) -> bool {
        self.nodes.iter().all(|entity| {
            world.get::<ChildOf>(*entity).is_some_and(|owner| {
                if self.roots.contains(entity) {
                    owner.parent() == parent
                } else {
                    self.nodes.contains(&owner.parent())
                }
            }) && world
                .get::<Children>(*entity)
                .is_none_or(|children| children.iter().all(|child| self.nodes.contains(&child)))
        })
    }
    /// Attach only recorded roots after complete staged materialization.
    pub fn attach(&self, world: &mut World, parent: Entity) {
        for root in &self.roots {
            world.entity_mut(*root).insert(ChildOf(parent));
        }
    }
    /// Retire only recorded entities, not shared assets or source documents.
    pub fn retire(self, world: &mut World) {
        for entity in self.nodes {
            if let Ok(entity) = world.get_entity_mut(entity) {
                entity.despawn();
            }
        }
    }
}

/// Mount an already loaded static WorldAsset with native remapping. Unsupported
/// skins/morphs/cameras/lights, external parents and exhausted budgets refuse.
/// Native write failure reclaims only this call's allocations.
pub fn mount(
    world: &mut World,
    parent: Entity,
    handle: &Handle<WorldAsset>,
    layer: usize,
    budget: usize,
) -> Result<Instance, String> {
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
    let mut roots = Vec::new();
    for entity in mapping.values() {
        if world.get::<ChildOf>(*entity).is_none() {
            roots.push(*entity);
            world.entity_mut(*entity).insert(ChildOf(parent));
        }
    }
    Ok(Instance {
        nodes: mapping.values().copied().collect(),
        roots,
    })
}
