//! Explicit, finite application recordings over Bevy tracing.
//!
//! Hosts opt into the `capture` feature and install [`log_layer`] through Bevy's
//! `LogPlugin::custom_layer`, then add [`ProfilingPlugin`]. Neither step starts a
//! recording, opens a socket or depends on editor/automation services.
//! [`Recorder`] owns admission and asynchronous completion. Native SDK work and
//! file writes run on an owned worker, never on Bevy's update thread.
mod category;
#[cfg(feature = "capture")]
mod frames;
mod instrumentation;
mod launch;
mod model;
pub use category::Category;
pub use instrumentation::{Counter, CounterValue, Marker, MarkerGuard, Operation, Unit};
pub use launch::CaptureOptions;
#[cfg(feature = "capture")]
mod backend;
pub use model::{CaptureConfig, CaptureState, Recording, Target};

use bevy::prelude::*;
use std::path::PathBuf;
use uuid::Uuid;

/// An explicitly configured application recording owner.
///
/// The directory is a native output locator, not an authoring asset source.
/// Creating a plugin does not create the directory or authorize project execution.
pub struct ProfilingPlugin {
    /// Immutable application/build identity, independent of process identity.
    pub target: Target,
    /// Host-authorized local recording directory; never an implicit project path.
    pub directory: PathBuf,
}

impl Plugin for ProfilingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Recorder {
            target: self.target.clone(),
            #[cfg(feature = "capture")]
            directory: self.directory.clone(),
            state: CaptureState::Idle,
            label: None,
            #[cfg(feature = "capture")]
            worker: None,
        });
        #[cfg(feature = "capture")]
        {
            app.add_systems(First, poll);
            frames::install(app);
        }
    }
}

/// Session admission and latest completion for one application instance.
///
/// Only one capture can run at a time. Rejected operations preserve the active
/// capture and last result. Completion is observed by Bevy's `First` schedule.
/// Dropping the owner requests cancellation; no partial recording is published.
#[derive(Resource)]
pub struct Recorder {
    target: Target,
    #[cfg(feature = "capture")]
    directory: PathBuf,
    state: CaptureState,
    label: Option<String>,
    #[cfg(feature = "capture")]
    worker: Option<backend::Worker>,
}

/// Recording admission or lifecycle failure; never permission to replay an effect.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The binary was built without recording support.
    #[error("This application was built without the profiling capture feature")]
    Unavailable,
    /// Another application-owned recording has not reached a terminal state.
    #[error("A profiling recording is already active")]
    Busy,
    /// The request exceeded the supported finite bounds.
    #[error("{0}")]
    Invalid(String),
    /// No active recording can accept a stop request.
    #[error("No profiling recording is active")]
    Inactive,
    /// A worker could not be created or admitted the request unsuccessfully.
    #[error("Profiling worker failed: {0}")]
    Worker(String),
}

impl Recorder {
    /// Whether this binary contains the recording backend.
    pub const fn available(&self) -> bool {
        cfg!(feature = "capture")
    }

    /// The current lifecycle/result. Timings inside a trace are elapsed scope time,
    /// not CPU residency, GPU time or physical input-to-photon latency.
    pub fn state(&self) -> &CaptureState {
        &self.state
    }

    /// Immutable host identity. The same host can produce many recording sessions.
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// Set the workload label for subsequent captures. An active capture's context
    /// is immutable. Invalid/busy requests preserve the previous label and state.
    pub fn set_label(&mut self, label: Option<String>) -> Result<(), Error> {
        if self.state.active() {
            return Err(Error::Busy);
        }
        if label.as_ref().is_some_and(|label| {
            label.is_empty() || label.len() > 256 || label.chars().any(char::is_control)
        }) {
            return Err(Error::Invalid(
                "Recording labels require 1..=256 UTF-8 bytes without control characters".into(),
            ));
        }
        self.label = label;
        Ok(())
    }

    /// Admit a finite recording without blocking the native update thread.
    ///
    /// Returns a fresh session identity only after the worker was spawned.
    /// Setup is asynchronous: `Starting` is not proof that recording has begun.
    /// Invalid/busy/unavailable requests do not write files or change serving state.
    pub fn start(&mut self, config: CaptureConfig) -> Result<Uuid, Error> {
        config.validate()?;
        if self.target.name.is_empty()
            || self.target.name.len() > 256
            || self.target.build.len() > 2048
        {
            return Err(Error::Invalid("Profiling requires a target name (1..=256 bytes) and bounded build identity (<=2048 bytes)".into()));
        }
        if self.state.active() {
            return Err(Error::Busy);
        }
        #[cfg(feature = "capture")]
        {
            if self.directory.as_os_str().is_empty() {
                return Err(Error::Invalid(
                    "Profiling requires a nonempty output directory".into(),
                ));
            }
            let id = Uuid::new_v4();
            let worker = backend::Worker::spawn(
                id,
                config,
                self.target.clone(),
                self.directory.clone(),
                self.label.clone(),
            )?;
            self.worker = Some(worker);
            self.state = CaptureState::Starting { id };
            Ok(id)
        }
        #[cfg(not(feature = "capture"))]
        Err(Error::Unavailable)
    }

    /// Request graceful stop once. The existing session continues through stopping
    /// and publication; callers must observe a terminal state, not restart it.
    /// Repeated stops while stopping succeed without issuing another effect.
    pub fn stop(&mut self) -> Result<(), Error> {
        #[cfg(feature = "capture")]
        {
            let id = match self.state {
                CaptureState::Starting { id } | CaptureState::Recording { id } => id,
                CaptureState::Stopping { .. } => return Ok(()),
                _ => return Err(Error::Inactive),
            };
            self.worker.as_ref().ok_or(Error::Inactive)?.stop()?;
            self.state = CaptureState::Stopping { id };
            Ok(())
        }
        #[cfg(not(feature = "capture"))]
        Err(Error::Unavailable)
    }

    /// Request cancellation without publishing a partial recording. Completion is
    /// asynchronous and retains the session identity. Foreign recordings are untouched.
    /// If publication has already committed, returns [`Error::Inactive`] and preserves
    /// its successful result; cancelling cannot undo an already published capture.
    pub fn cancel(&mut self) -> Result<(), Error> {
        #[cfg(feature = "capture")]
        {
            let id = self
                .state
                .id()
                .filter(|_| self.state.active())
                .ok_or(Error::Inactive)?;
            self.worker.as_ref().ok_or(Error::Inactive)?.cancel()?;
            self.state = CaptureState::Stopping { id };
            Ok(())
        }
        #[cfg(not(feature = "capture"))]
        Err(Error::Unavailable)
    }
}

/// Install the standard Perfetto tracing layer through Bevy's existing log setup.
///
/// This configures only an in-process producer: no listener or system tracing
/// service. Only scope names and the diagnostic `name` field are recorded after
/// explicit Start; arbitrary fields, source locations and ordinary log events are
/// excluded. Never install a second global subscriber.
pub fn log_layer(_app: &mut App) -> Option<bevy::log::BoxedLayer> {
    #[cfg(feature = "capture")]
    {
        use tracing_subscriber::Layer;
        backend::INITIALIZED.call_once(backend::initialize);
        Some(Box::new(backend::bevy_layer::BevyLayer.with_filter(
            tracing_subscriber::filter::filter_fn(|metadata| metadata.is_span()),
        )))
    }
    #[cfg(not(feature = "capture"))]
    None
}

#[cfg(feature = "capture")]
fn poll(mut recorder: ResMut<Recorder>) {
    if let Some(worker) = &recorder.worker
        && let Some(state) = worker.poll(&recorder.state)
    {
        let terminal = !state.active();
        recorder.state = state;
        if terminal {
            recorder.worker = None;
        }
    }
}
