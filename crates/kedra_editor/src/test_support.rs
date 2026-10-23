//! Headless integration fixtures: production source/policies/views, no Winit or renderer.
#![allow(clippy::unwrap_used, reason = "Headless fixture assertions")]

use bevy::{
    asset::{AssetApp, AssetLoader, AssetMetaCheck, AssetPlugin, LoadContext, io::Reader},
    input::InputPlugin,
    input_focus::InputFocus,
    prelude::*,
    state::app::StatesPlugin,
};
use kedra_scene::{
    assets::{CatalogLoader, PrefabSources, compile_catalog},
    composition::Catalog,
    document::PrefabDocument,
    reflection::ComponentPolicies,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

/// Deliberately not SVG rendering: supply labeled CPU images so real authored
/// view hydration can be tested independently of processing/GPU acceptance.
#[derive(Default, TypePath)]
struct CpuIcons;
impl AssetLoader for CpuIcons {
    type Asset = kedra_assets::images::ImageVariants;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        _: &mut dyn Reader,
        _: &(),
        context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        Ok(kedra_assets::images::ImageVariants {
            images: (1..=4)
                .map(|density| context.add_labeled_asset(format!("{density}x"), Image::default()))
                .collect(),
        })
    }
    fn extensions(&self) -> &[&str] {
        &["svg"]
    }
}

pub(crate) fn source_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/editor")
}

pub(crate) fn app() -> App {
    app_with_catalog("scenes/startup.prefabs.ron", false)
}

#[cfg(debug_assertions)]
pub(crate) fn diagnostic_app() -> App {
    app_with_catalog("scenes/debug.prefabs.ron", true)
}

fn app_with_catalog(catalog_path: &str, diagnostic: bool) -> App {
    let mut app = App::new();
    app.register_asset_source(
        "editor",
        bevy::asset::io::AssetSourceBuilder::new(|| {
            Box::new(bevy::asset::io::file::FileAssetReader::new(source_root()))
        }),
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            meta_check: AssetMetaCheck::Never,
            use_asset_processor_override: Some(false),
            watch_for_changes_override: Some(false),
            ..default()
        },
        StatesPlugin,
        InputPlugin,
        kedra_desktop::window_plugin(),
        bevy::input_focus::InputFocusPlugin,
        bevy::input_focus::InputDispatchPlugin,
        bevy::text::TextPlugin,
        bevy::ui_widgets::TextInputPlugin,
    ))
    .init_asset::<Font>()
    .init_asset::<Image>()
    .init_asset::<kedra_assets::images::ImageVariants>()
    .init_asset_loader::<bevy::text::FontLoader>()
    .init_asset_loader::<CpuIcons>()
    .init_resource::<InputFocus>()
    .init_resource::<UiScale>()
    .init_resource::<bevy::picking::events::PointerState>()
    .add_message::<bevy::picking::events::PointerRelease>()
    .add_plugins((
        kedra_app::KedraAppPlugin,
        kedra_desktop::DesktopPlugin,
        kedra_ui::authored::AuthoredUiPlugin,
        kedra_scene::assets::SceneAssetPlugin,
        crate::EditorPlugin,
    ));
    // Delivery verification has separate real-I/O fixtures. Hold this CPU view
    // host in windowless startup until each test explicitly chooses a workflow.
    app.world_mut().resource_mut::<crate::boot::Boot>().started = Some(Duration::ZERO);
    // Asset scheduling must not consume bootstrap deadlines. Individual tests
    // advance the clock explicitly; wall time is only a deadlock watchdog.
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::ZERO,
    ));
    #[cfg(debug_assertions)]
    {
        crate::remote::install(&mut app);
        if diagnostic {
            crate::diagnostics::install(&mut app).unwrap();
        }
    }
    #[cfg(not(debug_assertions))]
    let _ = diagnostic;
    let mut pending_sources = PrefabSources::default();
    pending_sources.pending = true;
    app.insert_resource(pending_sources);
    {
        let mut manager = app
            .world_mut()
            .resource_mut::<crate::model::ProjectManager>();
        manager.history_path = None;
        manager.history_writable = false;
    }
    // Flair completes CSS shorthand registration in Plugin::finish. Source
    // compilation requests async stylesheet loads, so finish before those
    // readers can overlap its nonblocking registry write locks.
    app.finish();
    app.cleanup();
    let mut catalog = Catalog::default();
    let mut native_documents = Vec::new();
    for index in [catalog_path] {
        for uri in
            CatalogLoader::parse(&std::fs::read_to_string(source_root().join(index)).unwrap())
                .unwrap()
        {
            let path = uri.strip_prefix("editor://").unwrap();
            let document =
                PrefabDocument::parse(&std::fs::read_to_string(source_root().join(path)).unwrap())
                    .unwrap();
            if !catalog.documents.contains_key(&document.id) {
                let handle = app
                    .world()
                    .resource::<AssetServer>()
                    .load::<PrefabDocument>(uri);
                app.world_mut()
                    .resource_mut::<Assets<PrefabDocument>>()
                    .insert(handle.id(), document.clone())
                    .unwrap();
                native_documents.push(handle);
                catalog.insert(document).unwrap();
            }
        }
    }
    assert_eq!(
        app.plugins_state(),
        bevy::app::PluginsState::Cleaned,
        "Finish plugin registration before compiling sources that request async CSS assets"
    );
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let mut loader = app.world().resource::<AssetServer>();
    let compiled = compile_catalog(
        &catalog,
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
        &mut loader,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if compiled.values().all(|snapshot| {
            snapshot
                .asset_status(app.world().resource::<AssetServer>())
                .unwrap()
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "CPU fixture dependencies did not finish loading"
        );
        std::thread::yield_now();
    }
    let mut sources = PrefabSources::default();
    sources.handle = app
        .world_mut()
        .resource_mut::<Assets<kedra_scene::assets::PrefabCatalogAsset>>()
        .add(kedra_scene::assets::PrefabCatalogAsset {
            documents: native_documents,
        });
    sources.managed = true;
    sources.publish(catalog, compiled).unwrap();
    sources.dependencies_ready = true;
    app.insert_resource(sources);
    // Never consult or persist the account's real preferences in any test.
    let mut manager = app
        .world_mut()
        .resource_mut::<crate::model::ProjectManager>();
    manager.history_path = None;
    manager.history_writable = false;
    app
}

pub(crate) fn element(world: &mut World, key: &str) -> Entity {
    let matches: Vec<_> = world
        .query::<(Entity, &kedra_ui::ElementId)>()
        .iter(world)
        .filter(|(_, id)| id.0 == key)
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(matches.len(), 1, "required unambiguous binding {key}");
    matches[0]
}

pub(crate) fn projects(app: &mut App) {
    app.world_mut().resource_mut::<crate::boot::Boot>().complete = true;
    app.world_mut()
        .resource_mut::<NextState<crate::AppPhase>>()
        .set(crate::AppPhase::Projects);
    for _ in 0..4 {
        app.update();
    }
    assert!(
        app.world()
            .resource::<kedra_ui::authored::components::UiFailure>()
            .0
            .is_none()
    );
    assert_eq!(
        *app.world().resource::<State<crate::AppPhase>>().get(),
        crate::AppPhase::Projects
    );
    element(app.world_mut(), "projects.new");
}

pub(crate) fn assert_failure(world: &World, expected: &str) {
    assert!(
        world
            .resource::<kedra_ui::authored::components::UiFailure>()
            .0
            .as_ref()
            .unwrap()
            .contains(expected)
    );
    assert!(
        world
            .resource::<Messages<AppExit>>()
            .get_cursor()
            .read(world.resource::<Messages<AppExit>>())
            .any(AppExit::is_error)
    );
}
