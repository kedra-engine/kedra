//! Per-presentation camera policy. Native PanOrbitCamera owns orbit/pan/zoom;
//! this adapter owns bounded framing, explicit views and window-scoped flight.
use bevy::{camera::Projection, camera_controller::pan_orbit_camera::prelude::*, prelude::*};

#[derive(Component)]
pub(crate) struct Navigation {
    pub pivot: Vec3,
    pub speed: f32,
    pub bounds: Option<(Vec3, Vec3)>,
    pub selection: Option<(Vec3, Vec3)>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Command {
    FrameAll,
    FrameSelection,
    Projection,
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
}

pub(super) fn install(app: &mut App) {
    // The optional plugin also rewrites near planes and camera positions for
    // orthographic depth precision. Editor framing owns those policies here.
    app.add_systems(
        PreUpdate,
        (PanOrbitCamera::update_camera_positions, sync_pivot)
            .chain()
            .after(bevy::picking::PickingSystems::Last),
    );
}
fn sync_pivot(mut cameras: Query<(&Transform, &mut Navigation, &mut PanOrbitCamera)>) {
    for (transform, mut navigation, mut controller) in &mut cameras {
        if let Some(anchor) = controller.anchor_view_space() {
            navigation.pivot = transform.transform_point(anchor.as_vec3());
        }
        // InputQueue expects zero samples too; a wheel pulse must not become an
        // indefinitely repeated zoom velocity in the next frame.
        controller.send_zoom_input(0.);
    }
}
pub(crate) fn attach(world: &mut World, camera: Entity, bounds: Option<(Vec3, Vec3)>) {
    let radius = bounds.map_or(2., |(lo, hi)| (hi - lo).length().max(1.) / 2.);
    let pivot = bounds.map_or(Vec3::ZERO, |(lo, hi)| (lo + hi) / 2.);
    world.entity_mut(camera).insert((
        Navigation {
            pivot,
            speed: radius.max(0.1),
            bounds,
            selection: None,
        },
        PanOrbitCamera {
            zoom_limits: bevy::camera_controller::pan_orbit_camera::controller::zoom::ZoomLimits {
                max_size_per_pixel: 1000.,
                ..default()
            },
            last_anchor_depth: f64::from(radius * 3.5),
            // Editing gestures must stop on release, not continue as momentum.
            smoothing:
                bevy::camera_controller::pan_orbit_camera::controller::smoothing::Smoothing {
                    pan: std::time::Duration::from_millis(1),
                    orbit: std::time::Duration::from_millis(1),
                    zoom: std::time::Duration::from_millis(1),
                },
            ..default()
        },
    ));
}
pub(crate) fn cancel(world: &mut World, camera: Entity) {
    if let Some(mut controller) = world.get_mut::<PanOrbitCamera>(camera) {
        controller.current_motion=bevy::camera_controller::pan_orbit_camera::controller::motion::CurrentMotion::Stationary;
    }
}
pub(crate) fn start(world: &mut World, camera: Entity, pan: bool) {
    let Some(nav) = world.get::<Navigation>(camera) else {
        return;
    };
    let Some(transform) = world.get::<Transform>(camera) else {
        return;
    };
    let anchor = transform
        .to_matrix()
        .inverse()
        .transform_point3(nav.pivot)
        .as_dvec3();
    if let Some(mut controller) = world.get_mut::<PanOrbitCamera>(camera) {
        if pan {
            controller.start_pan(Some(anchor));
        } else {
            controller.start_orbit(Some(anchor));
        }
    }
}
pub(crate) fn drag(world: &mut World, camera: Entity, delta: Vec2) {
    if delta.is_finite()
        && let Some(mut controller) = world.get_mut::<PanOrbitCamera>(camera)
    {
        controller.send_screenspace_input(delta);
    }
}
pub(crate) fn zoom(world: &mut World, camera: Entity, amount: f32) {
    if !amount.is_finite() {
        return;
    }
    if let Some(mut controller) = world.get_mut::<PanOrbitCamera>(camera) {
        controller.start_zoom(None);
        controller.send_zoom_input(amount.clamp(-100., 100.));
    }
}
pub(crate) fn fly(world: &mut World, camera: Entity, delta: Vec2, axis: Vec3, fast: bool, dt: f32) {
    if !delta.is_finite() || !axis.is_finite() || !dt.is_finite() {
        return;
    }
    let Some(speed) = world.get::<Navigation>(camera).map(|nav| nav.speed) else {
        return;
    };
    let depth = world
        .get::<Navigation>(camera)
        .zip(world.get::<Transform>(camera))
        .map_or(1., |(navigation, transform)| {
            (navigation.pivot - transform.translation).length().max(0.1)
        });
    let (position, forward) = {
        let Some(mut transform) = world.get_mut::<Transform>(camera) else {
            return;
        };
        let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        transform.rotation = Quat::from_euler(
            EulerRot::YXZ,
            yaw - delta.x * 0.004,
            (pitch - delta.y * 0.004).clamp(-1.55, 1.55),
            0.,
        );
        let shift =
            (*transform.right() * axis.x + Vec3::Y * axis.y + *transform.forward() * axis.z)
                .normalize_or_zero()
                * speed
                * if fast { 4. } else { 1. }
                * dt.clamp(0., 0.05);
        let next = transform.translation + shift;
        if next.is_finite() && next.abs().max_element() <= 100_000_000. {
            transform.translation = next;
        }
        (transform.translation, *transform.forward())
    };
    if let Some(mut nav) = world.get_mut::<Navigation>(camera) {
        nav.pivot = position + forward * depth;
    }
}
pub(crate) fn speed(world: &mut World, camera: Entity, amount: f32) {
    if amount.is_finite()
        && let Some(mut nav) = world.get_mut::<Navigation>(camera)
    {
        nav.speed = (nav.speed * (amount.clamp(-100., 100.) * 0.12).exp()).clamp(0.001, 100_000.);
    }
}
pub(crate) fn command(world: &mut World, camera: Entity, command: Command) -> Result<(), String> {
    cancel(world, camera);
    let nav = world
        .get::<Navigation>(camera)
        .ok_or("This target has no spatial camera")?;
    let pivot = nav.pivot;
    let bounds = nav.bounds;
    let selection = nav.selection;
    let transform = *world.get::<Transform>(camera).ok_or("Camera retired")?;
    let distance = (transform.translation - pivot).length().max(0.1);
    match command {
        Command::FrameAll => frame(world, camera, bounds),
        Command::FrameSelection => frame(world, camera, selection),
        Command::Projection => {
            let projection = world
                .get::<Projection>(camera)
                .ok_or("Camera projection missing")?;
            let (next, distance) = match projection {
                Projection::Perspective(p) => (
                    Projection::Orthographic(OrthographicProjection {
                        scale: 1.,
                        scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                            viewport_height: 2. * distance * (p.fov / 2.).tan(),
                        },
                        far: 2_000_000.,
                        ..OrthographicProjection::default_3d()
                    }),
                    distance,
                ),
                Projection::Orthographic(p) => {
                    let height = match p.scaling_mode {
                        bevy::camera::ScalingMode::FixedVertical { viewport_height } => {
                            viewport_height * p.scale
                        }
                        _ => p.area.height(),
                    };
                    let perspective = PerspectiveProjection {
                        far: 2_000_000.,
                        ..default()
                    };
                    let distance = (height / (2. * (perspective.fov / 2.).tan())).max(0.1);
                    (Projection::Perspective(perspective), distance)
                }
                _ => return Err("Unsupported editor projection".into()),
            };
            world.entity_mut(camera).insert(next);
            let mut pose = world.get_mut::<Transform>(camera).ok_or("Camera retired")?;
            pose.translation = pivot - *pose.forward() * distance;
            if let Some(mut controller) = world.get_mut::<PanOrbitCamera>(camera) {
                controller.last_anchor_depth = f64::from(distance);
            }
            Ok(())
        }
        command => {
            let (direction, up) = match command {
                Command::Front => (Vec3::Z, Vec3::Y),
                Command::Back => (Vec3::NEG_Z, Vec3::Y),
                Command::Left => (Vec3::NEG_X, Vec3::Y),
                Command::Right => (Vec3::X, Vec3::Y),
                Command::Top => (Vec3::Y, Vec3::NEG_Z),
                Command::Bottom => (Vec3::NEG_Y, Vec3::Z),
                _ => unreachable!(),
            };
            *world.get_mut::<Transform>(camera).ok_or("Camera retired")? =
                Transform::from_translation(pivot + direction * distance).looking_at(pivot, up);
            Ok(())
        }
    }
}
pub(crate) fn frame(
    world: &mut World,
    camera: Entity,
    bounds: Option<(Vec3, Vec3)>,
) -> Result<(), String> {
    let (lo, hi) = bounds.ok_or("No finite renderable bounds for this target")?;
    let radius = ((hi - lo).length() / 2.).max(0.05);
    let center = (lo + hi) / 2.;
    if !radius.is_finite() || !center.is_finite() {
        return Err("Bounds are nonfinite".into());
    }
    let aspect = world
        .get::<Camera>(camera)
        .and_then(Camera::logical_viewport_size)
        .map_or(1., |size| size.x / size.y.max(1.))
        .clamp(0.01, 100.);
    let distance = {
        let mut projection = world
            .get_mut::<Projection>(camera)
            .ok_or("Camera projection missing")?;
        match &mut *projection {
            Projection::Perspective(p) => {
                let angle = (p.fov / 2.).min(((p.fov / 2.).tan() * aspect).atan());
                radius / angle.sin() * 1.15
            }
            Projection::Orthographic(p) => {
                p.scale = 1.;
                p.scaling_mode = bevy::camera::ScalingMode::FixedVertical {
                    viewport_height: radius * 2.3 / aspect.min(1.),
                };
                radius * 3.5
            }
            _ => return Err("Unsupported editor projection".into()),
        }
    };
    {
        let mut transform = world.get_mut::<Transform>(camera).ok_or("Camera retired")?;
        transform.translation = center - *transform.forward() * distance;
    }
    world
        .get_mut::<Navigation>(camera)
        .ok_or("Camera navigation missing")?
        .pivot = center;
    if let Some(mut controller) = world.get_mut::<PanOrbitCamera>(camera) {
        controller.last_anchor_depth = f64::from(distance);
    }
    Ok(())
}
