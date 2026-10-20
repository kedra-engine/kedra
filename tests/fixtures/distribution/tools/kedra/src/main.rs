//! Generated projects need only this thin pinned tooling entry point.
fn main() -> std::process::ExitCode {
    kedra_build::run_cli(std::env::args_os())
}
