//! A failed native dependency must become an error, not an endless readiness wait.
use bevy::{
    asset::{AssetApp, AssetPlugin},
    prelude::*,
};
use kedra_scene::{
    assets::{PrefabPlugin, PrefabSources, compile_catalog},
    composition::Catalog,
    document::PrefabDocument,
    reflection::ComponentPolicies,
};
use std::{
    collections::BTreeMap,
    fs,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Asset, Reflect)]
struct WithDependency {
    #[dependency]
    child: Handle<PrefabDocument>,
}
#[derive(Default, TypePath)]
struct DependencyLoader;
impl bevy::asset::AssetLoader for DependencyLoader {
    type Asset = WithDependency;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        _: &mut dyn bevy::asset::io::Reader,
        _: &(),
        context: &mut bevy::asset::LoadContext<'_>,
    ) -> std::io::Result<WithDependency> {
        Ok(WithDependency {
            child: context.load("missing.prefab.ron"),
        })
    }
    fn extensions(&self) -> &[&str] {
        &["dependency"]
    }
}
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
struct AssetReference {
    asset: Handle<WithDependency>,
}

#[test]
fn recursive_failure_reaches_asset_status_and_native_or_managed_sources() -> Result<()> {
    for managed in [false, true] {
        let temp = tempfile::tempdir()?;
        fs::write(temp.path().join("root.dependency"), "dummy")?;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: temp.path().to_str().ok_or("non-Unicode temp path")?.into(),
                watch_for_changes_override: Some(false),
                use_asset_processor_override: Some(false),
                ..default()
            },
            PrefabPlugin {
                catalog_path: "catalog.prefabs.ron".into(),
            },
        ));
        app.init_asset::<WithDependency>()
            .register_asset_reflect::<WithDependency>()
            .init_asset_loader::<DependencyLoader>();
        let types = app.world().resource::<AppTypeRegistry>().clone();
        let mut policies = ComponentPolicies::default();
        policies.register::<AssetReference>(&mut types.write(), "asset", 1, |_| Ok(()))?;
        let server = app.world().resource::<AssetServer>().clone();
        let handle: Handle<WithDependency> = server.load("root.dependency");
        let mut document = PrefabDocument::new("Asset");
        document.nodes[0].components.insert(
            "asset".into(),
            policies.encode(
                "asset",
                &AssetReference {
                    asset: handle.clone(),
                },
                &types.read(),
                &BTreeMap::new(),
            )?,
        );
        fs::write(temp.path().join("root.prefab.ron"), document.serialize()?)?;
        fs::write(
            temp.path().join("catalog.prefabs.ron"),
            "[\"root.prefab.ron\"]",
        )?;
        let mut catalog = Catalog::default();
        catalog.insert(document.clone())?;
        let compiled = compile_catalog(&catalog, &types.read(), &policies, &mut &server)?;
        let snapshot = compiled[&document.id].clone();
        app.insert_resource(policies);
        app.world_mut()
            .resource_mut::<PrefabSources>()
            .publish(catalog, compiled)?;
        app.world_mut().resource_mut::<PrefabSources>().managed = managed;
        app.finish();
        app.cleanup();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            if matches!(
                server.get_recursive_dependency_load_state(handle.id()),
                Some(bevy::asset::RecursiveDependencyLoadState::Failed(_))
            ) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "missing dependency must reach a terminal load state"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(matches!(
            server.get_load_state(handle.id()),
            Some(bevy::asset::LoadState::Loaded)
        ));
        assert!(snapshot.asset_status(&server).is_err());
        app.update();
        let sources = app.world().resource::<PrefabSources>();
        assert!(!sources.ready());
        assert!(
            sources
                .error
                .as_ref()
                .is_some_and(|error| error.contains("missing.prefab.ron"))
        );
        let before = app.world().entities().count_spawned();
        let failure = snapshot
            .instantiate(app.world_mut(), None, &BTreeMap::new())
            .err()
            .ok_or("failed dependency instantiated")?;
        assert!(failure.to_string().contains("missing.prefab.ron"));
        assert_eq!(app.world().entities().count_spawned(), before);
    }
    Ok(())
}
