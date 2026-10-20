//! Windowless bootstrap retains integrity, timeout and source-readiness gates.
#![allow(clippy::unwrap_used, reason = "Bootstrap lifecycle assertions")]
use super::*;
use crate::startup_tests::{app, assert_failure};
use std::time::Instant;

fn empty_delivery() -> (tempfile::TempDir, kedra_assets::delivery::Manifest) {
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
    (directory, manifest)
}

#[test]
fn verification_starts_without_a_window_or_renderer_and_waits_for_catalog() {
    let (directory, manifest) = empty_delivery();
    let mut app = app();
    app.insert_resource(Boot::default());
    app.world_mut().resource_mut::<Boot>().delivery = Some((directory.path().into(), manifest));
    app.world_mut().resource_mut::<PrefabSources>().pending = true;
    poll(app.world_mut());
    assert!(app.world().resource::<Boot>().started.is_some());
    assert!(app.world().resource::<Boot>().verification.is_some());
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.world().resource::<Boot>().verified {
        assert!(Instant::now() < deadline);
        poll(app.world_mut());
        std::thread::yield_now();
    }
    assert!(!app.world().resource::<Boot>().complete);
    assert!(!app.world().contains_resource::<crate::jobs::PendingJob>());
    assert!(
        app.world()
            .resource::<kedra_desktop::DesktopSurface>()
            .window
            .is_none()
    );
    app.world_mut().resource_mut::<PrefabSources>().pending = false;
    poll(app.world_mut());
    assert!(app.world().resource::<Boot>().complete);
    assert!(app.world().contains_resource::<crate::jobs::PendingJob>());
    assert!(
        app.world()
            .resource::<kedra_desktop::DesktopSurface>()
            .window
            .is_none()
    );
}

#[test]
fn missing_delivery_integrity_failure_and_deadline_remain_terminal_without_fallback_ui() {
    let mut app = app();
    app.insert_resource(Boot::default());
    poll(app.world_mut());
    assert_failure(app.world(), "delivery is not configured");

    let mut app = crate::startup_tests::app();
    let task = IoTaskPool::get().spawn(async { Err("integrity mismatch".into()) });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !task.is_finished() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    app.world_mut().resource_mut::<Boot>().verification = Some(task);
    poll(app.world_mut());
    assert_failure(app.world(), "integrity mismatch");
    assert!(!app.world().resource::<Boot>().complete);
    assert!(
        app.world()
            .resource::<kedra_desktop::DesktopSurface>()
            .window
            .is_none()
    );

    let mut app = crate::startup_tests::app();
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_secs(30));
    poll(app.world_mut());
    assert!(
        app.world()
            .resource::<kedra_ui::authored::components::UiFailure>()
            .0
            .is_none()
    );
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_nanos(1));
    poll(app.world_mut());
    assert_failure(app.world(), "within 30 seconds");
    assert!(!app.world().contains_resource::<crate::jobs::PendingJob>());
}

#[test]
fn verification_rejects_manifest_changes_and_missing_members() {
    let (directory, expected) = empty_delivery();
    verify_delivery(directory.path(), &expected).unwrap();
    let marker = directory.path().join(kedra_assets::delivery::MANIFEST);
    let mut changed = expected.clone();
    let bytes = b"new delivery member";
    std::fs::write(directory.path().join("extra.txt"), bytes).unwrap();
    changed.files.insert(
        "extra.txt".into(),
        kedra_assets::delivery::FileDigest {
            bytes: bytes.len() as u64,
            sha256: kedra_assets::delivery::digest(bytes),
        },
    );
    std::fs::write(&marker, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(
        verify_delivery(directory.path(), &expected)
            .unwrap_err()
            .contains("manifest changed")
    );
    std::fs::remove_file(directory.path().join("extra.txt")).unwrap();
    assert!(verify_delivery(directory.path(), &changed).is_err());
}
