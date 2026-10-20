//! CPU events test bootstrap state/receipts, not GPU or native presentation.
#![allow(clippy::unwrap_used, reason = "Bootstrap lifecycle assertions")]
use super::*;
use crate::startup_tests::{app, assert_failure};
use std::time::Instant;

fn bootstrap() -> App {
    let mut app = app();
    for _ in 0..3 {
        app.update();
    }
    let frame = app.world().resource::<FrameCount>().0;
    {
        let mut boot = app.world_mut().resource_mut::<Boot>();
        boot.native = true;
        boot.hydrated = Some(frame.wrapping_sub(6));
    }
    app
}

fn frame() -> Image {
    let mut pixels = image::RgbaImage::from_pixel(2, 2, image::Rgba([36, 36, 36, 255]));
    pixels.put_pixel(0, 0, image::Rgba([230, 230, 230, 255]));
    Image::from_dynamic(image::DynamicImage::ImageRgba8(pixels), true, default())
}

#[test]
fn first_readback_event_is_validated_and_optional_artifact_is_published() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = bootstrap();
    app.insert_resource(kedra_app::capture::ArtifactRoot(directory.path().into()));
    poll(app.world_mut());
    let readback = app
        .world_mut()
        .query_filtered::<Entity, With<Screenshot>>()
        .single(app.world())
        .unwrap();
    app.world_mut().trigger(ScreenshotCaptured {
        entity: readback,
        image: frame(),
    });
    app.world_mut().flush();
    assert!(app.world().resource::<Boot>().rendered);
    let artifacts: Vec<_> = std::fs::read_dir(directory.path()).unwrap().collect();
    assert_eq!(artifacts.len(), 1);
    assert!(
        artifacts[0]
            .as_ref()
            .unwrap()
            .path()
            .extension()
            .is_some_and(|extension| extension == "png")
    );
    assert!(!app.world().resource::<Boot>().complete);
}

#[test]
fn retired_readback_is_ignored_and_artifact_write_failure_is_terminal() {
    let mut app = bootstrap();
    poll(app.world_mut());
    let readback = app
        .world_mut()
        .query_filtered::<Entity, With<Screenshot>>()
        .single(app.world())
        .unwrap();
    let window = app
        .world()
        .resource::<kedra_desktop::DesktopSurface>()
        .window
        .unwrap();
    app.world_mut().despawn(window);
    app.world_mut().trigger(ScreenshotCaptured {
        entity: readback,
        image: frame(),
    });
    app.world_mut().flush();
    assert!(!app.world().resource::<Boot>().rendered);

    let mut app = bootstrap();
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("not-a-directory");
    std::fs::write(&file, b"keep").unwrap();
    app.insert_resource(kedra_app::capture::ArtifactRoot(file.clone()));
    poll(app.world_mut());
    let readback = app
        .world_mut()
        .query_filtered::<Entity, With<Screenshot>>()
        .single(app.world())
        .unwrap();
    app.world_mut().trigger(ScreenshotCaptured {
        entity: readback,
        image: frame(),
    });
    app.world_mut().flush();
    assert!(
        app.world()
            .resource::<kedra_ui::authored::components::UiFailure>()
            .0
            .is_some()
    );
    assert!(!app.world().resource::<Boot>().rendered);
    assert_eq!(std::fs::read(file).unwrap(), b"keep");
}

#[test]
fn readback_and_bootstrap_timeouts_have_distinct_errors_and_no_ready_fallback() {
    let mut app = bootstrap();
    poll(app.world_mut());
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_secs(31));
    poll(app.world_mut());
    assert_failure(app.world(), "renderer readback did not complete");
    assert!(!app.world().resource::<Boot>().complete);

    let mut app = bootstrap();
    let now = app.world().resource::<Time<Real>>().elapsed();
    {
        let mut boot = app.world_mut().resource_mut::<Boot>();
        boot.rendered = true;
        boot.started = Some(now);
    }
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_secs(31));
    poll(app.world_mut());
    assert_failure(app.world(), "Engine startup did not complete");
    assert!(!app.world().resource::<Boot>().complete);
}

#[test]
fn missing_delivery_and_failed_completed_verification_are_explicit_failures() {
    let mut app = bootstrap();
    app.world_mut().resource_mut::<Boot>().rendered = true;
    poll(app.world_mut());
    assert_failure(app.world(), "delivery is not configured");

    let mut app = bootstrap();
    let now = app.world().resource::<Time<Real>>().elapsed();
    let task = IoTaskPool::get().spawn(async { Err("integrity mismatch".into()) });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !task.is_finished() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    {
        let mut boot = app.world_mut().resource_mut::<Boot>();
        boot.rendered = true;
        boot.started = Some(now);
        boot.verification = Some(task);
    }
    poll(app.world_mut());
    assert_failure(app.world(), "integrity mismatch");
    assert!(app.world().resource::<Boot>().verification.is_none());
    assert!(!app.world().contains_resource::<crate::jobs::PendingJob>());
}

#[test]
fn complete_verification_still_waits_for_sources_before_starting_history() {
    let mut app = bootstrap();
    let directory = tempfile::tempdir().unwrap();
    let manifest = kedra_assets::delivery::Manifest {
        format: 1,
        profile: "editor-desktop".into(),
        assets: vec![],
        files: default(),
    };
    std::fs::write(
        directory.path().join(kedra_assets::delivery::MANIFEST),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let root = directory.path().to_owned();
    let task = IoTaskPool::get().spawn(async move { verify_delivery(&root, &manifest) });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !task.is_finished() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    let now = app.world().resource::<Time<Real>>().elapsed();
    {
        let mut boot = app.world_mut().resource_mut::<Boot>();
        boot.rendered = true;
        boot.started = Some(now);
        boot.verification = Some(task);
    }
    app.world_mut().resource_mut::<PrefabSources>().pending = true;
    poll(app.world_mut());
    assert!(app.world().resource::<Boot>().verified);
    assert!(!app.world().resource::<Boot>().complete);
    assert!(!app.world().contains_resource::<crate::jobs::PendingJob>());
    app.world_mut().resource_mut::<PrefabSources>().pending = false;
    poll(app.world_mut());
    assert!(app.world().resource::<Boot>().complete);
    assert!(app.world().contains_resource::<crate::jobs::PendingJob>());
    assert_eq!(
        app.world()
            .resource::<crate::model::ProjectManager>()
            .phase(),
        crate::model::Phase::Projects
    );
}

#[test]
fn rendered_boot_starts_real_delivery_verification_and_explicit_editor_catalog() {
    let directory = tempfile::tempdir().unwrap();
    let manifest = kedra_assets::delivery::Manifest {
        format: 1,
        profile: "editor-desktop".into(),
        assets: vec![],
        files: default(),
    };
    std::fs::write(
        directory.path().join(kedra_assets::delivery::MANIFEST),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let mut app = bootstrap();
    {
        let mut boot = app.world_mut().resource_mut::<Boot>();
        boot.rendered = true;
        boot.delivery = Some((directory.path().into(), manifest));
    }
    poll(app.world_mut());
    let boot = app.world().resource::<Boot>();
    assert!(boot.started.is_some());
    assert!(boot.verification.is_some());
    assert!(!boot.complete);
    let sources = app.world().resource::<PrefabSources>();
    assert!(sources.pending);
    assert!(!sources.dependencies_ready);
    assert_eq!(
        sources.handle.path().unwrap().to_string(),
        "editor://scenes/startup.prefabs.ron"
    );
    let phase = crate::startup_tests::element(app.world_mut(), "boot.phase");
    assert_eq!(
        app.world().get::<Text>(phase).unwrap().0,
        "Loading editor assets…"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app
        .world()
        .resource::<Boot>()
        .verification
        .as_ref()
        .unwrap()
        .is_finished()
    {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    poll(app.world_mut());
    assert!(app.world().resource::<Boot>().verified);
    assert!(!app.world().resource::<Boot>().complete);
    app.world_mut().resource_mut::<PrefabSources>().error = Some("editor catalog refused".into());
    poll(app.world_mut());
    assert_failure(app.world(), "editor catalog refused");
    assert!(!app.world().contains_resource::<crate::jobs::PendingJob>());
}
