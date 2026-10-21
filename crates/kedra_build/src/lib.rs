//! Shared authoring-side application build and desktop distribution tooling.
//! This crate is not a runtime dependency and never opens an editor.
mod cli;
mod compiler;
mod config;
mod icons;
mod inventory;
mod licenses;
mod native_elf;
mod native_licenses;
mod native_runtime;
mod package;
mod pipeline;
mod worker;

use std::{ffi::OsString, io::IsTerminal, process::ExitCode};

/// Run project-local build/package arguments, including the launcher name.
///
/// This explicitly executes Cargo, project build scripts and native packaging;
/// callers must trust the project, just as for `cargo build`. It never opens an
/// editor, changes process working directory or installs system tools. Diagnostics
/// are streamed; argument errors return 2, operation failures 1, and success 0.
/// Existing distribution outputs are never overwritten. Failed fresh outputs are
/// retained for inspection and are not reported as successful packages.
/// Native packaging re-executes the launcher with internal arguments. Launchers
/// must forward `std::env::args_os()` unchanged. Only the child receives native
/// helper environment and working-directory settings; the caller is unchanged.
pub fn run_cli(args: impl IntoIterator<Item = OsString>) -> ExitCode {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .with_target(false)
        .without_time()
        .try_init();
    cli::run(args)
}
