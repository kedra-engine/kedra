//! Installed game-style scene consumer: no editor, UI, authoring writer or source tree.
use bevy::{asset::AssetPlugin, prelude::*};
use kedra_scene::{
    assets::{PrefabPlugin, PrefabSources},
    document::{ComponentData, PrefabDocument},
    instance::PrefabInstance,
    reflection::ComponentPolicies,
};
use std::{
    collections::BTreeMap,
    fs,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
struct Health {
    value: u32,
}

#[test]
fn installed_catalog_uses_document_uuid_and_works_without_source_checkout() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    let mut document = PrefabDocument::new("Independent actor");
    document.nodes[0]
        .components
        .insert("game.health".into(), ComponentData::ron(1, "(value:42)")?);
    fs::write(source.join("actor.prefab.ron"), document.serialize()?)?;
    fs::write(
        source.join("catalog.prefabs.ron"),
        "[\"game://actor.prefab.ron\"]",
    )?;
    assert_eq!(kedra_import::initialize(&source, |_| {})?, 2);
    let meta = fs::read_to_string(source.join("actor.prefab.ron.meta"))?;
    assert!(meta.contains(&document.id.to_string()));
    let output = temp.path().join("installed");
    let manifest = kedra_import::build(&source, &output, "game-desktop", "game", |_| {})?;
    let catalog = manifest
        .assets
        .iter()
        .find(|asset| asset.path == "catalog.prefabs.ron")
        .ok_or("catalog missing")?
        .id;
    fs::rename(&source, temp.path().join("offline-source"))?;
    let mut app = App::new();
    kedra_assets::register_source(
        &mut app,
        "game",
        output.to_str().ok_or("non-Unicode temp path")?.into(),
        manifest,
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            use_asset_processor_override: Some(false),
            watch_for_changes_override: Some(false),
            ..default()
        },
    ))
    .init_resource::<AppTypeRegistry>()
    .add_plugins(PrefabPlugin {
        catalog_path: "game://catalog.prefabs.ron".into(),
    });
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    app.world_mut()
        .resource_mut::<ComponentPolicies>()
        .register::<Health>(&mut registry.write(), "game.health", 1, |_| Ok(()))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let sources = app.world().resource::<PrefabSources>();
        if let Some(error) = &sources.error {
            return Err(error.clone().into());
        }
        if sources.ready() {
            break;
        }
        if Instant::now() > deadline {
            return Err("scene loading timed out".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let snapshot = app.world().resource::<PrefabSources>().snapshot();
    let actor =
        snapshot.compiled[&document.id].instantiate(app.world_mut(), None, &BTreeMap::new())?;
    let by_uuid: Handle<kedra_scene::assets::PrefabCatalogAsset> = app
        .world()
        .resource::<AssetServer>()
        .load(format!("game://{catalog}/asset.prefabs.ron"));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app
        .world()
        .resource::<AssetServer>()
        .is_loaded_with_dependencies(by_uuid.id())
    {
        app.update();
        if Instant::now() > deadline {
            return Err("UUID catalog reference timed out".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        app.world()
            .get::<Health>(actor)
            .ok_or("health missing")?
            .value,
        42
    );
    assert_eq!(
        app.world()
            .get::<PrefabInstance>(actor)
            .ok_or("instance owner missing")?
            .document,
        document.id
    );
    fs::write(output.join("actor.prefab.ron"), "corrupt source")?;
    assert!(kedra_assets::delivery::read_directory(&output, "game-desktop").is_err());
    Ok(())
}

#[test]
fn mismatched_native_identity_and_missing_catalog_member_reject_delivery() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    let document = PrefabDocument::new("Actor");
    fs::write(source.join("actor.prefab.ron"), document.serialize()?)?;
    kedra_import::initialize(&source, |_| {})?;
    let original = fs::read_to_string(source.join("actor.prefab.ron.meta"))?;
    fs::write(
        source.join("actor.prefab.ron.meta"),
        original.replace(&document.id.to_string(), &uuid::Uuid::new_v4().to_string()),
    )?;
    let output = temp.path().join("rejected");
    assert!(kedra_import::build(&source, &output, "game-desktop", "game", |_| {}).is_err());
    assert!(!output.exists());
    fs::write(source.join("actor.prefab.ron.meta"), &original)?;
    fs::write(
        source.join("catalog.prefabs.ron"),
        format!("[\"game://{}/asset.prefab.ron\"]", uuid::Uuid::new_v4()),
    )?;
    kedra_import::initialize(&source, |_| {})?;
    let error = kedra_import::build(&source, &output, "game-desktop", "game", |_| {})
        .err()
        .ok_or("missing member was accepted")?;
    assert!(error.contains("dependency could not load"), "{error}");
    assert!(!output.exists());
    assert_eq!(
        fs::read_to_string(source.join("actor.prefab.ron.meta"))?,
        original
    );
    Ok(())
}
