//! Bounded GitHub release discovery, verified package ownership and explicit native
//! installation. No Bevy, UI, scheduler, document settlement or implicit process exit.
//!
//! The consumer supplies a trusted [`Config`], schedules blocking IO off its frame
//! loop and separately authorizes download and installation. HTTPS and SHA-256 trust
//! the configured GitHub repository; they are not independent publisher signatures.
//! Dropping a [`VerifiedPackage`] or [`Prepared`] cancels its owned staging. Commit
//! is explicit and must run only after the host has settled work and stopped its UI.

mod config;
mod download;
mod installation;
mod macos;
mod network;
mod release;

pub use config::Config;
pub use download::{VerifiedPackage, stage};
pub use installation::{detect_at as detect_installation, reveal_command};
pub use macos::{Installation as MacInstallation, OpenError, Prepared, prepare as prepare_macos};
pub use network::discover;
pub use release::{Kind, Release};
pub use semver::Version;

use std::path::Path;

/// Validates a bounded release listing and the selected candidate's inventory.
///
/// Returns `None` when no newer official early-access release exists, and an error
/// for invalid/incomplete inventory or foreign assets. Performs no network or disk IO.
/// The existing protocol requires all five desktop payloads plus inventory/checksums.
pub fn select_release(
    config: &Config,
    listing: &[u8],
    inventory: &[u8],
    kind: Kind,
) -> Result<Option<Release>, String> {
    let Some(candidate) = release::candidate(config, listing)? else {
        return Ok(None);
    };
    release::validate(config, &candidate, inventory, kind).map(Some)
}

/// Downloads an explicitly selected release with a ten-minute transfer timeout.
///
/// Creates private staging beneath `cache`. Calls `progress` with received bytes
/// and `verifying` after the completed file is closed, before integrity verification.
/// Returns an owned package only after exact size/SHA-256 verification; errors
/// remove partial staging. Callbacks run synchronously on the caller's worker.
pub fn download(
    release: &Release,
    cache: &Path,
    progress: impl FnMut(u64),
    verifying: impl FnOnce(),
) -> Result<VerifiedPackage, String> {
    let mut response = network::get(&release.url, std::time::Duration::from_secs(600))?;
    stage(
        response.body_mut().as_reader(),
        release,
        cache,
        progress,
        verifying,
    )
}
