//! One bounded native mesh-bounds path for initial framing and scoped selection.
use bevy::{
    camera::primitives::{Aabb, MeshAabb},
    prelude::*,
};
use std::collections::HashMap;
/// Measure native mesh bounds below a preview root. Empty geometry returns None;
/// nonfinite/oversized transforms and bounded ancestry/traversal refuse. The
/// measurement-local mesh cache cannot survive asset replacement or removal.
pub fn measure(world: &World, root: Entity) -> Result<Option<(Vec3, Vec3)>, String> {
    let mut parents = Vec::new();
    let mut parent = world.get::<ChildOf>(root).map(ChildOf::parent);
    while let Some(entity) = parent {
        if parents.len() >= 128 {
            return Err("Presentation ancestry exceeds 128 nodes".into());
        }
        parents.push(entity);
        parent = world.get::<ChildOf>(entity).map(ChildOf::parent);
    }
    let parent = parents
        .into_iter()
        .rev()
        .fold(Mat4::IDENTITY, |matrix, entity| {
            matrix
                * world
                    .get::<Transform>(entity)
                    .map_or(Mat4::IDENTITY, Transform::to_matrix)
        });
    let mut stack = vec![(root, parent)];
    let mut count = 0;
    let mut minimum = Vec3::splat(f32::INFINITY);
    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
    // Measurement-local only: asset replacement/removal cannot leave stale bounds
    // in a later framing or selection request. Imported per-instance bounds win.
    let mut mesh_bounds = HashMap::new();
    while let Some((entity, parent)) = stack.pop() {
        count += 1;
        if count > 4096 {
            return Err("Presentation bounds exceed the 4096-node budget".into());
        }
        let transform = parent
            * world
                .get::<Transform>(entity)
                .map_or(Mat4::IDENTITY, Transform::to_matrix);
        if !transform.is_finite() {
            return Err("Composed spatial transform is nonfinite".into());
        }
        if let Some(mesh) = world.get::<Mesh3d>(entity) {
            let bounds = if let Some(native) = world.get::<Aabb>(entity) {
                *native
            } else {
                *mesh_bounds
                    .entry(mesh.0.id())
                    .or_insert_with(|| {
                        world
                            .get_resource::<Assets<Mesh>>()
                            .and_then(|meshes| meshes.get(&mesh.0))
                            .and_then(MeshAabb::get_aabb)
                    })
                    .as_ref()
                    .ok_or("Model mesh has no finite bounds")?
            };
            let half = Vec3::from(bounds.half_extents);
            let origin = Vec3::from(bounds.center);
            for x in [-1., 1.] {
                for y in [-1., 1.] {
                    for z in [-1., 1.] {
                        let point = transform.transform_point3(origin + half * Vec3::new(x, y, z));
                        if !point.is_finite() || point.abs().max_element() > 1_000_000. {
                            return Err(
                                "Composed scene exceeds the finite one-million-unit viewing bounds"
                                    .into(),
                            );
                        }
                        minimum = minimum.min(point);
                        maximum = maximum.max(point);
                    }
                }
            }
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter().map(|child| (child, transform)));
        }
    }
    Ok(minimum.is_finite().then_some((minimum, maximum)))
}
