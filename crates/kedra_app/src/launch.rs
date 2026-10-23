//! Shared application CLI configuration, without editor or test-runner dependencies.
use bevy::prelude::*;
use clap::Args;
use kedra_scene::startup::StartupScenePlugin;
#[cfg(debug_assertions)]
use std::path::PathBuf;

/// Optional debug-only remote flags. Release parsers expose none of these fields.
/// Cargo features cannot restore the excluded transport in a release profile.
#[derive(Args, Clone, Debug)]
#[cfg_attr(not(debug_assertions), derive(Default))]
pub struct RemoteOptions {
    /// Enable authenticated loopback automation; zero selects an available port.
    #[cfg(debug_assertions)]
    #[arg(long, requires = "remote_credentials")]
    pub remote_port: Option<u16>,
    /// NEW private credential file. The bearer token is never a CLI argument.
    #[cfg(debug_assertions)]
    #[arg(long, requires = "remote_port")]
    pub remote_credentials: Option<PathBuf>,
    /// Renderer artifact directory, separate from project sources.
    #[cfg(debug_assertions)]
    #[arg(long, default_value = "target/test/captures")]
    pub remote_artifacts: PathBuf,
}
#[cfg(debug_assertions)]
impl Default for RemoteOptions {
    fn default() -> Self {
        Self {
            remote_port: None,
            remote_credentials: None,
            remote_artifacts: PathBuf::from("target/test/captures"),
        }
    }
}
impl RemoteOptions {
    /// Apply diagnostic configuration after the host registers its native APIs.
    /// A failure must abort application launch; it does not authorize running a
    /// partially configured application. Release has no remote fields or transport.
    pub fn configure(self, app: &mut App) -> Result<(), String> {
        #[cfg(debug_assertions)]
        {
            match (self.remote_port, self.remote_credentials) {
                (None, None) => (),
                (Some(port), Some(credentials)) => {
                    if !app.world().contains_resource::<crate::AppSession>() {
                        return Err(
                            "Install KedraAppPlugin before configuring remote access".into()
                        );
                    }
                    if self.remote_artifacts.as_os_str().is_empty() {
                        return Err(
                            "Remote artifacts require an explicit nonempty directory".into()
                        );
                    }
                    let endpoint =
                        crate::transport::enable(app, port, &credentials, &self.remote_artifacts)?;
                    eprintln!(
                        "Automation configured at {}; credential publication pending: {}",
                        endpoint.url,
                        credentials.display()
                    );
                }
                _ => {
                    return Err(
                        "Remote access requires both --remote-port and --remote-credentials".into(),
                    );
                }
            }
        }
        #[cfg(not(debug_assertions))]
        let _ = (self, app);
        Ok(())
    }
}

/// Standard application flags, flattened into a game's own clap Parser.
/// Normal entry-scene configuration is supplied by trusted host code; only debug
/// builds expose its CLI override. Scene selection never grants script execution.
#[derive(Args, Clone, Debug, Default)]
pub struct LaunchOptions {
    /// Override the configured startup scene with another host-catalog asset path.
    #[cfg(debug_assertions)]
    #[arg(long)]
    pub scene: Option<String>,
    /// Explicit local diagnostic access, omitted from release CLI schemas.
    #[command(flatten)]
    pub remote: RemoteOptions,
}
impl LaunchOptions {
    /// Apply common options to an already composed Bevy application.
    /// The host selects native plugins, sources and the normal startup hydrator;
    /// CLI input cannot load Rust plugins or replace the host's capability set.
    /// Errors abort launch, never fall back to another scene or remote policy.
    pub fn configure(self, app: &mut App, startup: StartupScenePlugin) -> Result<(), String> {
        #[cfg(debug_assertions)]
        let startup = {
            let mut startup = startup;
            if let Some(path) = self.scene {
                startup.path = path;
            }
            startup
        };
        if startup.path.is_empty()
            || startup.path.len() > 4096
            || bevy::asset::AssetPath::try_parse(startup.path.as_str()).is_err()
        {
            return Err("Invalid startup scene asset path".into());
        }
        app.add_plugins(startup);
        self.remote.configure(app)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    #[derive(Parser)]
    struct Application {
        #[command(flatten)]
        launch: LaunchOptions,
    }
    #[test]
    fn standard_flags_follow_the_build_profile_and_never_relax_credentials() {
        assert!(
            Application::try_parse_from(["game"])
                .map(|app| app.launch)
                .is_ok()
        );
        #[cfg(debug_assertions)]
        {
            let parsed = Application::try_parse_from([
                "game",
                "--scene",
                "game://Scenes/Arena.scene.ron",
                "--remote-port",
                "0",
                "--remote-credentials",
                "session.json",
            ]);
            assert!(parsed.is_ok());
            assert!(Application::try_parse_from(["game", "--remote-port", "0"]).is_err());
            assert!(
                Application::try_parse_from(["game", "--remote-credentials", "session.json"])
                    .is_err()
            );
        }
        #[cfg(not(debug_assertions))]
        for args in [
            vec!["game", "--scene", "game://Scenes/Arena.scene.ron"],
            vec!["game", "--remote-port", "0"],
            vec!["game", "--remote-credentials", "session.json"],
            vec!["game", "--remote-artifacts", "frames"],
        ] {
            assert!(Application::try_parse_from(args).is_err());
        }
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn release_application_does_not_install_brp_or_diagnostic_methods() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            crate::KedraAppPlugin,
        ));
        assert!(
            !app.world()
                .contains_resource::<bevy_remote::RemoteMethods>()
        );
    }
}
