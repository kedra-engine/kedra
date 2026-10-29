//! Real recording ownership and publication; no editor, renderer, RPC or fake backend.
use bevy::prelude::*;
use kedra_profiling::{CaptureConfig, CaptureState, Error, ProfilingPlugin, Recorder, Target};
use std::path::Path;
use uuid::Uuid;

fn host(directory: &Path) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(ProfilingPlugin {
            target: Target {
                instance: Uuid::new_v4(),
                name: "Independent 日本語 host".into(),
                build: "contract".into(),
            },
            directory: directory.into(),
        });
    app
}

#[test]
fn invalid_limits_preserve_the_owner_and_do_not_create_output()
-> Result<(), Box<dyn std::error::Error>> {
    let parent = tempfile::tempdir()?;
    let directory = parent.path().join("not-created");
    let mut app = host(&directory);
    for config in [
        CaptureConfig {
            duration_ms: 0,
            ..default()
        },
        CaptureConfig {
            buffer_kib: u32::MAX,
            ..default()
        },
    ] {
        assert!(matches!(
            app.world_mut().resource_mut::<Recorder>().start(config),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            app.world().resource::<Recorder>().state(),
            CaptureState::Idle
        ));
        assert!(!directory.exists());
    }
    Ok(())
}

#[test]
fn instrumentation_rejects_invalid_names_and_values_without_a_recording()
-> Result<(), Box<dyn std::error::Error>> {
    use kedra_profiling::{Counter, Marker, Operation, Unit};
    for name in ["", "private\nvalue", "nul\0value"] {
        assert!(Counter::new(name, Unit::Count).is_err());
        assert!(Marker::named(name).is_err());
        assert!(Operation::begin(name).is_err());
    }
    let counter = Counter::new("objects.日本語", Unit::Count)?;
    assert!(counter.sample(f64::NAN).is_err());
    assert!(counter.sample(f64::INFINITY).is_err());
    counter.sample(i64::MAX)?;
    assert_eq!(counter.name(), "objects.日本語");
    assert_eq!(counter.unit(), Unit::Count);
    let _scope = Marker::new("cpu.work").enter();
    Operation::begin("logical.work")?.finish();
    Ok(())
}

#[cfg(not(feature = "capture"))]
#[test]
fn ordinary_build_has_no_recorder_backend_or_file_side_effects()
-> Result<(), Box<dyn std::error::Error>> {
    let parent = tempfile::tempdir()?;
    let directory = parent.path().join("not-created");
    let mut app = host(&directory);
    let mut recorder = app.world_mut().resource_mut::<Recorder>();
    assert!(!recorder.available());
    assert!(matches!(recorder.start(default()), Err(Error::Unavailable)));
    assert!(matches!(recorder.stop(), Err(Error::Unavailable)));
    assert!(!directory.exists());
    Ok(())
}

#[cfg(feature = "capture")]
#[path = "contracts/recording.rs"]
mod recording;

#[test]
fn launch_schema_respects_build_capability_and_explicit_output_authorization() {
    use clap::Parser;
    #[derive(Parser)]
    struct HostOptions {
        #[command(flatten)]
        profiling: kedra_profiling::CaptureOptions,
    }
    assert!(HostOptions::try_parse_from(["application"]).is_ok());
    #[cfg(not(feature = "capture"))]
    assert!(
        HostOptions::try_parse_from(["application", "--profile-output", "recordings"]).is_err()
    );
    #[cfg(feature = "capture")]
    {
        assert!(
            HostOptions::try_parse_from([
                "application",
                "--profile-output",
                "recordings",
                "--profile-manual"
            ])
            .is_ok()
        );
        for args in [
            vec!["application", "--profile-label", "menus"],
            vec!["application", "--profile-manual"],
            vec!["application", "--profile-duration-ms", "1000"],
            vec![
                "application",
                "--profile-output",
                "recordings",
                "--profile-duration-ms",
                "0",
            ],
            vec![
                "application",
                "--profile-output",
                "recordings",
                "--profile-buffer-kib",
                "999999",
            ],
        ] {
            assert!(HostOptions::try_parse_from(args).is_err());
        }
    }
}
