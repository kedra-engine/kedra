//! Refresh owned spatial values without replacing entities, cameras or asset handles.
use super::*;
use bevy::camera::primitives::MeshAabb;
mod models;
#[cfg(test)]
mod tests;

impl Projection {
    pub(in crate::scene_view) fn refresh_spatial_values(
        &mut self,
        world: &mut World,
        compiled: &CompiledPrefab,
        before: &ResolvedGraph,
    ) -> Result<(), String> {
        let mut meshes = Vec::new();
        let mut materials = Vec::new();
        let mut lights = Vec::new();
        let mut environment = None;
        // Complete every fallible lookup and derived value before invalidating frames
        // or touching an owned asset. No systems run between this preflight and swap.
        for (address, source) in compiled.source_entities() {
            let entity = *self
                .nodes
                .get(address)
                .ok_or("Projected source node unavailable")?;
            let values = compiled.authoring_world();
            let changed = |key: &str| {
                before.nodes[address].components.get(key)
                    != compiled.graph().nodes[address].components.get(key)
            };
            if changed("scene.environment") {
                let reference = values
                    .get::<kedra_scene::environment::SceneEnvironment>(*source)
                    .ok_or("Source environment reference unavailable")?
                    .clone();
                if world
                    .get::<kedra_scene::environment::SceneEnvironment>(entity)
                    .is_none()
                {
                    return Err("Projected environment reference unavailable".into());
                }
                let value = world
                    .get_resource::<Assets<kedra_scene::environment::Environment>>()
                    .and_then(|assets| assets.get(&reference.asset))
                    .cloned()
                    .ok_or("Scene environment dependency is unavailable")?;
                value.validate().map_err(|error| error.to_string())?;
                let camera = self.camera.ok_or("Scene camera missing")?;
                if world.get::<Camera>(camera).is_none() {
                    return Err("Scene environment camera unavailable".into());
                }
                // The stage's native visibility is the existing lighting-mode
                // authority. Updating authored lighting must not overwrite studio
                // inspection, but switching back must use the newly accepted value.
                let inspection = self
                    .inspection
                    .and_then(|stage| world.get::<Visibility>(stage))
                    .is_some_and(|visibility| *visibility != Visibility::Hidden);
                environment = Some((entity, reference, value, camera, inspection));
            }
            if changed("scene.primitive")
                && let Some(primitive) = values.get::<Primitive>(*source)
            {
                let handle = world
                    .get::<Mesh3d>(entity)
                    .ok_or("Projected primitive mesh unavailable")?
                    .0
                    .clone();
                if world.resource::<Assets<Mesh>>().get(&handle).is_none()
                    || !self.meshes.contains(&handle)
                {
                    return Err("Projected primitive mesh is not owned by this view".into());
                }
                let mesh = kedra_scene::rendering::mesh(primitive);
                let bounds = mesh
                    .get_aabb()
                    .ok_or("Primitive has no finite mesh bounds")?;
                meshes.push((entity, handle, mesh, bounds, primitive.clone()));
            }
            if changed("scene.surface")
                && let Some(surface) = values.get::<Surface>(*source)
            {
                let handle = world
                    .get::<MeshMaterial3d<StandardMaterial>>(entity)
                    .ok_or("Projected surface unavailable")?
                    .0
                    .clone();
                if world
                    .resource::<Assets<StandardMaterial>>()
                    .get(&handle)
                    .is_none()
                    || !self.materials.contains(&handle)
                {
                    return Err("Projected surface is not owned by this view".into());
                }
                materials.push((
                    entity,
                    handle,
                    kedra_scene::rendering::material(surface),
                    surface.clone(),
                ));
            }
            if changed("scene.sun")
                && let Some(sun) = values.get::<Sun>(*source)
            {
                if world.get::<DirectionalLight>(entity).is_none()
                    || !self.scene_lights.contains(&entity)
                {
                    return Err("Projected scene light unavailable".into());
                }
                lights.push((entity, kedra_scene::rendering::sun(sun), sun.clone()));
            }
        }
        let models = models::prepare(world, self, compiled, before)?;
        if let Err(error) = self.source_changed(world) {
            models.cancel(world);
            return Err(error);
        }
        models.apply(world, self);
        if let Some((entity, reference, value, camera, inspection)) = environment {
            if !inspection {
                // Validated above; no system or external callback runs between
                // preflight and this synchronous retained-camera replacement.
                kedra_scene::rendering::environment(world, camera, Some(&value))
                    .map_err(|error| error.to_string())?;
            }
            world.entity_mut(entity).insert(reference);
            self.scene_environment = Some(value);
        }
        for (entity, handle, mesh, bounds, source) in meshes {
            if let Some(mut current) = world.resource_mut::<Assets<Mesh>>().get_mut(&handle) {
                *current = mesh;
            }
            world.entity_mut(entity).insert((source, bounds));
        }
        for (entity, handle, material, source) in materials {
            if let Some(mut current) = world
                .resource_mut::<Assets<StandardMaterial>>()
                .get_mut(&handle)
            {
                *current = material;
            }
            world.entity_mut(entity).insert(source);
        }
        for (entity, light, source) in lights {
            world.entity_mut(entity).insert((light, source));
        }
        Ok(())
    }
}
