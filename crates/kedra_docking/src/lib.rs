//! Docking authority as ordinary Rust values, independent of entities and windows.
//!
//! [`model::Workspace`] owns validated topology. Prepare a command, inspect its
//! candidate, then commit against the unchanged workspace. Content, documents,
//! native resources, pixel layout and close approval belong to adapters, not this
//! model. Snapshot decoding never publishes an unchecked topology.
//!
//! ```
//! use kedra_docking::{command::{Command, Destination}, model::{HostId, View, ViewId, Workspace}};
//! # fn main() -> kedra_docking::Result<()> {
//! let host = HostId::new();
//! let mut layout = Workspace::new(host);
//! let view = ViewId::new();
//! let pending = layout.prepare(Command::Open {
//!     id: view,
//!     view: View { kind: "example.inspector".into(), restore_key: None },
//!     destination: Destination::EmptyHost(host),
//! })?;
//! assert!(layout.snapshot().views.is_empty());
//! layout.commit(pending)?;
//! assert!(layout.snapshot().location(view).is_some());
//! # Ok(())
//! # }
//! ```
pub mod command;
pub mod model;
pub mod session;
pub mod snapshot;

/// A rejected operation. The published model is unchanged on every error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// An ID does not exist or is not of the required kind.
    #[error("docking target is missing or has the wrong kind")]
    Missing,
    /// The requested placement would consume its own destination.
    #[error("docking destination is part of the moved selection")]
    SelfTarget,
    /// The insertion boundary is outside the destination tab list.
    #[error("docking insertion index is out of range")]
    Index,
    /// The prepared operation no longer belongs to the published state.
    #[error("docking candidate is stale or belongs to a different workspace")]
    Stale,
    /// A numeric value is nonfinite or outside its allowed interval.
    #[error("docking split fraction must be finite and strictly between zero and one")]
    Fraction,
    /// Snapshot version, topology, identity or ownership failed validation.
    #[error("invalid docking snapshot: {0}")]
    Invalid(String),
    /// A bounded resource or revision counter would be exceeded.
    #[error("docking limit exceeded: {0}")]
    Limit(&'static str),
}

/// A docking operation which never mutates the source on failure.
pub type Result<T> = std::result::Result<T, Error>;
