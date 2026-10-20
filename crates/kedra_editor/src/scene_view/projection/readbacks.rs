//! Native recipients outlive logical readiness and retiring camera hierarchies.
use super::*;
use bevy::render::view::screenshot::{Captured, Capturing};

// Persistent request accounting, including cancelled recipients. Eight native
// readbacks bound buffer/task costs even when many epochs reuse one target.
#[derive(Component)]
pub(super) struct SceneCapture;

#[derive(Component)]
struct RetainedTarget(Handle<Image>);

pub(super) fn admit(world: &mut World) -> Result<(), String> {
    if world
        .query_filtered::<Entity, With<SceneCapture>>()
        .iter(world)
        .count()
        >= 8
    {
        return Err("Scene View native readback capacity exhausted, including cancelled captures awaiting completion; wait for the renderer or restart the host; sources were kept".into());
    }
    Ok(())
}

pub(super) fn cancel(world: &mut World, entity: Entity) {
    let in_flight =
        world.get::<Capturing>(entity).is_some() && world.get::<Captured>(entity).is_none();
    if let Ok(mut recipient) = world.get_entity_mut(entity) {
        // Reject callbacks immediately, even while the old camera still exists.
        recipient.remove::<Readback>();
        if in_flight {
            warn!(
                "Cancelled Scene View readback awaits native completion; recipient retained and counted against admission"
            );
        } else {
            recipient.despawn();
        }
    }
}

pub(super) fn release_image(world: &mut World, target: Handle<Image>) {
    let recipients: Vec<_> = world
        .query_filtered::<(Entity, &Screenshot), (With<Capturing>, Without<Captured>)>()
        .iter(world)
        .filter_map(|(entity, screenshot)| match &screenshot.0 {
            RenderTarget::Image(image) if image.handle.id() == target.id() => Some(entity),
            _ => None,
        })
        .collect();
    if recipients.is_empty() {
        world.resource_mut::<Assets<Image>>().remove(target.id());
    } else {
        for entity in recipients {
            if world.get::<RetainedTarget>(entity).is_none() {
                world
                    .entity_mut(entity)
                    .insert(RetainedTarget(target.clone()))
                    .observe(reclaim_target);
            }
        }
    }
}

fn reclaim_target(
    event: On<Despawn<RetainedTarget>>,
    targets: Query<(Entity, &RetainedTarget)>,
    mut images: ResMut<Assets<Image>>,
) {
    let Ok((_, target)) = targets.get(event.entity) else {
        return;
    };
    // Multiple lighting epochs may retain the same image. The final native
    // recipient releases it; no timer is evidence of safe GPU completion.
    if !targets
        .iter()
        .any(|(entity, other)| entity != event.entity && other.0.id() == target.0.id())
    {
        images.remove(target.0.id());
    }
}

pub(super) fn retained_pixels(world: &mut World) -> u64 {
    let ids: std::collections::HashSet<_> = world
        .query::<&RetainedTarget>()
        .iter(world)
        .map(|target| target.0.id())
        .collect();
    let images = world.resource::<Assets<Image>>();
    ids.into_iter()
        .filter_map(|id| images.get(id))
        .map(|image| u64::from(image.width()) * u64::from(image.height()))
        .sum()
}

pub(super) fn retiring_pixels(world: &mut World, target: Option<&Handle<Image>>) -> u64 {
    let Some(target) = target else {
        return 0;
    };
    let retained = world
        .query::<&RetainedTarget>()
        .iter(world)
        .any(|other| other.0.id() == target.id());
    let in_flight = world.query_filtered::<&Screenshot, (With<Capturing>, Without<Captured>)>().iter(world)
        .any(|screenshot| matches!(&screenshot.0, RenderTarget::Image(image) if image.handle.id() == target.id()));
    if in_flight && !retained {
        world
            .resource::<Assets<Image>>()
            .get(target)
            .map_or(0, |image| {
                u64::from(image.width()) * u64::from(image.height())
            })
    } else {
        0
    }
}
