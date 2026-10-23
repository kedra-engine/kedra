//! Tooling-side native tests for ordinary project-owned Rust scenarios.
//! Applications use `kedra_app` at runtime; this crate is never a runtime plugin.
//! Failed runs retain evidence, never approve sources or choose another baseline.
pub mod cargo;
mod client;
pub mod evidence;
pub mod process;
pub mod visual;
pub use client::Client;
pub mod comparison;
pub mod report;
pub mod suite;
pub mod text;

/// A tooling failure with a visible diagnostic; callers must retain failed evidence.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
