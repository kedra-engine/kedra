//! Project-local launcher; all build policy belongs to the shared tooling crate.
fn main() -> std::process::ExitCode {
    kedra_build::run_cli(std::env::args_os())
}
