//! Minimal project-local launcher; native workers must receive original arguments.
fn main() -> std::process::ExitCode {
    kedra_build::run_cli(std::env::args_os())
}
