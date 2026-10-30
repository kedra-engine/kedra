//! Accepted value reprojection retains cameras, owned assets and native source nodes.
use super::*;
impl View {
    pub(crate) fn fail_source(&mut self, world: &mut World, graph: &Arc<ResolvedGraph>) {
        self.retire(world);
        self.compiled = None;
        self.requested_source = Some(graph.clone());
    }
    pub(crate) fn refresh_environment(&mut self, world: &mut World) -> Result<(), String> {
        if let Some(projection) = &mut self.projection {
            projection.refresh_environment(world)?;
        }
        Ok(())
    }
    #[cfg(any(debug_assertions, test))]
    pub(super) fn spatial_values(&self, world: &World) -> Vec<serde_json::Value> {
        let Some(projection) = &self.projection else {
            return Vec::new();
        };
        projection.nodes.iter().filter_map(|(address, entity)| {
            let material = world.get::<MeshMaterial3d<StandardMaterial>>(*entity)
                .and_then(|handle| world.resource::<Assets<StandardMaterial>>().get(&handle.0));
            let light = world.get::<DirectionalLight>(*entity);
            let model = world.get::<kedra_scene::spatial::Model>(*entity);
            if material.is_none() && light.is_none() && model.is_none() { return None; }
            let bounds = world.get::<bevy::camera::primitives::Aabb>(*entity);
            Some(serde_json::json!({
                "node": address,
                "mesh_half_extents": bounds.map(|bounds| Vec3::from(bounds.half_extents).to_array()),
                "material": material.map(|material| {
                    let color = material.base_color.to_srgba();
                    serde_json::json!({"rgba":[color.red,color.green,color.blue,color.alpha],"metallic":material.metallic,"roughness":material.perceptual_roughness})
                }),
                "illuminance": light.map(|light| light.illuminance),
                "model": model.map(|model| serde_json::json!({
                    "path": model.scene.path().map(ToString::to_string),
                    "materials": projection.models.get(entity).map(|instance| instance.material_colors(world)),
                })),
            }))
        }).collect()
    }
    pub(crate) fn refresh_source(
        &mut self,
        world: &mut World,
        graph: &Arc<ResolvedGraph>,
        source: &kedra_authoring::persistence::SourceSnapshot,
    ) -> Result<(), String> {
        if self.source_removed {
            self.prepare(world, graph, source, self.source_layer);
            return Ok(());
        }
        let Some(previous) = &self.compiled else {
            if self.requested_source.as_deref() != Some(graph.as_ref()) {
                self.prepare(world, graph, source, self.source_layer);
            }
            return Ok(());
        };
        if previous.graph() == graph.as_ref() {
            if self.pending_source.take().is_some() {
                self.lifecycle.instantiated();
            }
            return Ok(());
        }
        let before = previous.graph();
        let structural = before.nodes.keys().ne(graph.nodes.keys())
            || before.root != graph.root
            || before.bindings != graph.bindings
            || before.ports != graph.ports
            || before.frames != graph.frames
            || before.aliases != graph.aliases
            || graph.nodes.iter().any(|(address, node)| {
                let Some(old) = before.nodes.get(address) else {
                    return true;
                };
                old.name != node.name
                    || old.parent != node.parent
                    || old.children != node.children
                    || old.components.keys().ne(node.components.keys())
                    || old
                        .components
                        .iter()
                        .filter(|(key, _)| !refreshable(key))
                        .ne(node.components.iter().filter(|(key, _)| !refreshable(key)))
            });
        let spatial = before
            .nodes
            .values()
            .chain(graph.nodes.values())
            .all(|node| !node.components.contains_key("ui.layout"));
        if structural && !spatial {
            self.prepare(world, graph, source, self.source_layer);
            return Ok(());
        }
        let Some(projection) = &mut self.projection else {
            return Ok(());
        };
        let registry = world.resource::<AppTypeRegistry>().clone();
        let manager = world.resource::<crate::model::ProjectManager>();
        let root = &manager.project.as_ref().ok_or("No project is open")?.assets;
        let compiled = Arc::new(binding::compile_preview(
            (**graph).clone(),
            &registry.read(),
            world.resource::<ComponentPolicies>(),
            world.resource::<AssetServer>(),
            source,
            root,
            world.get_resource::<binding::Delivery>(),
        )?);
        if !compiled
            .asset_status(world.resource::<AssetServer>())
            .map_err(|error| error.to_string())?
        {
            if self
                .pending_source
                .as_ref()
                .is_none_or(|pending| pending.graph() != graph.as_ref())
            {
                self.elapsed = 0.;
                self.lifecycle.begin()?;
            }
            // Retain strong native dependencies while the source compiler and
            // delivery admission are repeated against the current request each tick.
            // Undo/Redo owns symbolic paths, not a permanent loaded-asset cache.
            self.pending_source = Some(compiled);
            self.elapsed += world.resource::<Time>().delta_secs();
            if self.elapsed > 30. {
                return Err("Source refresh dependencies timed out".into());
            }
            return Ok(());
        }
        if projection.mounted() {
            if structural {
                projection.replace_spatial(world, compiled.clone())?;
                self.marker.clear(world);
                self.compiled = Some(compiled);
                self.pending_source = None;
                self.lifecycle.instantiated();
                return Ok(());
            }
            let mut changes = Vec::new();
            let ui = projection
                .passive
                .as_ref()
                .map(|view| view.prepare_refresh(world, compiled.clone(), 1.))
                .transpose()
                .map_err(|e| e.to_string())?;
            for (address, source_entity) in compiled.source_entities() {
                if graph.nodes[address]
                    .components
                    .contains_key("scene.transform")
                {
                    let entity = *projection
                        .nodes
                        .get(address)
                        .ok_or("Projected source node unavailable")?;
                    let transform = *compiled
                        .authoring_world()
                        .get::<Transform>(*source_entity)
                        .ok_or("Source Transform unavailable")?;
                    if world.get::<Transform>(entity).is_none() {
                        return Err("Projected Transform unavailable".into());
                    }
                    changes.push((entity, transform));
                }
            }
            projection.refresh_spatial_values(world, &compiled, before)?;
            if let Some(ui) = ui {
                ui.apply(world).map_err(|e| e.to_string())?;
            }
            for (entity, transform) in changes {
                if let Some(mut current) = world.get_mut::<Transform>(entity)
                    && *current != transform
                {
                    *current = transform;
                }
            }
            self.marker.clear(world);
            self.lifecycle.resized();
        }
        self.compiled = Some(compiled);
        self.pending_source = None;
        self.lifecycle.instantiated();
        Ok(())
    }
}

fn refreshable(key: &str) -> bool {
    matches!(
        key,
        "scene.transform"
            | "scene.primitive"
            | "scene.surface"
            | "scene.sun"
            | "scene.environment"
            | "scene.model"
            | "ui.caption"
            | "ui.layout"
            | "ui.classes"
            | "ui.image"
            | "ui.stylesheet"
            | "ui.background"
            | "ui.border"
            | "ui.shadow"
            | "ui.z"
            | "ui.global_z"
            | "ui.outline"
            | "ui.spin"
            | "ui.editable"
            | "ui.pickable"
            | "ui.modal"
            | "ui.id"
            | "ui.screen"
    )
}
