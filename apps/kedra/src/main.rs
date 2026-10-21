//! Desktop and asset-tool entry point; no project code is built or executed.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
use clap::{Parser, Subcommand};
use std::{path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    version,
    about = "Kedra native editor and shared engine asset pipeline"
)]
struct Cli {
    /// Existing project root to open through the normal validated lifecycle.
    project: Option<PathBuf>,
    /// Verified editor delivery directory (default: assets next to executable).
    #[arg(long, global = true)]
    assets: Option<PathBuf>,
    /// Verified game-namespace delivery for the explicitly opened project's Scene View.
    /// No assets are built and no project code runs.
    #[arg(long, requires = "project")]
    project_assets: Option<PathBuf>,
    /// Enable authenticated engine automation on loopback (zero selects a port).
    #[arg(long, requires = "remote_credentials")]
    remote_port: Option<u16>,
    /// NEW local credential file; the token is never placed in command arguments.
    #[arg(long, requires = "remote_port")]
    remote_credentials: Option<PathBuf>,
    /// Directory for remote renderer captures.
    #[arg(long, default_value = "target/qa/captures")]
    remote_artifacts: PathBuf,
    /// Override local preference storage, e.g. to isolate a native QA session.
    #[arg(long)]
    state_directory: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_assets_override_environment_and_broken_executable_lookup() {
        let root = asset_root(
            Some(PathBuf::from("explicit")),
            Some("environment".into()),
            Err(std::io::Error::other("executable unavailable")),
        );
        assert_eq!(root, Ok(PathBuf::from("explicit")));
        let root = asset_root(
            None,
            Some("environment".into()),
            Err(std::io::Error::other("executable unavailable")),
        );
        assert_eq!(root, Ok(PathBuf::from("environment")));
        assert!(asset_root(None, None, Err(std::io::Error::other("unavailable"))).is_err());
    }

    #[test]
    fn automation_requires_explicit_credentials_for_every_port() {
        assert!(Cli::try_parse_from(["kedra", "--project-assets", "delivery"]).is_err());
        assert!(Cli::try_parse_from(["kedra", "project", "--project-assets", "delivery"]).is_ok());
        for port in ["0", "12345"] {
            assert!(Cli::try_parse_from(["kedra", "--remote-port", port]).is_err());
            assert!(
                Cli::try_parse_from([
                    "kedra",
                    "--remote-port",
                    port,
                    "--remote-credentials",
                    "session.json"
                ])
                .is_ok()
            );
        }
        assert!(Cli::try_parse_from(["kedra", "--remote-credentials", "session.json"]).is_err());
        assert!(Cli::try_parse_from(["kedra", "--state-directory", "isolated-state"]).is_ok());
    }

    #[test]
    fn bundled_asset_verification_accepts_default_lookup_without_relaxing_flags() {
        assert!(Cli::try_parse_from(["kedra", "assets", "verify"]).is_ok());
        assert!(Cli::try_parse_from(["kedra", "assets", "verify", "--profile"]).is_err());
        assert!(Cli::try_parse_from(["kedra", "assets", "verify", "--unknown"]).is_err());
    }

    #[test]
    fn asset_cli_requires_explicit_namespace_and_complete_unambiguous_commands() {
        for args in [
            vec!["kedra", "assets"],
            vec!["kedra", "assets", "build", "source", "output"],
            vec!["kedra", "assets", "init"],
            vec!["kedra", "assets", "verify", "delivery", "--unknown"],
            vec!["kedra", "--remote-port", "65536"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
        assert!(
            Cli::try_parse_from([
                "kedra",
                "assets",
                "build",
                "source",
                "output",
                "--namespace",
                "game",
                "--profile",
                "game-desktop",
            ])
            .is_ok()
        );
    }

    #[test]
    fn asset_cli_propagates_verification_and_publication_refusals_without_modifying_source() {
        let existing_file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let before = std::fs::read(&existing_file).unwrap_or_else(|error| panic!("{error}"));
        let verify = Cli::try_parse_from([
            std::ffi::OsString::from("kedra"),
            "assets".into(),
            "verify".into(),
            existing_file.as_os_str().to_owned(),
        ])
        .unwrap_or_else(|error| panic!("{error}"));
        assert!(run(verify).is_err());
        let build = Cli::try_parse_from([
            std::ffi::OsString::from("kedra"),
            "assets".into(),
            "build".into(),
            existing_file.as_os_str().to_owned(),
            existing_file.as_os_str().to_owned(),
            "--namespace".into(),
            "game".into(),
        ])
        .unwrap_or_else(|error| panic!("{error}"));
        let Err(error) = run(build) else {
            panic!("Existing delivery target was accepted");
        };
        assert!(error.contains("new directory"));
        assert_eq!(
            std::fs::read(existing_file).unwrap_or_else(|error| panic!("{error}")),
            before
        );
    }
}
#[derive(Subcommand)]
enum Command {
    /// Headless asset tooling shared by editor resources and game projects.
    Assets {
        #[command(subcommand)]
        command: AssetCommand,
    },
}
#[derive(Subcommand)]
enum AssetCommand {
    /// Assign missing UUID/native Bevy sidecars without changing existing IDs.
    Init { source: PathBuf },
    /// Process and validate sources into a NEW immutable delivery directory.
    Build {
        source: PathBuf,
        output: PathBuf,
        #[arg(long, default_value = "editor-desktop")]
        profile: String,
        /// Explicit logical source used by delivered UUID references (e.g. editor).
        #[arg(long)]
        namespace: String,
    },
    /// Check all manifest members, hashes and loader metadata, without writes.
    Verify {
        /// Explicit delivery; omitted uses the same platform-relative lookup as startup.
        root: Option<PathBuf>,
        #[arg(long, default_value = "editor-desktop")]
        profile: String,
    },
}
fn asset_root(
    explicit: Option<PathBuf>,
    environment: Option<std::ffi::OsString>,
    executable: std::io::Result<PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(root) = explicit.or_else(|| environment.map(PathBuf::from)) {
        return Ok(root);
    }
    let executable = executable.map_err(|error| format!("Cannot locate executable: {error}"))?;
    kedra_assets::location::bundled_asset_root(
        &executable,
        kedra_assets::location::BundlePlatform::current(),
    )
    .map_err(|error| format!("Cannot locate editor assets; pass --assets PATH: {error}"))
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Some(Command::Assets { command }) => match command {
            AssetCommand::Init { source } => {
                eprintln!("Initializing asset identities in {}", source.display());
                let count = kedra_import::initialize(&source, |line| eprintln!("{line}"))?;
                eprintln!(
                    "SUCCESS: initialized {count} missing sidecars; existing metadata unchanged"
                );
                Ok(())
            }
            AssetCommand::Build {
                source,
                output,
                profile,
                namespace,
            } => {
                eprintln!(
                    "Building asset delivery: {} → {}",
                    source.display(),
                    output.display()
                );
                let manifest =
                    kedra_import::build(&source, &output, &profile, &namespace, |line| {
                        eprintln!("{line}")
                    })?;
                eprintln!(
                    "SUCCESS: {} assets delivered to {}",
                    manifest.assets.len(),
                    output.display()
                );
                Ok(())
            }
            AssetCommand::Verify { root, profile } => {
                let root = asset_root(
                    root.or(cli.assets),
                    std::env::var_os("KEDRA_ASSETS"),
                    std::env::current_exe(),
                )?;
                eprintln!("Verifying asset delivery {}", root.display());
                let (_, manifest) = kedra_assets::delivery::read_directory(&root, &profile)?;
                eprintln!("SUCCESS: {} assets verified", manifest.assets.len());
                Ok(())
            }
        },
        None => {
            let root = asset_root(
                cli.assets,
                std::env::var_os("KEDRA_ASSETS"),
                std::env::current_exe(),
            )?;
            let remote = match cli.remote_port {
                Some(port) => Some(kedra_editor::RemoteOptions {
                    port,
                    credentials: cli
                        .remote_credentials
                        .ok_or("--remote-credentials PATH is required with --remote-port")?,
                    artifacts: cli.remote_artifacts,
                }),
                None => None,
            };
            kedra_editor::run(
                cli.project,
                root,
                remote,
                cli.state_directory,
                cli.project_assets,
            )
        }
    }
}
fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("FAILED: {error}");
            ExitCode::FAILURE
        }
    }
}
