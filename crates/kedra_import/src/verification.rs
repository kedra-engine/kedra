//! Verify the actual installed manifest source, not a permissive filesystem approximation.
use bevy::{
    asset::{AssetApp, AssetPlugin, LoadState},
    prelude::*,
};
use kedra_assets::delivery::Manifest;
use kedra_scene::{assets::SceneAssetPlugin, composition::Catalog, document::PrefabDocument};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(crate) fn install_models(app: &mut App) {
    app.add_plugins(bevy::mesh::MeshPlugin)
        .init_asset::<bevy::pbr::StandardMaterial>()
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .insert_resource(bevy::image::CompressedImageFormatSupport(
            bevy::image::CompressedImageFormats::NONE,
        ))
        .add_plugins(bevy::gltf::GltfPlugin::default());
}

pub(crate) fn verify_processed(
    root: &Path,
    manifest: &Manifest,
    namespace: &str,
    progress: &mut impl FnMut(&str),
) -> Result<(), String> {
    manifest.verify(&manifest.profile, |path| {
        std::fs::read(root.join(path)).map_err(|e| e.to_string())
    })?;
    let mut app = App::new();
    kedra_assets::register_source(
        &mut app,
        namespace.to_owned(),
        root.to_str().ok_or("Delivery path must be Unicode")?.into(),
        manifest.clone(),
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            use_asset_processor_override: Some(false),
            watch_for_changes_override: Some(false),
            ..default()
        },
        bevy::image::ImagePlugin::default(),
    ));
    app.register_asset_loader(bevy::image::ImageLoader::new(
        bevy::image::CompressedImageFormats::all(),
    ))
    .init_asset::<Font>()
    .init_asset_loader::<bevy::text::FontLoader>()
    .add_plugins((
        kedra_assets::KedraAssetsPlugin,
        SceneAssetPlugin,
        kedra_ui::styles::StyleAssetsPlugin,
    ));
    install_models(&mut app);
    app.finish();
    app.cleanup();
    let server = app.world().resource::<AssetServer>().clone();
    let mut handles = Vec::new();
    for asset in &manifest.assets {
        // Metadata selects the real loader; paths still pass through manifest integrity checks.
        handles.push(
            server
                .load_builder()
                .load_untyped(format!("{namespace}://{}", asset.path)),
        );
    }
    progress("Verifying native runtime loading through the manifest-checked asset source");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        app.update();
        let mut ready = true;
        for handle in &handles {
            if let Some(LoadState::Failed(error)) = server.get_load_state(handle.id()) {
                return Err(format!("Processed runtime asset could not load: {error}"));
            }
            if let Some(bevy::asset::RecursiveDependencyLoadState::Failed(error)) =
                server.get_recursive_dependency_load_state(handle.id())
            {
                return Err(format!(
                    "Processed runtime dependency could not load: {error}"
                ));
            }
            ready &= server.is_loaded_with_dependencies(handle.id());
        }
        if ready {
            break;
        }
        if Instant::now() > deadline {
            return Err("Runtime asset/dependency verification timed out".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut catalog = Catalog::default();
    for (_, document) in app.world().resource::<Assets<PrefabDocument>>().iter() {
        catalog
            .insert(document.clone())
            .map_err(|e| e.to_string())?;
    }
    for id in catalog.documents.keys() {
        catalog.resolve(*id).map_err(|e| e.to_string())?;
    }
    Ok(())
}
