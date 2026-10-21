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
    /// Explicitly prepare user-local build tools; never installs system SDKs.
    Tools {
        #[command(subcommand)]
        command: ToolCommand,
    },
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
    /// Build, verify, then explicitly launch a fresh standalone application.
    Run {
        #[command(flatten)]
        app: AppArgs,
        #[arg(long)]
        release: bool,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(last = true)]
        arguments: Vec<OsString>,
    },
    /// Compile and prepare a fresh verified runnable application.
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

#[derive(Subcommand)]
enum ToolCommand {
    /// Install checksum-verified cargo-about with an isolated advisory-audited lock.
    InstallAbout {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        archive: Option<PathBuf>,
    },
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
    if let Command::Run {
        app,
        release,
        output,
        arguments,
    } = command
    {
        let application =
            Application::load(&app.manifest_path, app.package.as_deref(), app.target)?;
        let built = pipeline::execute(&application, release, output.as_deref(), false)?;
        return crate::launch::request(&application, &built, &arguments);
    }
    let (args, release, output, package) = match command {
        Command::Run { .. } => unreachable!("Run handled before non-launching operations"),
        Command::Tools {
            command: ToolCommand::InstallAbout { output, archive },
        } => {
            return crate::tools::install_about(&output, archive.as_deref());
        }
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
    pipeline::execute(&application, release, output.as_deref(), package).map(|_| ())
}
