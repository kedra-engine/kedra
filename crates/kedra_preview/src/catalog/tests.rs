//! Native scene-label provenance and selected-catalog retention, without claiming
//! a GLTF loader or rendered-model interaction check.
use super::*;
use bevy::{asset::AssetApp, world_serialization::WorldAsset};
#[test]
fn choices_use_only_native_scene_paths_of_current_admitted_parents()
-> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            meta_check: bevy::asset::AssetMetaCheck::Never,
            use_asset_processor_override: Some(false),
            watch_for_changes_override: Some(false),
            ..default()
        },
    ));
    app.init_asset::<Gltf>().init_asset::<WorldAsset>();
    let root = app.world_mut().spawn_empty().id();
    let paths = vec!["game://ship.glb".to_owned()];
    let (initial, _) = choices(app.world_mut(), root, &paths);
    assert!(
        initial.is_empty(),
        "unloaded metadata must not invent Scene0"
    );
    let handle = app
        .world()
        .get::<Catalog>(root)
        .ok_or("retained metadata")?
        .handles[0]
        .clone();
    let server = app.world().resource::<AssetServer>();
    let scene = server.load::<WorldAsset>("game://ship.glb#Scene7");
    let foreign = server.load::<WorldAsset>("game://other.glb#Scene0");
    app.world_mut().resource_mut::<Assets<Gltf>>().insert(
        handle.id(),
        Gltf {
            scenes: vec![scene.clone(), foreign],
            named_scenes: default(),
            meshes: Vec::new(),
            named_meshes: default(),
            materials: Vec::new(),
            named_materials: default(),
            nodes: Vec::new(),
            named_nodes: default(),
            skins: Vec::new(),
            named_skins: default(),
            default_scene: Some(scene),
            source: None,
        },
    )?;
    let (ready, changed) = choices(app.world_mut(), root, &paths);
    assert!(changed);
    assert_eq!(ready, ["game://ship.glb#Scene7"]);
    let (unchanged, changed) = choices(app.world_mut(), root, &paths);
    assert!(!changed);
    assert_eq!(unchanged, ready);
    let (retired, changed) = choices(app.world_mut(), root, &[]);
    assert!(changed && retired.is_empty());
    assert!(
        app.world()
            .get::<Catalog>(root)
            .ok_or("catalog")?
            .handles
            .is_empty()
    );
    Ok(())
}
