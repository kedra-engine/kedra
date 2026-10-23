//! Public build/CLI contracts using real independent consumers, not Cargo self-tests.
#[path = "contracts/build.rs"]
mod build;
#[path = "contracts/cli.rs"]
mod cli;
