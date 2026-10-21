//! Shared authoring-side application build and desktop distribution tooling.
//! This crate is not a runtime dependency. Only explicit `run` launches an app.
mod application;
mod cli;
mod compiler;
mod config;
mod icons;
mod inventory;
mod launch;
mod licenses;
mod native_elf;
mod native_licenses;
mod native_runtime;
mod package;
mod package_verify;
#[cfg(test)]
mod package_verify_tests;
mod pipeline;
mod tools;
mod verify;
mod worker;

#[cfg(test)]
mod application_tests;
#[cfg(test)]
mod test_support;

use std::{ffi::OsString, io::IsTerminal, process::ExitCode};

/// Run project-local build/package/run/tool arguments, including the launcher name.
///
/// This explicitly executes Cargo, project build scripts and native packaging;
/// callers must trust the project, just as for `cargo build`. Only `run` launches
/// the selected verified application. Tool installation is explicit and user-local;
/// system SDKs are never installed. The caller's working directory is unchanged. Diagnostics
/// are streamed; argument errors return 2, operation failures 1, and success 0.
/// Existing distribution outputs are never overwritten. Failed fresh outputs are
/// retained for inspection and are not reported as successful packages.
/// Native packaging re-executes the launcher with internal arguments. Launchers
/// must forward `std::env::args_os()` unchanged. Only the child receives native
/// helper environment and working-directory settings; the caller is unchanged.
/// Successful `run` means launch-request acceptance, not runtime exit success,
/// OS trust or GUI readiness. Direct-child logs remain in the build output;
/// macOS uses Launch Services. Applications are not killed when the CLI exits.
pub fn run_cli(args: impl IntoIterator<Item = OsString>) -> ExitCode {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .with_target(false)
        .without_time()
        .try_init();
    cli::run(args)
}
