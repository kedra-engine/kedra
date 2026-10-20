//! Early splash uses the same scene/style runtime, but a minimal catalog. Normal
//! editor documents and whole-delivery I/O start only after native splash frames.
use bevy::{
    diagnostic::FrameCount,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    tasks::{IoTaskPool, Task, block_on, poll_once},
};
use kedra_scene::assets::PrefabSources;
use kedra_ui::{ScreenId, authored::view::UiView};
use std::{path::PathBuf, time::Duration};

#[derive(Resource, Default)]
pub(crate) struct Boot {
    pub complete: bool,
    pub delivery: Option<(PathBuf, kedra_assets::delivery::Manifest)>,
    hydrated: Option<u32>,
    native: bool,
    started: Option<Duration>,
    verification: Option<Task<Result<(), String>>>,
    verified: bool,
    readback_started: Option<Duration>,
    rendered: bool,
}
#[derive(Component)]
struct BootPhase;

pub(crate) fn enter(world: &mut World) {
    let root = kedra_desktop::replace(
        world,
        kedra_desktop::SurfaceKind::Boot,
        "Kedra Engine".into(),
    );
    if let Err(error) = crate::views::request(
        world,
        root,
        crate::views::Kind::Frame(kedra_desktop::SurfaceKind::Boot),
    ) {
        crate::views::fail(world, error);
    }
}

pub(crate) fn bind(world: &mut World, view: &UiView) -> Result<(), String> {
    let version = view.element("boot.version").map_err(|e| e.to_string())?;
    let phase = view.element("boot.phase").map_err(|e| e.to_string())?;
    world
        .get_mut::<Text>(version)
        .ok_or("boot version is not native text")?
        .0 = env!("CARGO_PKG_VERSION").into();
    world.entity_mut(phase).insert(BootPhase);
    world.entity_mut(view.root).insert(ScreenId("boot".into()));
    world.resource_mut::<Boot>().hydrated = Some(world.resource::<FrameCount>().0);
    Ok(())
}

pub(crate) fn created(
    mut events: MessageReader<bevy::window::WindowCreated>,
    surface: Res<kedra_desktop::DesktopSurface>,
    mut boot: ResMut<Boot>,
) {
    for event in events.read() {
        if Some(event.window) == surface.window {
            boot.native = true;
        }
    }
}

fn may_start(native: bool, rendered: bool, hydrated: Option<u32>, frame: u32) -> bool {
    native && rendered && hydrated.is_some_and(|ready| frame.wrapping_sub(ready) >= 2)
}

pub(crate) fn poll(world: &mut World) {
    if *world.resource::<State<crate::AppPhase>>().get() != crate::AppPhase::Boot
        || world.resource::<Boot>().complete
    {
        return;
    }
    let boot = world.resource::<Boot>();
    let elapsed = world.resource::<Time<Real>>().elapsed();
    if !boot.rendered {
        if boot
            .readback_started
            .is_some_and(|start| elapsed.saturating_sub(start) > Duration::from_secs(30))
        {
            crate::views::fail(
                world,
                "Engine splash renderer readback did not complete within 30 seconds".into(),
            );
            return;
        }
        if boot.readback_started.is_none()
            && boot.native
            && boot
                .hydrated
                .is_some_and(|ready| world.resource::<FrameCount>().0.wrapping_sub(ready) >= 6)
        {
            request_first_frame(world, elapsed);
        }
        return;
    }
    if boot.started.is_none() {
        if !may_start(
            boot.native,
            boot.rendered,
            boot.hydrated,
            world.resource::<FrameCount>().0,
        ) {
            return;
        }
        let Some((delivery, manifest)) = boot.delivery.clone() else {
            crate::views::fail(world, "Bootstrap delivery is not configured".into());
            return;
        };
        info!(
            "Engine splash renderer frame confirmed; starting complete delivery verification and editor catalog loading"
        );
        let task = IoTaskPool::get().spawn(async move { verify_delivery(&delivery, &manifest) });
        let handle = world
            .resource::<AssetServer>()
            .load("editor://scenes/startup.prefabs.ron");
        let mut sources = world.resource_mut::<PrefabSources>();
        sources.handle = handle;
        sources.pending = true;
        sources.dependencies_ready = false;
        sources.error = None;
        let mut boot = world.resource_mut::<Boot>();
        boot.verification = Some(task);
        boot.started = Some(elapsed);
        for mut text in world
            .query_filtered::<&mut Text, With<BootPhase>>()
            .iter_mut(world)
        {
            text.0 = "Loading editor assets…".into();
        }
        return;
    }
    let result = world
        .resource_mut::<Boot>()
        .verification
        .as_mut()
        .and_then(|task| block_on(poll_once(task)));
    if let Some(result) = result {
        world.resource_mut::<Boot>().verification = None;
        match result {
            Ok(()) => world.resource_mut::<Boot>().verified = true,
            Err(error) => {
                crate::views::fail(world, error);
                return;
            }
        }
    }
    let sources = world.resource::<PrefabSources>();
    if let Some(error) = sources.error.clone() {
        crate::views::fail(world, error);
        return;
    }
    if world
        .resource::<Boot>()
        .started
        .is_some_and(|start| elapsed.saturating_sub(start) > Duration::from_secs(30))
    {
        crate::views::fail(
            world,
            "Engine startup did not complete within 30 seconds".into(),
        );
        return;
    }
    if sources.ready() && world.resource::<Boot>().verified {
        world.resource_mut::<Boot>().complete = true;
        crate::jobs::load_history(world);
        info!("Engine startup complete; entering project workflow");
    }
}

fn verify_delivery(
    root: &std::path::Path,
    expected: &kedra_assets::delivery::Manifest,
) -> Result<(), String> {
    let (_, verified) = kedra_assets::delivery::read_directory(root, "editor-desktop")?;
    if serde_json::to_value(expected).map_err(|e| e.to_string())?
        != serde_json::to_value(verified).map_err(|e| e.to_string())?
    {
        return Err("Asset delivery manifest changed during engine startup".into());
    }
    Ok(())
}

fn request_first_frame(world: &mut World, elapsed: Duration) {
    let surface = world.resource::<kedra_desktop::DesktopSurface>();
    let (Some(window), Some(root)) = (surface.window, surface.root) else {
        return;
    };
    world.resource_mut::<Boot>().readback_started = Some(elapsed);
    world
        .spawn((Screenshot::window(window), ChildOf(root)))
        .observe(
            move |event: On<ScreenshotCaptured>, mut commands: Commands| {
                let image = event.image.clone();
                commands.queue(move |world: &mut World| {
                    if world.get::<Window>(window).is_none() || world.resource::<Boot>().complete {
                        return;
                    }
                    let result = (|| -> Result<(), String> {
                        validate_frame(&image)?;
                        if let Some(root) = world.get_resource::<kedra_app::capture::ArtifactRoot>()
                        {
                            let artifact = kedra_app::capture::save_frame(
                                &root.0,
                                image,
                                uuid::Uuid::new_v4(),
                            )?;
                            info!(
                                "Engine splash first frame: {} ({}x{})",
                                artifact.path.display(),
                                artifact.size[0],
                                artifact.size[1]
                            );
                        }
                        world.resource_mut::<Boot>().rendered = true;
                        Ok(())
                    })();
                    if let Err(error) = result {
                        crate::views::fail(world, error);
                    }
                });
            },
        );
}

fn validate_frame(image: &Image) -> Result<(), String> {
    let pixels = image
        .clone()
        .try_into_dynamic()
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let first = pixels
        .pixels()
        .next()
        .ok_or("Engine splash renderer returned no pixels")?;
    if pixels.pixels().all(|pixel| pixel.0[3] == 0) || pixels.pixels().all(|pixel| pixel == first) {
        return Err("Engine splash renderer returned a transparent or uniform frame".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "boot/lifecycle_tests.rs"]
mod lifecycle_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn async_verification_must_validate_the_same_manifest_as_bootstrap()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let expected = kedra_assets::delivery::Manifest {
            format: 1,
            profile: "editor-desktop".into(),
            assets: vec![],
            files: default(),
        };
        let marker = directory.path().join(kedra_assets::delivery::MANIFEST);
        std::fs::write(&marker, serde_json::to_vec(&expected)?)?;
        verify_delivery(directory.path(), &expected)?;
        let mut changed = expected.clone();
        let bytes = b"new delivery member";
        std::fs::write(directory.path().join("extra.txt"), bytes)?;
        changed.files.insert(
            "extra.txt".into(),
            kedra_assets::delivery::FileDigest {
                bytes: bytes.len() as u64,
                sha256: kedra_assets::delivery::digest(bytes),
            },
        );
        std::fs::write(marker, serde_json::to_vec(&changed)?)?;
        assert!(verify_delivery(directory.path(), &expected).is_err());
        Ok(())
    }
    #[test]
    fn blank_or_transparent_renderer_frames_cannot_release_boot() {
        let make = |pixels: image::RgbaImage| {
            Image::from_dynamic(image::DynamicImage::ImageRgba8(pixels), true, default())
        };
        let mut pixels = image::RgbaImage::from_pixel(2, 2, image::Rgba([36, 36, 36, 255]));
        assert!(validate_frame(&make(pixels.clone())).is_err());
        pixels.put_pixel(0, 0, image::Rgba([238, 238, 238, 255]));
        assert!(validate_frame(&make(pixels)).is_ok());
        let mut transparent = image::RgbaImage::new(2, 2);
        transparent.put_pixel(0, 0, image::Rgba([238, 238, 238, 0]));
        assert!(validate_frame(&make(transparent)).is_err());
    }
    #[test]
    fn project_requests_cannot_bypass_boot_verification() {
        let mut world = World::new();
        world.init_resource::<Boot>();
        let result = crate::actions::request_project(
            &mut world,
            crate::model::Intent::Open(PathBuf::from("not-opened")),
        );
        assert!(result.is_err());
        assert!(!world.contains_resource::<crate::jobs::PendingJob>());
    }
    #[test]
    fn heavy_work_waits_for_native_hydration_and_actual_renderer_readback() {
        assert!(!may_start(false, true, Some(10), 20));
        assert!(!may_start(true, true, None, 20));
        assert!(!may_start(true, true, Some(10), 10));
        assert!(!may_start(true, true, Some(10), 11));
        assert!(!may_start(true, false, Some(10), 20));
        assert!(may_start(true, true, Some(10), 12));
        assert!(may_start(true, true, Some(u32::MAX), 1));
    }
}
