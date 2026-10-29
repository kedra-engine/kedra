use crate::Error;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Immutable source/build identity supplied by the application host.
/// Profiling does not infer a project, world or source revision from a process ID.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// Fresh identity for this application instance, not a persisted document ID.
    pub instance: Uuid,
    /// User-facing application name.
    pub name: String,
    /// Host build/version description; include optimization/instrumentation settings.
    pub build: String,
}

/// Finite CPU-scope capture limits. The SDK uses its own bounded trace buffer.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureConfig {
    /// Maximum recording duration in milliseconds, 100..=30_000.
    pub duration_ms: u32,
    /// SDK buffer size in KiB, 1024..=65_536. Buffer pressure may lose events;
    /// the viewer must display backend loss statistics, not assume completeness.
    pub buffer_kib: u32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            duration_ms: 5000,
            buffer_kib: 32 * 1024,
        }
    }
}

impl CaptureConfig {
    pub(crate) fn validate(self) -> Result<(), Error> {
        if !(100..=30_000).contains(&self.duration_ms) {
            return Err(Error::Invalid(
                "Capture duration must be 100..=30000 ms".into(),
            ));
        }
        if !(1024..=65_536).contains(&self.buffer_kib) {
            return Err(Error::Invalid(
                "Capture buffer must be 1024..=65536 KiB".into(),
            ));
        }
        Ok(())
    }
}

/// Saved recording provenance. `trace.pftrace` is the standard Perfetto format;
/// this sidecar describes host identity and admission, not a custom event format.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recording {
    /// Manifest schema, currently 1.
    pub schema: u32,
    /// Unique recording session.
    pub id: Uuid,
    /// Application identity.
    pub target: Target,
    /// OS process identity, independent of application/world identity.
    pub process: u32,
    /// Applied bounds.
    pub config: CaptureConfig,
    /// Backend and exact version used to encode the trace.
    pub backend: String,
    /// Whether stop was requested rather than reaching the configured duration.
    pub stopped: bool,
    /// Bytes successfully persisted. Not an event count or completeness guarantee.
    pub trace_bytes: u64,
    /// Host-supplied workload label, never inferred from user actions.
    /// Older schema-1 manifests without this additive field remain readable.
    #[serde(default)]
    pub label: Option<String>,
    /// Scope events suppressed by adapter depth limits or mismatched instrumentation.
    /// Nonzero means scope analysis is incomplete, independent of SDK buffer losses.
    pub adapter_dropped_events: u64,
}

/// One application-owned recording lifecycle. Failures and cancellation never
/// masquerade as a successful file or replace unrelated recordings.
#[derive(Clone, Debug)]
pub enum CaptureState {
    /// No recording has been admitted.
    Idle,
    /// Worker setup is pending.
    Starting {
        /// Session identity.
        id: Uuid,
    },
    /// Backend acknowledged start.
    Recording {
        /// Session identity.
        id: Uuid,
    },
    /// Stop/cancellation or completion publication is pending.
    Stopping {
        /// Session identity.
        id: Uuid,
    },
    /// Trace and manifest have both been published successfully.
    Completed {
        /// Recording metadata.
        recording: Recording,
        /// Native saved directory; UI formats its display separately.
        directory: PathBuf,
    },
    /// No trace was published for this cancelled session.
    Cancelled {
        /// Session identity.
        id: Uuid,
    },
    /// Terminal setup, backend or publication failure.
    Failed {
        /// Session identity.
        id: Uuid,
        /// Actionable diagnostic.
        error: String,
    },
}

impl CaptureState {
    /// Whether this owner still has a nonterminal recording.
    pub fn active(&self) -> bool {
        matches!(
            self,
            Self::Starting { .. } | Self::Recording { .. } | Self::Stopping { .. }
        )
    }

    /// The recording session identity, if any.
    pub fn id(&self) -> Option<Uuid> {
        match self {
            Self::Idle => None,
            Self::Completed { recording, .. } => Some(recording.id),
            Self::Starting { id }
            | Self::Recording { id }
            | Self::Stopping { id }
            | Self::Cancelled { id }
            | Self::Failed { id, .. } => Some(*id),
        }
    }
}
