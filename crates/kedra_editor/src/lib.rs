//! Project-first native editor lifecycle and a retained dockable workspace.
//! Scene/document editing and executable viewports are not implemented.
//!
//! The editor owns domain state, desktop owns native surfaces, the UI kit owns
//! controls and `kedra_project` owns source validation and creation.
mod actions;
mod asset_browser;
mod boot;
#[cfg(debug_assertions)]
mod catalog;
mod chrome;
pub mod context;
#[cfg(debug_assertions)]
mod diagnostics;
mod display_paths;
mod documents;
pub mod domain;
pub mod drag;
pub mod edit_scripts;
mod entry;
mod entry_preview;
mod hierarchy;
mod inspector;
mod jobs;
mod loading;
mod model;
mod profiling;
mod recent;
#[cfg(debug_assertions)]
mod remote;
mod scene_view;
mod screens;
#[cfg(debug_assertions)]
mod scripting;
mod updates;
mod views;
mod workspace;
use bevy::prelude::*;
pub use model::AppPhase;
use std::path::PathBuf;

/// Startup plugin. Install after Bevy defaults, native widget/navigation plugins,
/// DesktopPlugin and AuthoredUiPlugin.
pub struct EditorPlugin;

/// Selects startup authority, not a different set of native editor capabilities.
#[derive(Resource, Default)]
struct ExplicitScene(bool);

fn project_lifecycle(scene: Res<ExplicitScene>) -> bool {
    !scene.0
}

fn wake_startup(phase: Res<State<AppPhase>>, mut demand: ResMut<kedra_wakeup::WakeDemand>) {
    if matches!(phase.get(), AppPhase::Boot | AppPhase::Loading) {
        demand.active();
    }
}

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        let wakeup = kedra_wakeup::Wakeup::for_world(app.world());
        app.insert_resource(wakeup);
        app.init_resource::<kedra_wakeup::WakeDemand>();
        if let Err(error) = kedra_ui::docking::scene::register(app) {
            app.world_mut()
                .resource_mut::<kedra_ui::authored::components::UiFailure>()
                .0 = Some(error.to_string());
        }
        chrome::install(app);
        updates::install(app);
        asset_browser::install(app);
        hierarchy::install(app);
        drag::install(app);
        inspector::install(app);
        scene_view::install(app);
        workspace::install(app);
        app.init_resource::<ExplicitScene>()
            .init_state::<AppPhase>()
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
            .add_systems(
                OnEnter(AppPhase::Projects),
                screens::projects.run_if(project_lifecycle),
            )
            .add_systems(
                OnEnter(AppPhase::Loading),
                (loading::enter, jobs::prepare)
                    .chain()
                    .run_if(project_lifecycle),
            )
            .add_systems(
                OnEnter(AppPhase::Editor),
                screens::editor.run_if(project_lifecycle),
            )
            .add_systems(
                Update,
                (
                    screens::sync_fields,
                    jobs::poll,
                    views::hydrate,
                    boot::poll,
                    recent::poll,
                    jobs::finish_loading,
                    loading::show_if_needed,
                    recent::sync,
                    entry_preview::sync,
                    screens::sync_status,
                    loading::trace,
                    model::project_phase,
                    wake_startup,
                )
                    .chain()
                    .run_if(project_lifecycle),
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
    launch: kedra_app::launch::LaunchOptions,
    profiling: kedra_profiling::CaptureOptions,
    state_directory: Option<PathBuf>,
    project_assets: Option<PathBuf>,
    editor_script: Option<edit_scripts::Launch>,
) -> Result<(), String> {
    #[cfg(debug_assertions)]
    if launch.scene.is_some() && (project.is_some() || project_assets.is_some()) {
        return Err("Explicit startup scenes cannot run the project lifecycle concurrently".into());
    }
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
        app.insert_resource(kedra_authoring_assets::Delivery { source, manifest });
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
    kedra_authoring_assets::environments::install(&mut app, "game");
    app.add_plugins(
        DefaultPlugins
            .set(bevy::log::LogPlugin {
                custom_layer: kedra_profiling::log_layer,
                ..default()
            })
            .set(kedra_desktop::window_plugin())
            .set(bevy::asset::AssetPlugin {
                use_asset_processor_override: Some(false),
                watch_for_changes_override: Some(false),
                ..default()
            }),
    )
    .add_plugins((
        kedra_app::KedraAppPlugin,
        kedra_desktop::DesktopPlugin,
        bevy::input_focus::tab_navigation::TabNavigationPlugin,
        kedra_ui::authored::AuthoredUiPlugin,
        kedra_ui::tooltip::TooltipPlugin,
        EditorPlugin,
    ));
    profiling::configure(&mut app, profiling)?;
    // The bounded safety sweep services third-party assets/deadlines without a
    // public wake seam. Known active work retains the former focused cadence;
    // timed UI and preferences can shorten the idle wait. Games opt in separately.
    app.add_plugins(kedra_wakeup::ReactivePlugin {
        idle: std::time::Duration::from_millis(100),
        focused_active: std::time::Duration::from_secs_f64(1. / 60.),
        unfocused_active: std::time::Duration::from_millis(100),
    });
    if let Some(script) = editor_script {
        edit_scripts::configure(
            &mut app,
            project
                .as_deref()
                .ok_or("Editor script execution requires an explicit project")?,
            script,
        )?;
    }
    #[cfg(debug_assertions)]
    {
        diagnostics::install(&mut app)?;
        remote::install(&mut app);
    }
    #[cfg(debug_assertions)]
    if launch.scene.is_some() {
        app.world_mut().resource_mut::<ExplicitScene>().0 = true;
        app.add_plugins(kedra_scene::assets::PrefabPlugin {
            catalog_path: "editor://scenes/debug.prefabs.ron".into(),
        });
        launch.configure(
            &mut app,
            kedra_scene::startup::StartupScenePlugin {
                path: "editor://scenes/editor.scene.ron".into(),
                mount: diagnostics::mount,
            },
        )?;
        return if app.run().is_error() {
            Err("Startup scene stopped with an error; inspect diagnostics".into())
        } else {
            Ok(())
        };
    }
    app.add_plugins(kedra_scene::assets::PrefabPlugin {
        catalog_path: "editor://scenes/startup.prefabs.ron".into(),
    });
    updates::configure(app.world_mut(), state_directory.as_deref())?;
    app.world_mut().resource_mut::<boot::Boot>().delivery = Some((assets.clone(), manifest));
    if let Some(directory) = state_directory {
        let directory = std::path::absolute(directory)
            .map_err(|error| format!("Cannot resolve state directory: {error}"))?;
        let mut manager = app.world_mut().resource_mut::<model::ProjectManager>();
        manager.history_path = Some(directory.join("recent-projects.toml"));
        manager.history_writable = true;
    }
    launch.remote.configure(&mut app)?;
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
    let handoff = updates::handoff(app.world_mut());
    let exit = app.run();
    let update = handoff.try_recv().ok();
    drop(app);
    if exit.is_error() {
        Err("Editor stopped with an error; inspect the diagnostics above.".into())
    } else {
        if let Some((prepared, arguments)) = update
            && let Err(error) = prepared.commit(&arguments)
        {
            rfd::MessageDialog::new()
                .set_title("Kedra update failed")
                .set_level(rfd::MessageLevel::Error)
                .set_description(&error)
                .show();
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod startup_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
