//! Renderer readback with bounded completion, explicit target and artifact ownership.
use bevy::{
    prelude::*,
    render::view::screenshot::{Captured, Capturing, Screenshot, ScreenshotCaptured},
};
use bevy_remote::BrpResult;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

/// Host-owned artifact directory, unrelated to project authoring sources.
#[derive(Resource)]
pub struct ArtifactRoot(pub PathBuf);

/// Published renderer artifact. Uniformity is diagnostic, not visual acceptance.
pub struct FrameArtifact {
    /// File published without replacing existing artifacts.
    pub path: PathBuf,
    /// Physical image dimensions.
    pub size: [u32; 2],
    /// All pixels have the same RGBA value.
    pub uniform: bool,
}

/// Publish a completed renderer readback to an explicitly host-owned artifact
/// directory. Never captures a window, replaces a file or writes project sources.
/// Empty/undecodable images and filesystem failures return an error. The caller
/// owns readiness, window lifecycle and the interpretation of uniform pixels.
pub fn save_frame(root: &Path, image: Image, id: Uuid) -> Result<FrameArtifact, String> {
    let image = image
        .try_into_dynamic()
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err("Renderer returned an empty image".into());
    }
    let uniform = image
        .pixels()
        .next()
        .is_some_and(|first| image.pixels().all(|pixel| pixel == first));
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let path = root.join(format!("{id}.png"));
    let mut output = tempfile::NamedTempFile::new_in(root).map_err(|e| e.to_string())?;
    image::DynamicImage::ImageRgba8(image)
        .write_to(output.as_file_mut(), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    output.as_file().sync_all().map_err(|e| e.to_string())?;
    output.persist_noclobber(&path).map_err(|e| e.to_string())?;
    Ok(FrameArtifact {
        path,
        size: [width, height],
        uniform,
    })
}
/// Shared native capture service. Ready same-target requests share an unstarted
/// renderer readback but publish separate artifacts/receipts. Host screenshots can supply
/// that readback and are never retired by this service. Capturing does not activate
/// the target window or change input focus. This is not OS automation.
pub struct CapturePlugin;
impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingCaptures>()
            // Feature-owned Update/PostUpdate screenshots are visible before we
            // choose a readback. Adopt them instead of spawning a duplicate.
            .add_systems(Last, advance)
            .add_observer(captured)
            .add_observer(screenshot_added);
    }
}
struct Pending {
    window: Entity,
    requested: u32,
    readback: Option<Readback>,
}
#[derive(Clone, Copy)]
struct Readback {
    entity: Entity,
    owned: bool,
}
#[derive(Resource, Default)]
struct PendingCaptures(BTreeMap<Uuid, Pending>);
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    session: String,
    id: Uuid,
    window: u64,
}
pub(crate) fn request(In(value): In<Option<Value>>, world: &mut World) -> BrpResult {
    let original = value.clone().unwrap_or(Value::Null);
    let request: Request = crate::params(value)?;
    if let Some(receipt) = crate::input::retry(world, &request.session, request.id, &original)? {
        return Ok(receipt);
    }
    let window =
        crate::input::target(world, &request.session, request.window).map_err(crate::invalid)?;
    if !world.contains_resource::<ArtifactRoot>() {
        return Err(crate::invalid(
            "Capture artifact directory is not configured",
        ));
    }
    if world.resource::<crate::input::InputQueue>().busy() {
        return Err(crate::invalid(
            "Wait for input delivery before requesting a capture",
        ));
    }
    if world.resource::<PendingCaptures>().0.len() >= 8
        || world
            .query_filtered::<Entity, With<CaptureOwned>>()
            .iter(world)
            .count()
            >= 8
    {
        return Err(crate::invalid(
            "Capture queue is full, including unfinished native readbacks; wait for renderer completion or restart the host",
        ));
    }
    let admitted = world
        .resource_mut::<crate::operations::Operations>()
        .admit(request.id, &original)
        .map_err(crate::invalid)?;
    if admitted {
        let frame = world.resource::<bevy::diagnostic::FrameCount>().0;
        world.resource_mut::<PendingCaptures>().0.insert(
            request.id,
            Pending {
                window,
                requested: frame,
                readback: None,
            },
        );
    }
    serde_json::to_value(
        world
            .resource::<crate::operations::Operations>()
            .get(request.id),
    )
    .map_err(bevy_remote::BrpError::internal)
}
fn advance(world: &mut World) {
    let frame = world.resource::<bevy::diagnostic::FrameCount>().0;
    let mut captures = world
        .remove_resource::<PendingCaptures>()
        .unwrap_or_default();
    let mut failed = Vec::new();
    for (&id, pending) in &mut captures.0 {
        if world.get::<Window>(pending.window).is_none()
            || frame.wrapping_sub(pending.requested) > 180
        {
            world
                .resource_mut::<crate::operations::Operations>()
                .finish(
                    id,
                    Err("Capture target retired or renderer readback timed out".into()),
                );
            failed.push(id);
            continue;
        }
        if frame.wrapping_sub(pending.requested) < 6 {
            continue;
        }
        if pending
            .readback
            .is_some_and(|readback| world.get::<Screenshot>(readback.entity).is_some())
        {
            continue;
        }
        // Bevy may retire a duplicate without an event. A lost entity is not an
        // in-flight readback: reselect a live target until the bounded deadline.
        let primary = world
            .query_filtered::<Entity, With<bevy::window::PrimaryWindow>>()
            .iter(world)
            .next();
        let target = Screenshot::window(pending.window).0.normalize(primary);
        let existing = world
            // Already extracted frames can predate this request's settling
            // interval. Bevy excludes them from duplicate-target detection too.
            .query_filtered::<(Entity, &Screenshot), (Without<Capturing>, Without<Captured>)>()
            .iter(world)
            .find_map(|(entity, screenshot)| {
                (screenshot.0.normalize(primary) == target).then_some(entity)
            });
        pending.readback = Some(match existing {
            Some(entity) => Readback {
                entity,
                owned: captures_owned(world, entity),
            },
            None => {
                if world
                    .query_filtered::<Entity, With<CaptureOwned>>()
                    .iter(world)
                    .count()
                    >= 8
                {
                    world.resource_mut::<crate::operations::Operations>().finish(id,
                        Err("Capture capacity exhausted by unfinished native readbacks; wait for completion or restart the host".into()));
                    failed.push(id);
                    continue;
                }
                let entity = world
                    .spawn((Screenshot::window(pending.window), CaptureOwned))
                    .id();
                Readback {
                    entity,
                    owned: true,
                }
            }
        });
    }
    for id in failed {
        if let Some(readback) = captures.0.remove(&id).and_then(|pending| pending.readback)
            && readback.owned
            && !captures.0.values().any(|pending| {
                pending
                    .readback
                    .is_some_and(|other| other.entity == readback.entity)
            })
        {
            // Capturing is native ownership, not a cancellable GPU task. Keep
            // the recipient until Bevy inserts Captured and cleans it in First.
            if world.get::<Capturing>(readback.entity).is_some()
                && world.get::<Captured>(readback.entity).is_none()
            {
                warn!(
                    "Cancelled capture still awaits native completion; recipient retained and counted against capture admission"
                );
            } else {
                world.despawn(readback.entity);
            }
        }
    }
    world.insert_resource(captures);
}

#[derive(Component)]
struct CaptureOwned;
fn captures_owned(world: &World, entity: Entity) -> bool {
    world.get::<CaptureOwned>(entity).is_some()
}
fn screenshot_added(event: On<Add<Screenshot>>, mut commands: Commands) {
    let entity = event.entity;
    commands.queue(move |world: &mut World| {
        // A host may enqueue its screenshot even after our Last selection. Yield
        // the unextracted owned request, never the host's entity or in-flight work.
        if captures_owned(world, entity)
            || world.get::<Capturing>(entity).is_some()
            || world.get::<Captured>(entity).is_some()
        {
            return;
        }
        let primary = world.query_filtered::<Entity, With<bevy::window::PrimaryWindow>>()
            .iter(world).next();
        let Some(target) = world.get::<Screenshot>(entity).and_then(|screenshot| screenshot.0.normalize(primary)) else { return; };
        let duplicates: Vec<_> = world
            .query_filtered::<(Entity, &Screenshot), (With<CaptureOwned>, Without<Capturing>, Without<Captured>)>()
            .iter(world)
            .filter_map(|(owned, screenshot)| (screenshot.0.normalize(primary).as_ref() == Some(&target)).then_some(owned))
            .collect();
        let Some(mut captures) = world.get_resource_mut::<PendingCaptures>() else { return; };
        for pending in captures.0.values_mut() {
            if pending.readback.is_some_and(|readback| duplicates.contains(&readback.entity)) {
                pending.readback = Some(Readback { entity, owned: false });
            }
        }
        for duplicate in duplicates {
            world.despawn(duplicate);
        }
    });
}
fn captured(event: On<ScreenshotCaptured>, pending: Res<PendingCaptures>, mut commands: Commands) {
    let entity = event.entity;
    if !pending.0.values().any(|pending| {
        pending
            .readback
            .is_some_and(|readback| readback.entity == entity)
    }) {
        return;
    }
    let image = event.image.clone();
    commands.queue(move |world: &mut World| {
        let ids: Vec<_> = world.resource::<PendingCaptures>().0.iter()
            .filter_map(|(&id, pending)| pending.readback.is_some_and(|readback| readback.entity == entity).then_some(id))
            .collect();
        for id in ids {
            let Some(pending) = world.resource_mut::<PendingCaptures>().0.remove(&id) else { continue; };
            let result = (|| -> Result<Value, String> {
                if world.get::<Window>(pending.window).is_none() {
                    return Err("Capture window retired before readback completed".into());
                }
                let artifact = save_frame(&world.resource::<ArtifactRoot>().0, image.clone(), id)?;
                Ok(json!({"path":artifact.path,"width":artifact.size[0],"height":artifact.size[1],"uniform":artifact.uniform,"window":pending.window.to_bits(),"frame":world.resource::<bevy::diagnostic::FrameCount>().0}))
            })();
            world.resource_mut::<crate::operations::Operations>().finish(id, result);
        }
    });
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
#[path = "capture/lifecycle_tests.rs"]
mod lifecycle_tests;

#[cfg(test)]
mod tests {
    use super::test_support::{admit, host, image};
    use super::*;
    #[test]
    fn same_window_requests_share_readback_and_both_receipts_complete() {
        let (mut app, _directory, window) = host();
        let ids = [
            admit(app.world_mut(), window),
            admit(app.world_mut(), window),
        ];
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = 6;
        advance(app.world_mut());
        let screenshots: Vec<_> = app
            .world_mut()
            .query_filtered::<Entity, With<Screenshot>>()
            .iter(app.world())
            .collect();
        assert_eq!(screenshots.len(), 1);
        app.world_mut().trigger(ScreenshotCaptured {
            entity: screenshots[0],
            image: image(),
        });
        app.world_mut().flush();
        for id in ids {
            assert_eq!(
                app.world()
                    .resource::<crate::operations::Operations>()
                    .get(id)
                    .map(|op| &op.state),
                Some(&crate::operations::OperationState::Succeeded)
            );
        }
    }
    #[test]
    fn host_owned_readback_is_adopted_and_not_retired_on_remote_failure() {
        let (mut app, _directory, window) = host();
        let screenshot = app.world_mut().spawn(Screenshot::window(window)).id();
        let id = admit(app.world_mut(), window);
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = 6;
        advance(app.world_mut());
        assert_eq!(
            app.world_mut()
                .query::<&Screenshot>()
                .iter(app.world())
                .count(),
            1
        );
        app.world_mut().despawn(window);
        advance(app.world_mut());
        assert!(app.world().entities().contains(screenshot));
        assert_eq!(
            app.world()
                .resource::<crate::operations::Operations>()
                .get(id)
                .map(|op| &op.state),
            Some(&crate::operations::OperationState::Failed)
        );
    }
    #[test]
    fn host_primary_alias_completes_remote_receipt_without_an_owned_screenshot() {
        let (mut app, _directory, window) = host();
        app.world_mut()
            .entity_mut(window)
            .insert(bevy::window::PrimaryWindow);
        let screenshot = app.world_mut().spawn(Screenshot::primary_window()).id();
        let id = admit(app.world_mut(), window);
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = 6;
        advance(app.world_mut());
        assert_eq!(
            app.world_mut()
                .query::<&Screenshot>()
                .iter(app.world())
                .count(),
            1
        );
        app.world_mut().trigger(ScreenshotCaptured {
            entity: screenshot,
            image: image(),
        });
        app.world_mut().flush();
        assert_eq!(
            app.world()
                .resource::<crate::operations::Operations>()
                .get(id)
                .map(|op| &op.state),
            Some(&crate::operations::OperationState::Succeeded)
        );
        assert!(app.world().entities().contains(screenshot));
    }
    #[test]
    fn renderer_retired_duplicate_is_retried_and_failure_remains_terminal() {
        let (mut app, directory, window) = host();
        let screenshot = app.world_mut().spawn(Screenshot::window(window)).id();
        let id = admit(app.world_mut(), window);
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = 6;
        advance(app.world_mut());
        app.world_mut().despawn(screenshot);
        advance(app.world_mut());
        let screenshots: Vec<_> = app
            .world_mut()
            .query_filtered::<Entity, With<Screenshot>>()
            .iter(app.world())
            .collect();
        assert_eq!(screenshots.len(), 1);
        assert_ne!(screenshots[0], screenshot);
        // Publication failure must be terminal for every coalesced receipt too.
        let second = admit(app.world_mut(), window);
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = 12;
        advance(app.world_mut());
        std::fs::write(directory.path().join(format!("{id}.png")), b"existing")
            .unwrap_or_else(|error| panic!("{error}"));
        app.world_mut().trigger(ScreenshotCaptured {
            entity: screenshots[0],
            image: image(),
        });
        app.world_mut().flush();
        let operations = app.world().resource::<crate::operations::Operations>();
        assert_eq!(
            operations.get(id).map(|op| &op.state),
            Some(&crate::operations::OperationState::Failed)
        );
        assert_eq!(
            operations.get(second).map(|op| &op.state),
            Some(&crate::operations::OperationState::Succeeded)
        );
        assert!(app.world().resource::<PendingCaptures>().0.is_empty());
    }
    #[test]
    fn timeout_retires_only_owned_readbacks_and_late_events_cannot_publish() {
        let (mut app, directory, window) = host();
        let ids = [
            admit(app.world_mut(), window),
            admit(app.world_mut(), window),
        ];
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = 6;
        advance(app.world_mut());
        let entity = app
            .world_mut()
            .query_filtered::<Entity, With<Screenshot>>()
            .iter(app.world())
            .next()
            .unwrap_or_else(|| panic!("No screenshot"));
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = 181;
        advance(app.world_mut());
        assert!(!app.world().entities().contains(entity));
        app.world_mut().trigger(ScreenshotCaptured {
            entity,
            image: image(),
        });
        app.world_mut().flush();
        for id in ids {
            assert_eq!(
                app.world()
                    .resource::<crate::operations::Operations>()
                    .get(id)
                    .map(|op| &op.state),
                Some(&crate::operations::OperationState::Failed)
            );
            assert!(!directory.path().join(format!("{id}.png")).exists());
        }
    }
    #[test]
    fn older_in_flight_host_frame_cannot_complete_a_new_request() {
        let (mut app, _directory, window) = host();
        let old = app
            .world_mut()
            .spawn((Screenshot::window(window), Capturing))
            .id();
        let id = admit(app.world_mut(), window);
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = 6;
        advance(app.world_mut());
        let fresh = app
            .world_mut()
            .query_filtered::<Entity, With<CaptureOwned>>()
            .iter(app.world())
            .next()
            .unwrap_or_else(|| panic!("No fresh screenshot"));
        app.world_mut().trigger(ScreenshotCaptured {
            entity: old,
            image: image(),
        });
        app.world_mut().flush();
        assert_eq!(
            app.world()
                .resource::<crate::operations::Operations>()
                .get(id)
                .map(|op| &op.state),
            Some(&crate::operations::OperationState::Pending)
        );
        app.world_mut().trigger(ScreenshotCaptured {
            entity: fresh,
            image: image(),
        });
        app.world_mut().flush();
        assert_eq!(
            app.world()
                .resource::<crate::operations::Operations>()
                .get(id)
                .map(|op| &op.state),
            Some(&crate::operations::OperationState::Succeeded)
        );
        assert!(app.world().entities().contains(old));
    }
    #[test]
    fn host_screenshot_added_after_selection_replaces_only_the_owned_duplicate() {
        let (mut app, _directory, window) = host();
        let id = admit(app.world_mut(), window);
        app.world_mut()
            .resource_mut::<bevy::diagnostic::FrameCount>()
            .0 = 6;
        advance(app.world_mut());
        let host_screenshot = app.world_mut().spawn(Screenshot::window(window)).id();
        app.world_mut().flush();
        assert_eq!(
            app.world_mut()
                .query::<&Screenshot>()
                .iter(app.world())
                .count(),
            1
        );
        assert!(app.world().entities().contains(host_screenshot));
        app.world_mut().trigger(ScreenshotCaptured {
            entity: host_screenshot,
            image: image(),
        });
        app.world_mut().flush();
        assert_eq!(
            app.world()
                .resource::<crate::operations::Operations>()
                .get(id)
                .map(|op| &op.state),
            Some(&crate::operations::OperationState::Succeeded)
        );
    }
    #[test]
    fn completed_frames_publish_once_and_invalid_images_do_not_replace_artifacts()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let id = Uuid::new_v4();
        let image = Image::from_dynamic(image::DynamicImage::new_rgba8(2, 2), true, default());
        let frame = save_frame(directory.path(), image.clone(), id)?;
        assert_eq!(frame.size, [2, 2]);
        assert!(frame.uniform);
        let bytes = std::fs::read(&frame.path)?;
        assert!(save_frame(directory.path(), image, id).is_err());
        assert_eq!(std::fs::read(&frame.path)?, bytes);
        let invalid = Image {
            data: None,
            ..default()
        };
        assert!(save_frame(directory.path(), invalid, Uuid::new_v4()).is_err());
        Ok(())
    }
}
