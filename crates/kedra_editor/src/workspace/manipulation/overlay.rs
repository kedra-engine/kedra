//! Retained per-viewport handles. Rendering and picking share the same polylines.
use super::*;
use bevy::{
    camera::visibility::RenderLayers,
    gizmos::{GizmoAsset, retained::Gizmo},
};

#[derive(Clone, PartialEq)]
pub(super) struct Geometry {
    origin: Vec3,
    length: f32,
    directions: [Vec3; 3],
    camera_right: Vec3,
    center: Vec2,
    mode: Mode,
    handles: Vec<HandleGeometry>,
    layer: usize,
}
#[derive(Clone, PartialEq)]
struct HandleGeometry {
    axis: usize,
    points: Vec<Vec3>,
    screen: Vec<Vec2>,
}
impl Geometry {
    pub(super) fn hit(&self, point: Vec2) -> Option<usize> {
        if self.mode == Mode::Scale && point.distance(self.center) <= 7. {
            return Some(3);
        }
        self.handles
            .iter()
            .filter_map(|handle| {
                let distance = handle
                    .screen
                    .windows(2)
                    .map(|p| {
                        bevy::gizmos::transform_gizmo::point_to_segment_dist(point, p[0], p[1])
                    })
                    .min_by(f32::total_cmp)?;
                (distance <= 7.).then_some((handle.axis, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(axis, _)| axis)
    }
    pub(super) fn begin(
        &self,
        baseline: Transform,
        parent: Mat4,
        axis: usize,
        ray: Ray3d,
        snap: bool,
    ) -> Result<Constraint, &'static str> {
        match self.mode {
            Mode::Move => kedra_gizmo::MoveDrag::begin(
                baseline,
                parent,
                Vec3::AXES[axis],
                ray,
                snap.then_some(1.),
            )
            .map(Constraint::Move),
            Mode::Rotate => kedra_gizmo::RotateDrag::begin(
                baseline,
                parent,
                axis,
                ray,
                snap.then_some(15_f32.to_radians()),
            )
            .map(Constraint::Rotate),
            Mode::Scale => kedra_gizmo::ScaleDrag::begin(
                baseline,
                parent,
                (axis < 3).then_some(axis),
                if axis < 3 {
                    self.directions[axis]
                } else {
                    self.camera_right
                },
                self.length,
                ray,
                snap.then_some(0.1),
            )
            .map(Constraint::Scale),
        }
    }
}
pub(super) fn geometry(
    world: &World,
    camera: Entity,
    area: Rect,
    (local, parent, mode): (Transform, Mat4, Mode),
    layer: usize,
) -> Option<Geometry> {
    let camera_data = world.get::<Camera>(camera)?;
    let transform = world.get::<GlobalTransform>(camera)?;
    let size = camera_data.logical_viewport_size()?;
    let origin = parent.transform_point3(local.translation);
    let project = |point| {
        camera_data
            .world_to_viewport(transform, point)
            .ok()
            .map(|point| area.min + point / size * area.size())
    };
    let center = project(origin)?;
    let unit = project(origin + transform.right().as_vec3())?.distance(center);
    if !unit.is_finite() || unit < 1e-5 {
        return None;
    }
    let length = 76. / unit;
    let frame = parent * Mat4::from_rotation_translation(local.rotation, local.translation);
    let basis = Vec3::AXES.map(|axis| frame.transform_vector3(axis));
    let max_length = basis.iter().map(|v| v.length()).fold(0_f32, f32::max);
    if !max_length.is_finite() || max_length < 1e-6 {
        return None;
    }
    let directions = if mode == Mode::Move {
        Vec3::AXES
    } else {
        basis.map(|v| v.normalize_or_zero())
    };
    let mut handles = Vec::new();
    for axis in 0..3 {
        let points = if mode == Mode::Rotate {
            let u = basis[(axis + 1) % 3] / max_length * length;
            let v = basis[(axis + 2) % 3] / max_length * length;
            let normal = u.cross(v).try_normalize()?;
            let ray = camera_data
                .viewport_to_world(transform, (center - area.min) / area.size() * size)
                .ok()?;
            if normal.dot(*ray.direction).abs() < 0.08 {
                continue;
            }
            (0..=64)
                .map(|i| {
                    let angle = i as f32 * std::f32::consts::TAU / 64.;
                    origin + u * angle.cos() + v * angle.sin()
                })
                .collect::<Vec<_>>()
        } else {
            vec![
                origin + directions[axis] * length * 0.18,
                origin + directions[axis] * length,
            ]
        };
        let Some(screen) = points
            .iter()
            .map(|p| project(*p))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        if mode != Mode::Rotate && screen[0].distance(screen[1]) < 12. {
            continue;
        }
        handles.push(HandleGeometry {
            axis,
            points,
            screen,
        });
    }
    Some(Geometry {
        origin,
        length,
        directions,
        camera_right: transform.right().as_vec3(),
        center,
        mode,
        handles,
        layer,
    })
}
#[derive(Resource)]
struct Overlay {
    entity: Entity,
    handle: Handle<GizmoAsset>,
    geometry: Geometry,
    axis: Option<usize>,
}
#[cfg(debug_assertions)]
pub(super) fn diagnostics(world: &World) -> serde_json::Value {
    world.get_resource::<Overlay>().map(|overlay| serde_json::json!(
        overlay.geometry.handles.iter().map(|handle| {
            serde_json::json!({"axis":handle.axis,"start":handle.screen[0].to_array(),"end":handle.screen[handle.screen.len()-1].to_array(),
                "points":handle.screen.iter().map(|p|p.to_array()).collect::<Vec<_>>(), "center":overlay.geometry.center.to_array()})
        }).collect::<Vec<_>>()
    )).unwrap_or(serde_json::Value::Null)
}
pub(super) fn clear(world: &mut World) {
    if let Some(overlay) = world.remove_resource::<Overlay>() {
        if let Ok(entity) = world.get_entity_mut(overlay.entity) {
            entity.despawn();
        }
        if let Some(mut assets) = world.get_resource_mut::<Assets<GizmoAsset>>() {
            assets.remove(overlay.handle.id());
        }
    }
}
pub(super) fn draw(world: &mut World, geometry: Geometry, active: Option<usize>) {
    if world
        .get_resource::<Overlay>()
        .is_some_and(|o| !world.entities().contains(o.entity))
    {
        clear(world);
    }
    if world.get_resource::<Overlay>().is_some_and(|o| {
        o.geometry == geometry && o.axis == active && world.entities().contains(o.entity)
    }) {
        return;
    }
    let mut lines = GizmoAsset::default();
    let colors = [
        Color::srgb(0.95, 0.2, 0.2),
        Color::srgb(0.25, 0.85, 0.35),
        Color::srgb(0.25, 0.5, 1.),
    ];
    for handle in &geometry.handles {
        let color = if active == Some(handle.axis) {
            Color::srgb(1., 0.85, 0.2)
        } else {
            colors[handle.axis]
        };
        if geometry.mode == Mode::Move {
            lines
                .arrow(handle.points[0], handle.points[1], color)
                .with_tip_length(geometry.length * 0.15);
        } else {
            lines.linestrip(handle.points.iter().copied(), color);
            if geometry.mode == Mode::Scale {
                lines.cube(
                    Transform::from_translation(handle.points[1])
                        .with_scale(Vec3::splat(geometry.length * 0.09)),
                    color,
                );
            }
        }
    }
    if geometry.mode == Mode::Scale {
        let color = if active == Some(3) {
            Color::srgb(1., 0.85, 0.2)
        } else {
            Color::srgb(0.9, 0.9, 0.9)
        };
        lines.cube(
            Transform::from_translation(geometry.origin)
                .with_scale(Vec3::splat(geometry.length * 0.09)),
            color,
        );
    }
    if let Some(mut overlay) = world.remove_resource::<Overlay>() {
        if let Some(mut assets) = world.get_resource_mut::<Assets<GizmoAsset>>()
            && let Some(mut asset) = assets.get_mut(&overlay.handle)
        {
            *asset = lines;
        }
        if let Ok(mut entity) = world.get_entity_mut(overlay.entity) {
            entity.insert(RenderLayers::layer(geometry.layer));
        }
        overlay.geometry = geometry;
        overlay.axis = active;
        world.insert_resource(overlay);
    } else {
        let handle = world.resource_mut::<Assets<GizmoAsset>>().add(lines);
        let entity = world
            .spawn((
                Gizmo {
                    handle: handle.clone(),
                    line_config: bevy::gizmos::config::GizmoLineConfig {
                        width: 3.,
                        ..default()
                    },
                    depth_bias: -1.,
                },
                RenderLayers::layer(geometry.layer),
            ))
            .id();
        world.insert_resource(Overlay {
            entity,
            handle,
            geometry,
            axis: active,
        });
    }
}
