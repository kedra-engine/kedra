//! Native asset path resolution never falls back to UUID dispatch or another revision.
use super::*;
use bevy::asset::AssetPlugin;
use kedra_scene::{
    assets::compile_catalog, composition::Catalog, document::DocumentKind,
    reflection::ComponentPolicies,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture() -> Result<App, Box<dyn std::error::Error>> {
    let mut app = App::new();
    test_support::install(&mut app);
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        kedra_scene::assets::SceneAssetPlugin,
    ))
    .init_resource::<AppTypeRegistry>()
    .init_resource::<ComponentPolicies>();
    let prefab = PrefabDocument::new("Value presentation");
    let mut scene = PrefabDocument::new("Entry composition");
    scene.kind = DocumentKind::Scene;
    let documents = vec![
        ("editor://prefabs/value.prefab.ron".into(), prefab.clone()),
        ("editor://scenes/entry.scene.ron".into(), scene.clone()),
    ];
    let mut catalog = Catalog::default();
    catalog.insert(prefab)?;
    catalog.insert(scene)?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let compiled = compile_catalog(
        &catalog,
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
        &mut app.world().resource::<AssetServer>(),
    )?;
    let mut sources = PrefabSources::default();
    sources.handle = test_support::materialize(app.world_mut(), documents)?;
    sources.publish(catalog, compiled)?;
    sources.dependencies_ready = true;
    app.insert_resource(sources);
    Ok(app)
}

#[test]
fn loaded_member_paths_resolve_prefabs_and_scenes_without_numeric_identity() -> TestResult {
    let app = fixture()?;
    assert!(compiled(app.world(), "editor://prefabs/value.prefab.ron").is_ok());
    assert!(compiled(app.world(), "editor://scenes/entry.scene.ron").is_ok());
    assert!(compiled(app.world(), "prefabs/value.prefab.ron").is_err());
    assert!(compiled(app.world(), "editor://prefabs/missing.prefab.ron").is_err());
    assert!(compiled(app.world(), "editor://prefabs/value.prefab.ron#label").is_err());
    Ok(())
}

#[test]
fn unready_unlisted_missing_and_changed_native_assets_fail_closed() -> TestResult {
    let mut app = fixture()?;
    let path = "editor://prefabs/value.prefab.ron";
    app.world_mut().resource_mut::<PrefabSources>().pending = true;
    assert!(compiled(app.world(), path).is_err());
    app.world_mut().resource_mut::<PrefabSources>().pending = false;
    let outside = test_support::materialize(
        app.world_mut(),
        vec![(
            "editor://prefabs/outside.prefab.ron".into(),
            PrefabDocument::new("Outside"),
        )],
    )?;
    assert!(
        compiled(app.world(), "editor://prefabs/outside.prefab.ron")
            .is_err_and(|e| e.contains("member"))
    );
    // Retain the outside catalog only as native storage, not admitted authority.
    assert!(
        app.world()
            .resource::<Assets<PrefabCatalogAsset>>()
            .get(&outside)
            .is_some()
    );
    let handle = app
        .world()
        .resource::<AssetServer>()
        .get_handle::<PrefabDocument>(path)
        .ok_or("Missing test handle")?;
    let document = app
        .world_mut()
        .resource_mut::<Assets<PrefabDocument>>()
        .remove(handle.id())
        .ok_or("Missing test document")?;
    assert!(compiled(app.world(), path).is_err_and(|e| e.contains("materialized")));
    let mut changed = document;
    changed.title = "Changed before publication".into();
    app.world_mut()
        .resource_mut::<Assets<PrefabDocument>>()
        .insert(handle.id(), changed)?;
    assert!(compiled(app.world(), path).is_err_and(|e| e.contains("published source revision")));
    Ok(())
}
