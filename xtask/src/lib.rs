//! Repository checks shared by contributors, automation and CI.
mod architecture;
mod cli;
mod files;
mod output;
mod visual;

pub use architecture::allowed;
use clap::Parser;
use std::{env, error::Error, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Run the requested repository check, with a menu on an interactive terminal.
///
/// Child output is streamed and recorded; failures stop the remaining stages.
/// CI and redirected output never prompt or render terminal animation.
pub fn run() -> Result<()> {
    let options = cli::Options::parse();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("workspace root missing")?;
    env::set_current_dir(root)?;
    let interactive = cli::interactive(&options);
    let task = cli::choose_task(options.task, interactive)?;
    if task == cli::Task::Test && !options.test_arguments.is_empty() {
        if options.visual.is_present() {
            return Err("Pass native test options after --".into());
        }
        return kedra_test::cargo::run(
            &root.join("tools/test/Cargo.toml"),
            "kedra-test",
            "test",
            false,
            &options.test_arguments,
        );
    }
    if !options.test_arguments.is_empty() {
        return Err("Trailing scenario arguments require test".into());
    }
    if task == cli::Task::Visual {
        return visual::run(&options.visual, interactive);
    }
    if options.visual.is_present() {
        return Err("visual options require the visual task".into());
    }
    let stages = task.stages();
    let mut output = output::Session::new(interactive, task.name())?;
    let mut result = Ok(());
    for (index, stage) in stages.iter().enumerate() {
        result = output.stage(
            index + 1,
            stages.len(),
            stage.label(),
            |output| match stage {
                cli::Stage::Files => output.message(&files::check()?),
                cli::Stage::Architecture => output.message(&architecture::check()?),
                cli::Stage::Cargo { args, rustdoc, .. } => output.cargo(args, *rustdoc),
            },
        );
        if result.is_err() {
            break;
        }
    }
    output.finish(stages.len())?;
    result
}
