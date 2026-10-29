//! Interruptible pointer alignment. Camera and orientation widget share one pose.
//! Bevy owns quaternion interpolation and the cubic-Bezier timing solver.
use super::{camera, orientation::Axis};
use bevy::prelude::*;
use kedra_ui::authored::components::UiMotion;

const DURATION: f32 = 0.2;

#[derive(Component)]
pub(super) struct Alignment {
    window: Entity,
    from: Quat,
    to: Transform,
    pivot: Vec3,
    radius: f32,
    elapsed: f32,
}

/// Resolve click intent before the pointer lease interrupts the current turn.
pub(crate) fn target(world: &World, camera: Entity, axis: Axis) -> Result<camera::Command, String> {
    let pose = world.get::<Transform>(camera).ok_or("Camera retired")?;
    if !pose.rotation.is_normalized() {
        return Err("Camera alignment requires a normalized rotation".into());
    }
    let direction = world
        .get::<Alignment>(camera)
        .map_or(*pose.back(), |motion| *motion.to.back());
    Ok(axis.command(direction.dot(axis.vector()) > 0.9999))
}

pub(crate) fn request(
    world: &mut World,
    camera: Entity,
    window: Entity,
    target: camera::Command,
) -> Result<(), String> {
    let from = *world.get::<Transform>(camera).ok_or("Camera retired")?;
    if !from.rotation.is_normalized()
        || !from.translation.is_finite()
        || !world
            .get::<Camera>(camera)
            .is_some_and(|camera| camera.is_active)
    {
        return Err("Camera alignment requires a finite active camera".into());
    }
    let pivot = world
        .get::<camera::Navigation>(camera)
        .ok_or("Spatial camera missing")?
        .pivot;
    if !world
        .get::<Window>(window)
        .is_some_and(|window| window.focused)
    {
        return Err("Camera alignment requires the owning focused window".into());
    }
    let radius = (from.translation - pivot).length().max(0.1);
    let to = camera::view_pose(pivot, radius, target)?;
    camera::cancel(world, camera);
    if world
        .get_resource::<UiMotion>()
        .is_some_and(|motion| motion.reduced)
    {
        *world.get_mut::<Transform>(camera).ok_or("Camera retired")? = to;
    } else {
        world.entity_mut(camera).insert(Alignment {
            window,
            from: from.rotation,
            to,
            pivot,
            radius,
            elapsed: 0.,
        });
    }
    Ok(())
}

pub(super) fn cancel(world: &mut World, camera: Entity) {
    if let Ok(mut camera) = world.get_entity_mut(camera) {
        camera.remove::<Alignment>();
    }
}
#[cfg(any(debug_assertions, test))]
pub(super) fn active(world: &World, camera: Entity) -> bool {
    world.get::<Alignment>(camera).is_some()
}
pub(crate) fn cancel_window(world: &mut World, window: Option<Entity>) {
    let cameras = world
        .query::<(Entity, &Alignment)>()
        .iter(world)
        .filter(|(_, motion)| window.is_none_or(|window| window == motion.window))
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    for camera in cameras {
        cancel(world, camera);
    }
}

pub(super) fn advance(
    time: Res<Time<Real>>,
    preference: Option<Res<UiMotion>>,
    windows: Query<&Window>,
    mut cameras: Query<(Entity, &Camera, &mut Transform, &mut Alignment)>,
    mut commands: Commands,
    mut demand: Option<ResMut<kedra_wakeup::WakeDemand>>,
) {
    for (entity, camera, mut pose, mut motion) in &mut cameras {
        if !camera.is_active
            || !windows
                .get(motion.window)
                .is_ok_and(|window| window.focused)
        {
            commands.entity(entity).remove::<Alignment>();
            continue;
        }
        motion.elapsed += time.delta_secs();
        if motion.elapsed >= DURATION
            || preference
                .as_ref()
                .is_some_and(|preference| preference.reduced)
        {
            *pose = motion.to;
            commands.entity(entity).remove::<Alignment>();
            continue;
        }
        let curve = CubicSegment::new_bezier_easing((0.77, 0.), (0.175, 1.));
        if let Some(demand) = demand.as_deref_mut() {
            demand.active();
        }
        let amount = curve.ease((motion.elapsed / DURATION).clamp(0., 1.));
        pose.rotation = motion.from.slerp(motion.to.rotation, amount);
        pose.translation = motion.pivot + *pose.back() * motion.radius;
    }
}
