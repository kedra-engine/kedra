//! Compile and mount the production source, not the synthetic workspace catalog.
use bevy::{asset::AssetApp, prelude::*};
use kedra_scene::{
    composition::Catalog, document::PrefabDocument, instance::CompiledPrefab,
    reflection::ComponentPolicies,
};
use kedra_ui::authored::{
    AuthoredUiPlugin,
    components::{UiButton, UiImage},
};
use std::collections::BTreeMap;

#[test]
fn scene_controls_are_reachable_native_targets_with_local_density_icons()
-> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .add_plugins(AuthoredUiPlugin);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/editor");
    let document: PrefabDocument = ron::de::from_bytes(&std::fs::read(
        root.join("prefabs/source-document.prefab.ron"),
    )?)?;
    let id = document.id;
    let mut catalog = Catalog::default();
    catalog.insert(document)?;
    let registry = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let compiled = CompiledPrefab::compile(
        catalog.resolve(id)?,
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
        &mut loader,
        &BTreeMap::new(),
    )?;
    let source = compiled.authoring_world();
    let elements = compiled
        .source_entities()
        .values()
        .filter_map(|entity| {
            source
                .get::<kedra_ui::ElementId>(*entity)
                .map(|id| (id.0.as_str(), *entity))
        })
        .collect::<BTreeMap<_, _>>();
    assert!(!elements.contains_key("toolbar"));
    for name in [
        "hand",
        "move",
        "rotate",
        "scale",
        "snap",
        "settings",
        "orbit",
        "frame",
        "projection",
    ] {
        let entity = *elements
            .get(name)
            .ok_or_else(|| format!("Missing functional target: {name}"))?;
        assert!(source.get::<UiButton>(entity).is_some(), "{name}");
        let viewport = elements["viewport"];
        if [
            "move",
            "rotate",
            "scale",
            "snap",
            "orbit",
            "frame",
            "projection",
        ]
        .contains(&name)
        {
            let mut ancestor = entity;
            loop {
                ancestor = source
                    .get::<ChildOf>(ancestor)
                    .ok_or("Detached scene tool")?
                    .parent();
                if ancestor == viewport {
                    break;
                }
            }
        }
    }
    for entity in compiled.source_entities().values() {
        if let Some(image) = source.get::<UiImage>(*entity) {
            assert_eq!(image.variants.len(), 4);
            for (index, variant) in image.variants.iter().enumerate() {
                let path = variant
                    .image
                    .path()
                    .ok_or("Image has no delivery identity")?;
                assert_eq!(variant.density, (index + 1) as f32);
                assert_eq!(path.source().as_str(), Some("editor"));
                assert_eq!(path.label(), Some(format!("{}x", index + 1).as_str()));
                assert!(!std::fs::read(root.join(path.path()))?.is_empty());
            }
        }
    }
    let transform_row = source
        .get::<ChildOf>(elements["move"])
        .ok_or("Move row missing")?
        .parent();
    let shared_panel = source
        .get::<ChildOf>(transform_row)
        .ok_or("Tool panel missing")?
        .parent();
    {
        let name = "frame";
        assert_eq!(
            source
                .get::<ChildOf>(elements[name])
                .ok_or("Navigation panel missing")?
                .parent(),
            shared_panel
        );
    }
    assert_eq!(
        source
            .get::<ChildOf>(elements["settings"])
            .ok_or("Settings panel missing")?
            .parent(),
        shared_panel
    );
    Ok(())
}
