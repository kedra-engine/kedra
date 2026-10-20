//! Project-first native editor lifecycle and a retained dockable workspace.
//! Scene/document editing and executable viewports are not implemented.
//!
//! The editor owns domain state, desktop owns native surfaces, the UI kit owns
//! controls and `kedra_project` owns source validation and creation.
mod actions;
mod asset_browser;
mod boot;
mod chrome;
mod display_paths;
mod documents;
pub mod domain;
mod entry;
mod entry_preview;
mod hierarchy;
mod jobs;
mod loading;
mod model;
mod recent;
mod remote;
mod scene_view;
mod screens;
mod views;
mod workspace;
use bevy::prelude::*;
pub use model::AppPhase;
use std::path::PathBuf;
/// Run the opt-in isolated dock acceptance host, without editor startup/tools.
/// Only available in builds explicitly enabling the `docking-fixture` feature.
#[cfg(feature = "docking-fixture")]
pub use workspace::fixture::run as run_docking_fixture;

/// Startup plugin. Install after Bevy defaults, native widget/navigation plugins,
/// DesktopPlugin and AuthoredUiPlugin.
pub struct EditorPlugin;
impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        chrome::install(app);
        asset_browser::install(app);
        hierarchy::install(app);
        scene_view::install(app);
        workspace::install(app);
        app.init_state::<AppPhase>()
            .init_resource::<actions::native::Pending>()
            .init_resource::<model::ProjectManager>()
            .init_resource::<model::EntryForm>()
            .init_resource::<recent::RecentView>()
            .init_resource::<views::ViewClock>()
            .add_observer(actions::activated)
            .add_observer(actions::request_close)
            .add_systems(OnExit(AppPhase::Editor), workspace::retire)
            .add_observer(actions::submit_key)
            .add_systems(
                PostUpdate,
                actions::native::apply.after(bevy::text::EditableTextSystems),
            )
            .init_resource::<boot::Boot>()
            .add_systems(OnEnter(AppPhase::Boot), boot::enter)
            .add_systems(OnEnter(AppPhase::Projects), screens::projects)
            .add_systems(
                OnEnter(AppPhase::Loading),
                (loading::enter, jobs::prepare).chain(),
            )
            .add_systems(OnEnter(AppPhase::Editor), screens::editor)
            .add_systems(
                Update,
                (
                    screens::sync_fields,
                    jobs::poll,
                    boot::created,
                    views::hydrate,
                    boot::poll,
                    recent::poll,
                    jobs::finish_loading,
                    recent::sync,
                    entry_preview::sync,
                    screens::sync_status,
                    loading::trace,
                    model::project_phase,
                )
                    .chain(),
            );
    }
}

/// Run one desktop process, optionally opening a root through the same validated
/// lifecycle as UI selection. No project code, build script or plugin is run.
/// `project_assets`, when supplied, requires an explicit project and registers a
/// verified immutable `game` delivery before AssetPlugin initialization. Open
/// separately verifies correspondence with admitted sources; a delivery is never
/// inferred from a project path or built as a side effect.
pub fn run(
    project: Option<PathBuf>,
    assets: PathBuf,
    remote: Option<RemoteOptions>,
    state_directory: Option<PathBuf>,
    project_assets: Option<PathBuf>,
) -> Result<(), String> {
    let (assets, manifest) = kedra_assets::delivery::read_manifest(&assets, "editor-desktop")?;
    let mut app = App::new();
    if let Some(delivery) = project_assets {
        let source = kedra_project::open(
            project
                .as_ref()
                .ok_or("Project assets require an explicit project root")?,
        )
        .map_err(|e| e.to_string())?
        .assets;
        eprintln!("Verifying Scene View delivery: {}", delivery.display());
        let (root, manifest) = kedra_assets::delivery::read_directory(&delivery, "editor-desktop")?;
        kedra_assets::register_source(
            &mut app,
            "game",
            root.to_str().ok_or("Delivery path must be Unicode")?.into(),
            manifest.clone(),
        );
        app.insert_resource(scene_view::binding::Delivery { source, manifest });
        eprintln!("Scene View delivery verified; source matching is checked on document Open");
    }
    kedra_assets::register_source(
        &mut app,
        "editor",
        assets
            .to_str()
            .ok_or("Asset delivery root must be Unicode")?
            .into(),
        manifest.clone(),
    );
    app.add_plugins(DefaultPlugins.set(kedra_desktop::window_plugin()).set(
        bevy::asset::AssetPlugin {
            use_asset_processor_override: Some(false),
            watch_for_changes_override: Some(false),
            ..default()
        },
    ))
    .add_plugins((
        kedra_app::KedraAppPlugin,
        kedra_desktop::DesktopPlugin,
        bevy::input_focus::tab_navigation::TabNavigationPlugin,
        kedra_ui::authored::AuthoredUiPlugin,
        kedra_ui::tooltip::TooltipPlugin,
        kedra_scene::assets::PrefabPlugin {
            catalog_path: "editor://scenes/boot.prefabs.ron".into(),
        },
        EditorPlugin,
    ));
    remote::install(&mut app);
    app.world_mut().resource_mut::<boot::Boot>().delivery = Some((assets.clone(), manifest));
    if let Some(directory) = state_directory {
        let directory = std::path::absolute(directory)
            .map_err(|error| format!("Cannot resolve state directory: {error}"))?;
        let mut manager = app.world_mut().resource_mut::<model::ProjectManager>();
        manager.history_path = Some(directory.join("recent-projects.toml"));
        manager.history_writable = true;
    }
    if let Some(options) = remote {
        let endpoint = kedra_app::transport::enable(
            &mut app,
            options.port,
            &options.credentials,
            &options.artifacts,
        )?;
        eprintln!(
            "Automation listening at {}; credentials: {}",
            endpoint.url,
            options.credentials.display()
        );
    }
    if let Some(path) = project {
        app.world_mut()
            .resource_mut::<model::ProjectManager>()
            .initial_project = Some(path);
    }
    if std::env::var("KEDRA_REDUCED_MOTION")
        .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    {
        app.world_mut()
            .resource_mut::<kedra_ui::authored::components::UiMotion>()
            .reduced = true;
    }
    let exit = app.run();
    if exit.is_error() {
        Err("Editor stopped with an error; inspect the diagnostics above.".into())
    } else {
        Ok(())
    }
}

/// Explicit local automation configuration. Disabled when absent.
pub struct RemoteOptions {
    /// Loopback TCP port; zero chooses an available port.
    pub port: u16,
    /// New credential file; never overwritten or printed as a token.
    pub credentials: PathBuf,
    /// Captured artifact directory, separate from project source files.
    pub artifacts: PathBuf,
}

#[cfg(test)]
mod startup_tests;
#[cfg(test)]
mod tests;
