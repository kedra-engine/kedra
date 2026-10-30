//! Resource-free source preview and dependency refusal are independent of a rendered frame.
use bevy::prelude::*;
use kedra_authoring::persistence::SourceSnapshot;
use kedra_authoring_assets::compile_preview;
use kedra_scene::{
    composition::Catalog,
    document::{ComponentData, PrefabDocument},
    reflection::ComponentPolicies,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn resource_free_preview_compiles_but_a_resource_reference_requires_a_delivery() -> TestResult {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_resource::<ComponentPolicies>();
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    kedra_scene::spatial::register(
        &mut registry.write(),
        &mut app.world_mut().resource_mut::<ComponentPolicies>(),
    )?;
    let policies = app.world().resource::<ComponentPolicies>();
    let mut source = SourceSnapshot {
        environments: Default::default(),
        namespace: "game".into(),
        generation: 1,
        documents: vec![],
        assets: vec![],
        index: None,
    };
    let document = PrefabDocument::new("Empty");
    let bytes = document.serialize()?;
    source
        .documents
        .push(kedra_authoring::persistence::SourceFile {
            path: "new.scene.ron".into(),
            hash: kedra_assets::delivery::digest(bytes.as_bytes()),
            source: bytes,
        });
    let id = document.id;
    let mut catalog = Catalog::default();
    catalog.insert(document)?;
    let graph = catalog.resolve(id)?;
    let compiled = compile_preview(
        graph.clone(),
        &registry.read(),
        policies,
        app.world().resource::<AssetServer>(),
        &source,
        std::path::Path::new("assets"),
        None,
    )?;
    assert_eq!(compiled.graph(), &graph);
    let delivery = kedra_authoring_assets::Delivery {
        source: "assets".into(),
        manifest: kedra_assets::delivery::Manifest {
            format: 1,
            profile: "editor-desktop".into(),
            assets: vec![],
            files: Default::default(),
        },
    };
    // The authored graph is authoritative, even when a launch delivery predates it.
    assert!(delivery.matches(&delivery.source, &source).is_err());
    let compiled = compile_preview(
        graph.clone(),
        &registry.read(),
        policies,
        app.world().resource::<AssetServer>(),
        &source,
        &delivery.source,
        Some(&delivery),
    )?;
    assert_eq!(compiled.graph(), &graph);
    assert!(
        compile_preview(
            graph.clone(),
            &registry.read(),
            policies,
            app.world().resource::<AssetServer>(),
            &source,
            std::path::Path::new("foreign"),
            Some(&delivery)
        )
        .is_err()
    );
    catalog.documents.get_mut(&id).ok_or("document")?.nodes[0]
        .components
        .insert(
            "scene.model".into(),
            ComponentData::ron(1, "(scene:Path(\"game://character.glb#Scene0\"))")?,
        );
    let result = compile_preview(
        catalog.resolve(id)?,
        &registry.read(),
        policies,
        app.world().resource::<AssetServer>(),
        &source,
        std::path::Path::new("assets"),
        None,
    );
    let error = result
        .err()
        .ok_or("resource-bearing graph bypassed delivery admission")?;
    assert!(error.contains("matching project asset delivery"), "{error}");
    Ok(())
}
