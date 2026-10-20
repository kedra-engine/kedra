//! Project-local commands; no repository-specific paths or implicit editor launch.
use crate::{
    config::{Application, NativeTarget},
    pipeline,
};
use clap::{Args, Parser, Subcommand};
use std::{ffi::OsString, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    name = "cargo kedra",
    version,
    about = "Build applications and verified native packages"
)]
struct Options {
    #[command(subcommand)]
    command: Command,
}

#[derive(Args)]
struct AppArgs {
    #[arg(long, default_value = "Cargo.toml")]
    manifest_path: PathBuf,
    #[arg(long)]
    package: Option<String>,
    #[arg(long)]
    target: Option<NativeTarget>,
}

#[derive(Subcommand)]
enum Command {
    #[command(hide = true)]
    BundleNative {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    /// Explicit source-asset authoring operations, without the editor application.
    Assets {
        #[command(subcommand)]
        command: AssetCommand,
    },
    /// Generate native icon inputs from a self-contained SVG without an app build.
    Icons {
        source: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Compile and prepare a fresh verified resource distribution.
    Build {
        #[command(flatten)]
        app: AppArgs,
        #[arg(long)]
        release: bool,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Build a release application and its native installable packages.
    Package {
        #[command(flatten)]
        app: AppArgs,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum AssetCommand {
    /// Assign missing sidecars without replacing existing resource identities.
    Init { source: PathBuf },
}

pub(crate) fn run(args: impl IntoIterator<Item = OsString>) -> ExitCode {
    let options = match Options::try_parse_from(args) {
        Ok(options) => options,
        Err(error) => {
            let code = if error.use_stderr() { 2 } else { 0 };
            if let Err(print_error) = error.print() {
                eprintln!("FAILED: cannot print command diagnostics: {print_error}");
                return ExitCode::FAILURE;
            }
            return ExitCode::from(code);
        }
    };
    match execute(options.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("FAILED: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute(command: Command) -> Result<(), String> {
    let (args, release, output, package) = match command {
        Command::BundleNative { config, report } => {
            return crate::worker::execute(&config, &report);
        }
        Command::Assets {
            command: AssetCommand::Init { source },
        } => {
            eprintln!("Initializing asset identities in {}", source.display());
            let count = kedra_import::initialize(&source, |line| eprintln!("{line}"))?;
            eprintln!("SUCCESS: initialized {count} missing sidecars; existing metadata unchanged");
            return Ok(());
        }
        Command::Icons { source, output } => {
            let files = crate::icons::generate(&source, &output)?;
            eprintln!(
                "SUCCESS: generated {} PNG variants and application.ico in {}",
                files.len(),
                output.display()
            );
            return Ok(());
        }
        Command::Build {
            app,
            release,
            output,
        } => (app, release, output, false),
        Command::Package { app, output } => (app, true, Some(output), true),
    };
    let application = Application::load(&args.manifest_path, args.package.as_deref(), args.target)?;
    pipeline::execute(&application, release, output.as_deref(), package)
}
