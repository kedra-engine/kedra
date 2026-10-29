use super::*;
use std::time::{Duration, Instant};

fn until(app: &mut App, condition: impl Fn(&CaptureState) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        let state = app.world().resource::<Recorder>().state();
        if condition(state) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Recording failed to reach expected state: {state:?}"
        );
        std::thread::yield_now();
    }
}

#[test]
fn repeated_recordings_cancellation_and_failed_publication_preserve_foreign_files()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let foreign = directory.path().join("foreign.pftrace");
    std::fs::write(&foreign, b"foreign recording")?;
    let mut app = host(directory.path());
    app.add_plugins(bevy::log::LogPlugin {
        custom_layer: kedra_profiling::log_layer,
        ..default()
    });
    app.finish();
    app.cleanup();
    let config = CaptureConfig {
        duration_ms: 1000,
        buffer_kib: 1024,
    };
    let retained = bevy::log::info_span!("contract.retained_system");
    app.world_mut()
        .resource_mut::<Recorder>()
        .set_label(Some("menus 日本語".into()))?;
    for invalid in ["", "bad\nlabel", &"x".repeat(257)] {
        assert!(matches!(
            app.world_mut()
                .resource_mut::<Recorder>()
                .set_label(Some(invalid.into())),
            Err(Error::Invalid(_))
        ));
    }
    let mut published = Vec::new();
    for _ in 0..2 {
        let id = app.world_mut().resource_mut::<Recorder>().start(config)?;
        assert!(matches!(
            app.world_mut()
                .resource_mut::<Recorder>()
                .set_label(Some("replacement".into())),
            Err(Error::Busy)
        ));
        assert!(matches!(
            app.world_mut().resource_mut::<Recorder>().start(config),
            Err(Error::Busy)
        ));
        assert_eq!(app.world().resource::<Recorder>().state().id(), Some(id));
        until(&mut app, |state| {
            matches!(
                state,
                CaptureState::Recording { .. } | CaptureState::Failed { .. }
            )
        });
        assert!(matches!(
            app.world().resource::<Recorder>().state(),
            CaptureState::Recording { .. }
        ));
        {
            let _scope = retained.enter();
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    let _scope = bevy::log::info_span!("contract.worker").entered();
                    std::hint::black_box(vec![1u8; 128]);
                });
            });
        }
        app.world_mut().resource_mut::<Recorder>().stop()?;
        app.world_mut().resource_mut::<Recorder>().stop()?;
        until(&mut app, |state| !state.active());
        let CaptureState::Completed {
            recording,
            directory,
        } = app.world().resource::<Recorder>().state()
        else {
            return Err(format!(
                "Expected published recording: {:?}",
                app.world().resource::<Recorder>().state()
            )
            .into());
        };
        assert_eq!(recording.id, id);
        assert!(recording.stopped);
        assert!(recording.trace_bytes > 0);
        let metadata: kedra_profiling::Recording =
            serde_json::from_slice(&std::fs::read(directory.join("recording.json"))?)?;
        assert_eq!(metadata.id, id);
        assert_eq!(metadata.label.as_deref(), Some("menus 日本語"));
        assert_eq!(
            std::fs::metadata(directory.join("trace.pftrace"))?.len(),
            recording.trace_bytes
        );
        published.push((
            directory.clone(),
            std::fs::read(directory.join("trace.pftrace"))?,
        ));
    }
    app.world_mut().resource_mut::<Recorder>().start(config)?;
    app.world_mut().resource_mut::<Recorder>().cancel()?;
    until(&mut app, |state| !state.active());
    assert!(matches!(
        app.world().resource::<Recorder>().state(),
        CaptureState::Cancelled { .. }
    ));
    for (path, bytes) in published {
        assert_eq!(std::fs::read(path.join("trace.pftrace"))?, bytes);
    }
    assert_eq!(std::fs::read(&foreign)?, b"foreign recording");
    // A real invalid output locator fails on the worker without replacing the file.
    let mut denied = host(&foreign);
    denied
        .world_mut()
        .resource_mut::<Recorder>()
        .start(config)?;
    until(&mut denied, |state| !state.active());
    assert!(matches!(
        denied.world().resource::<Recorder>().state(),
        CaptureState::Failed { .. }
    ));
    assert_eq!(std::fs::read(&foreign)?, b"foreign recording");
    Ok(())
}
