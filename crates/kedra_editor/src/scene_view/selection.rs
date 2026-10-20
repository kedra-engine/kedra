//! Native CPU mesh hits and reversible bounds markers. No material mutation,
//! source writes, or custom triangle intersection implementation.
use super::projection::Projection;
use bevy::{
    camera::visibility::RenderLayers,
    ecs::system::RunSystemOnce,
    gizmos::{GizmoAsset, retained::Gizmo},
    picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings},
    prelude::*,
};
use kedra_scene::document::NodeAddress;
use std::collections::HashSet;

pub(super) fn pick(
    world: &mut World,
    projection: &Projection,
    point: Vec2,
    area: Rect,
) -> Result<Option<NodeAddress>, String> {
    let camera = projection.camera().ok_or("Projection camera missing")?;
    let camera_data = world.get::<Camera>(camera).ok_or("Camera retired")?;
    let size = camera_data
        .logical_viewport_size()
        .ok_or("Camera target unmeasured")?;
    let position = (point - area.min) / area.size() * size;
    let transform = world
        .get::<GlobalTransform>(camera)
        .ok_or("Camera transform missing")?;
    let ray = camera_data
        .viewport_to_world(transform, position)
        .map_err(|error| error.to_string())?;
    cast(world, projection, ray)
}
pub(super) fn cast(
    world: &mut World,
    projection: &Projection,
    ray: Ray3d,
) -> Result<Option<NodeAddress>, String> {
    let mut eligible = HashSet::new();
    for entity in world.iter_entities() {
        if entity.contains::<Mesh3d>()
            && entity
                .get::<InheritedVisibility>()
                .is_some_and(|visibility| visibility.get())
            && entity
                .get::<RenderLayers>()
                .is_some_and(|layers| layers.intersects(&RenderLayers::layer(projection.layer())))
        {
            // CPU triangle casts cannot resolve alpha texture holes. Only opaque
            // surfaces participate; unsupported surfaces are not pixel-accurate.
            if entity
                .get::<MeshMaterial3d<StandardMaterial>>()
                .and_then(|material| {
                    world
                        .resource::<Assets<StandardMaterial>>()
                        .get(&material.0)
                })
                .is_some_and(|material| material.alpha_mode == AlphaMode::Opaque)
            {
                eligible.insert(entity.id());
            }
        }
    }
    let hit = world
        .run_system_once(move |mut raycast: MeshRayCast| {
            raycast
                .cast_ray(
                    ray,
                    &MeshRayCastSettings::default()
                        .with_filter(&|entity| eligible.contains(&entity)),
                )
                .first()
                .map(|(entity, _)| *entity)
        })
        .map_err(|error| error.to_string())?;
    let Some(mut entity) = hit else {
        return Ok(None);
    };
    for _ in 0..4096 {
        if let Some((address, _)) = projection
            .nodes
            .iter()
            .find(|(_, mapped)| **mapped == entity)
        {
            return Ok(Some(address.clone()));
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            break;
        };
        entity = parent.parent();
    }
    Err("Native hit has no source address in this projection".into())
}

#[derive(Default)]
pub(super) struct Marker {
    selected: Option<NodeAddress>,
    entity: Option<Entity>,
    asset: Option<Handle<GizmoAsset>>,
    ui: Option<Entity>,
}
impl Marker {
    pub(super) fn update(
        &mut self,
        world: &mut World,
        projection: &Projection,
        selected: Option<&NodeAddress>,
    ) -> Result<(), String> {
        let ui_source = selected
            .and_then(|address| projection.nodes.get(address))
            .copied()
            .filter(|entity| world.get::<Node>(*entity).is_some());
        if let Some(source) = ui_source {
            if self.selected.as_ref() != selected
                || self
                    .ui
                    .is_none_or(|entity| !world.entities().contains(entity))
            {
                self.clear(world);
                self.ui = super::ui_marker::spawn(world, source);
                self.selected = selected.cloned();
            }
            if let Some(camera) = projection.camera()
                && let Some(mut navigation) = world.get_mut::<super::camera::Navigation>(camera)
            {
                navigation.selection = None;
            }
            return Ok(());
        }
        if self.selected.as_ref() == selected {
            return Ok(());
        }
        self.clear(world);
        let bounds = selected
            .and_then(|address| projection.nodes.get(address))
            .map(|entity| super::bounds::measure(world, *entity))
            .transpose()?
            .flatten();
        if let Some(camera) = projection.camera()
            && let Some(mut navigation) = world.get_mut::<super::camera::Navigation>(camera)
        {
            navigation.selection = bounds;
        }
        if let Some((lo, hi)) = bounds {
            let mut lines = GizmoAsset::default();
            // Workbench's selection/focus color; this overlay is not source material.
            let color = super::ui_marker::SELECTION_COLOR;
            for axis in 0..3 {
                for a in [0., 1.] {
                    for b in [0., 1.] {
                        let mut start = lo;
                        let other = (axis + 1) % 3;
                        let last = (axis + 2) % 3;
                        start[other] = lo[other] + (hi[other] - lo[other]) * a;
                        start[last] = lo[last] + (hi[last] - lo[last]) * b;
                        let mut end = start;
                        end[axis] = hi[axis];
                        lines.line(start, end, color);
                    }
                }
            }
            let handle = world.resource_mut::<Assets<GizmoAsset>>().add(lines);
            self.entity = Some(
                world
                    .spawn((
                        Gizmo {
                            handle: handle.clone(),
                            depth_bias: -0.001,
                            ..default()
                        },
                        RenderLayers::layer(projection.layer()),
                    ))
                    .id(),
            );
            self.asset = Some(handle);
        }
        // A failed measurement must not poison the cache for an unchanged
        // selection; publish the cache only after marker/bounds publication.
        self.selected = selected.cloned();
        Ok(())
    }
    pub(super) fn clear(&mut self, world: &mut World) {
        if let Some(entity) = self.ui.take()
            && let Ok(entity) = world.get_entity_mut(entity)
        {
            entity.despawn();
        }
        if let Some(entity) = self.entity.take()
            && let Ok(entity) = world.get_entity_mut(entity)
        {
            entity.despawn();
        }
        if let Some(asset) = self.asset.take() {
            world
                .resource_mut::<Assets<GizmoAsset>>()
                .remove(asset.id());
        }
        self.selected = None;
    }
}
