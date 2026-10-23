//! Ordinary shared-project game: no editor, importer or test-tool dependency.
mod config;
#[cfg(debug_assertions)]
mod docking;
mod scripting;
use bevy::prelude::*;
use clap::Parser;
#[cfg(debug_assertions)]
use kedra_app::RemoteAppExt;
use kedra_scene::{
    assets::PrefabPlugin,
    startup::{StartupScene, StartupScenePlugin, StartupState},
};
use kedra_ui::authored::{AuthoredUiPlugin, view::UiView};

#[derive(Parser)]
struct Options {
    /// Required verified visual delivery; registers resources, never capabilities.
    #[arg(long)]
    assets: std::path::PathBuf,
    /// Required verified game delivery from the canonical test_project assets.
    #[arg(long)]
    project_assets: std::path::PathBuf,
    #[command(flatten)]
    launch: kedra_app::launch::LaunchOptions,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = Options::parse();
    let (root, manifest) =
        kedra_assets::delivery::read_directory(&options.assets, "editor-desktop")?;
    let (game_root, game_manifest) =
        kedra_assets::delivery::read_directory(&options.project_assets, "editor-desktop")?;
    let mut app = App::new();
    // One canonical cross-delivery catalogue, embedded as trusted host source.
    // It is not inside either independently verified delivery's asset manifest.
    let catalog = bevy::asset::io::memory::Dir::default();
    catalog.insert_asset_text(
        std::path::Path::new("catalogs/native.prefabs.ron"),
        config::CATALOG_SOURCE,
    );
    app.register_asset_source(
        "application",
        bevy::asset::io::AssetSourceBuilder::new(move || {
            Box::new(bevy::asset::io::memory::MemoryAssetReader {
                root: catalog.clone(),
            })
        }),
    );
    kedra_assets::register_source(
        &mut app,
        "game",
        game_root
            .to_str()
            .ok_or("Game delivery must be Unicode")?
            .into(),
        game_manifest,
    );
    kedra_assets::register_source(
        &mut app,
        "editor",
        root.to_str().ok_or("Delivery must be Unicode")?.into(),
        manifest,
    );
    app.add_plugins((
        DefaultPlugins
            .set(AssetPlugin {
                watch_for_changes_override: Some(false),
                use_asset_processor_override: Some(false),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Kedra Test Project".into(),
                    resolution: (320, 320).into(),
                    ..default()
                }),
                ..default()
            }),
        kedra_app::KedraAppPlugin,
        bevy::input_focus::tab_navigation::TabNavigationPlugin,
        AuthoredUiPlugin,
        kedra_script_bevy::ScriptBevyPlugin,
    ));
    // Inert schema is required by the fixed catalogue in release as well.
    config::register_schema(&mut app)?;
    #[cfg(debug_assertions)]
    docking::install(&mut app);
    app.add_plugins(PrefabPlugin {
        catalog_path: config::CATALOG.into(),
    });
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn(Camera2d);
    });
    app.add_systems(
        Update,
        |scene: Res<StartupScene>, mut exit: MessageWriter<AppExit>| {
            if let StartupState::Failed(error) = scene.state() {
                eprintln!("Startup scene {} failed: {error}", scene.path());
                exit.write(AppExit::error());
            }
        },
    );
    #[cfg(debug_assertions)]
    kedra_app::scripting::register(
        &mut app,
        kedra_script::Program::sequence(
            "scene.remote",
            "scripts/Native/remote.luau",
            include_str!("../scripts/Native/remote.luau"),
            kedra_app::scripting::install_app_api,
        ),
    );
    #[cfg(debug_assertions)]
    kedra_app::scripting::register(
        &mut app,
        kedra_script::Program::sequence(
            "scene.switch",
            "scripts/Native/switch.luau",
            include_str!("../scripts/Native/switch.luau"),
            kedra_app::scripting::install_app_api,
        ),
    );
    // Optional manual diagnostic shortcut. Automated shutdown uses the ordinary
    // native close request and independently checks the child process exit.
    #[cfg(debug_assertions)]
    app.add_systems(
        Update,
        |keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>| {
            if keys.just_pressed(KeyCode::F8) {
                exit.write(AppExit::Success);
            }
        },
    );
    #[cfg(debug_assertions)]
    app.register_remote("game.state", "Read independent game renderer identity", serde_json::json!({}),
        |_: In<Option<serde_json::Value>>, adapter: Res<bevy::render::renderer::RenderAdapterInfo>| -> bevy_remote::BrpResult {
            Ok(serde_json::json!({"renderer":{"name":adapter.name,"backend":format!("{:?}",adapter.backend),"driver":adapter.driver,"driver_info":adapter.driver_info}}))
        });
    options.launch.configure(
        &mut app,
        StartupScenePlugin {
            path: config::STARTUP_SCENE.into(),
            mount: mount,
        },
    )?;
    if app.run().is_error() {
        return Err("Independent host failed".into());
    }
    Ok(())
}

fn mount(
    world: &mut World,
    scene: &std::sync::Arc<kedra_scene::instance::CompiledPrefab>,
) -> kedra_scene::Result<Entity> {
    let canvas = UiView::mount(
        world,
        scene,
        None,
        &std::collections::BTreeMap::new(),
        "",
        1.,
    )?;
    if let Err(error) = scripting::bind(world, &canvas) {
        world.despawn(canvas.root);
        return Err(kedra_scene::Error(error.to_string()));
    }
    #[cfg(debug_assertions)]
    if let Err(error) = docking::bind(world, &canvas) {
        world.despawn(canvas.root);
        return Err(kedra_scene::Error(error));
    }
    Ok(canvas.root)
}
