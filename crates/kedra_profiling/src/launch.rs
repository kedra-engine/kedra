//! Shared explicit launch authorization, independent of editor and debug automation.
#[cfg(feature = "capture")]
use crate::{CaptureConfig, CaptureState, ProfilingPlugin, Recorder};
use crate::{Error, Target};
use bevy::prelude::*;
use clap::Args;
#[cfg(feature = "capture")]
use std::path::PathBuf;

/// Flatten into a host's clap parser to expose recording only in capable builds.
/// No remote access or project code execution is enabled by these options.
#[derive(Args, Clone, Debug, Default)]
pub struct CaptureOptions {
    /// Authorize CPU recording into a local output directory, separate from sources.
    #[cfg(feature = "capture")]
    #[arg(long)]
    pub profile_output: Option<PathBuf>,
    /// Maximum CPU capture duration (milliseconds).
    #[cfg(feature = "capture")]
    #[arg(long, default_value = "5000", requires = "profile_output", value_parser = clap::value_parser!(u32).range(100..=30000))]
    pub profile_duration_ms: u32,
    /// Perfetto trace buffer size (KiB); inspect losses before trusting results.
    #[cfg(feature = "capture")]
    #[arg(long, default_value = "32768", requires = "profile_output", value_parser = clap::value_parser!(u32).range(1024..=65536))]
    pub profile_buffer_kib: u32,
    /// Authorize recording but leave Start to the host rather than recording startup.
    #[cfg(feature = "capture")]
    #[arg(long, requires = "profile_output")]
    pub profile_manual: bool,
    /// Workload/scenario label saved in the recording, not an inferred action.
    #[cfg(feature = "capture")]
    #[arg(long, requires = "profile_output")]
    pub profile_label: Option<String>,
}

#[cfg(feature = "capture")]
#[derive(Resource)]
struct LaunchCapture(CaptureConfig);

impl CaptureOptions {
    /// Whether the caller explicitly authorized a recording output directory.
    /// Hosts use this to reject profiling options on unsupported headless commands.
    pub fn requested(&self) -> bool {
        #[cfg(feature = "capture")]
        {
            self.profile_output.is_some()
        }
        #[cfg(not(feature = "capture"))]
        {
            false
        }
    }

    /// Install the session owner only after explicit output authorization.
    ///
    /// The host must configure [`crate::log_layer`] on Bevy's `LogPlugin` before
    /// building it. No subscriber is installed/replaced here. Invalid configuration
    /// aborts launch without starting an application with a silent fallback.
    /// Automatic mode starts at Startup; manual mode exposes only [`Recorder`].
    pub fn configure(self, app: &mut App, target: Target) -> Result<(), Error> {
        #[cfg(feature = "capture")]
        {
            let Some(directory) = self.profile_output else {
                return Ok(());
            };
            let config = CaptureConfig {
                duration_ms: self.profile_duration_ms,
                buffer_kib: self.profile_buffer_kib,
            };
            config.validate()?;
            if directory.as_os_str().is_empty() {
                return Err(Error::Invalid(
                    "Profiling requires a nonempty output directory".into(),
                ));
            }
            app.add_plugins(ProfilingPlugin { target, directory });
            app.world_mut()
                .resource_mut::<Recorder>()
                .set_label(self.profile_label)?;
            if !self.profile_manual {
                app.insert_resource(LaunchCapture(config))
                    .add_systems(Startup, start);
            }
            app.add_systems(Update, progress);
        }
        #[cfg(not(feature = "capture"))]
        let _ = (self, app, target);
        Ok(())
    }
}

#[cfg(feature = "capture")]
fn start(mut recorder: ResMut<Recorder>, config: Res<LaunchCapture>) {
    match recorder.start(config.0) {
        Ok(id) => eprintln!("CPU profiling: starting recording {id}"),
        Err(error) => eprintln!("CPU profiling FAILED: {error}"),
    }
}

#[cfg(feature = "capture")]
fn progress(recorder: Res<Recorder>, mut previous: Local<Option<(uuid::Uuid, u8)>>) {
    let (Some(id), code) = (
        recorder.state().id(),
        match recorder.state() {
            CaptureState::Idle => 0,
            CaptureState::Starting { .. } => 1,
            CaptureState::Recording { .. } => 2,
            CaptureState::Stopping { .. } => 3,
            CaptureState::Completed { .. } => 4,
            CaptureState::Cancelled { .. } => 5,
            CaptureState::Failed { .. } => 6,
        },
    ) else {
        return;
    };
    if *previous == Some((id, code)) {
        return;
    }
    *previous = Some((id, code));
    match recorder.state() {
        CaptureState::Recording { .. } => eprintln!("CPU profiling: recording {id}"),
        CaptureState::Stopping { .. } => eprintln!("CPU profiling: stopping {id}"),
        CaptureState::Completed {
            directory,
            recording,
        } => {
            eprintln!(
                "CPU profiling SUCCESS: saved {} ({} bytes); inspect trace health in Perfetto UI",
                directory.join("trace.pftrace").display(),
                recording.trace_bytes
            );
            if recording.adapter_dropped_events != 0 {
                eprintln!(
                    "CPU profiling WARNING: {} adapter events dropped; scope analysis is incomplete",
                    recording.adapter_dropped_events
                );
            }
        }
        CaptureState::Cancelled { .. } => eprintln!("CPU profiling CANCELLED: {id}"),
        CaptureState::Failed { error, .. } => eprintln!("CPU profiling FAILED: {id}: {error}"),
        _ => {}
    }
}
