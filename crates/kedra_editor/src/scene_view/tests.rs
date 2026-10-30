use super::test_support::compiled;
use super::*;
use bevy::camera::visibility::RenderLayers;
use kedra_scene::{composition::Catalog, document::PrefabDocument, reflection::ComponentPolicies};
use std::collections::BTreeMap;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn unchanged_selection_after_delayed_mount_publishes_marker_and_frame_selection_bounds()
-> TestResult {
    let mut world = World::new();
    let compiled = compiled(&mut world)?;
    world.init_resource::<Assets<bevy::gizmos::GizmoAsset>>();
    let element = world.spawn_empty().id();
    let mut view = View {
        root: element,
        viewport: element,
        render: element,
        information: element,
        heading: element,
        controls: default(),
        marker: default(),
        compiled: Some(compiled.clone()),
        pending_source: None,
        projection: Some(projection::Projection::pending(1)),
        lifecycle: default(),
        elapsed: 0.,
        raw_model: false,
        source_removed: false,
        source_layer: 0,
        requested_source: None,
    };
    let selected = compiled.graph().root.clone();
    // This is the production select boundary while tick is waiting for resources.
    view.select(&mut world, Some(&selected))?;
    view.select(&mut world, Some(&selected))?;
    assert!(
        world
            .resource::<Assets<bevy::gizmos::GizmoAsset>>()
            .is_empty()
    );
    let projection = view.projection.as_mut().ok_or("Projection missing")?;
    projection.mount(&mut world, compiled)?;
    let camera = projection.camera().ok_or("Camera missing")?;
    view.select(&mut world, Some(&selected))?;
    assert_eq!(
        world.resource::<Assets<bevy::gizmos::GizmoAsset>>().len(),
        1
    );
    assert_eq!(
        world
            .get::<camera::Navigation>(camera)
            .ok_or("Navigation missing")?
            .selection,
        Some((Vec3::splat(-1.), Vec3::splat(1.)))
    );
    camera::command(&mut world, camera, camera::Command::FrameSelection)?;
    view.select(&mut world, Some(&selected))?;
    assert_eq!(
        world.resource::<Assets<bevy::gizmos::GizmoAsset>>().len(),
        1
    );
    view.select(&mut world, None)?;
    assert_eq!(
        world
            .get::<camera::Navigation>(camera)
            .ok_or("Navigation missing")?
            .selection,
        None
    );
    assert!(camera::command(&mut world, camera, camera::Command::FrameSelection).is_err());
    view.retire(&mut world);
    view.select(&mut world, Some(&selected))?;
    assert!(
        world
            .resource::<Assets<bevy::gizmos::GizmoAsset>>()
            .is_empty()
    );
    assert!(world.get::<Camera>(camera).is_none());
    Ok(())
}

#[test]
fn native_projection_is_disposable_isolated_and_never_writes_compilation() -> TestResult {
    let mut world = World::new();
    let compiled = compiled(&mut world)?;
    let graph = compiled.graph().clone();
    let mut first = projection::Projection::pending(1);
    let mut second = projection::Projection::pending(2);
    first.mount(&mut world, compiled.clone())?;
    second.mount(&mut world, compiled.clone())?;
    assert!(
        world
            .query::<&bevy::camera::RenderTarget>()
            .iter(&world)
            .all(|target| matches!(target, bevy::camera::RenderTarget::None { .. })),
        "Unmeasured/hidden preview cameras must never default to the serving window or its DPR"
    );
    assert_eq!(world.query::<&Mesh3d>().iter(&world).count(), 2);
    assert!(
        world
            .query::<&bevy::core_pipeline::tonemapping::Tonemapping>()
            .iter(&world)
            .all(|curve| *curve == bevy::core_pipeline::tonemapping::Tonemapping::Reinhard),
        "Default LUT tonemapping produces an error material in the minimal feature set"
    );
    let layers = world
        .query::<(&Mesh3d, &RenderLayers)>()
        .iter(&world)
        .map(|(_, layer)| layer.clone())
        .collect::<Vec<_>>();
    assert!(!layers[0].intersects(&layers[1]));
    assert_eq!(compiled.graph(), &graph);
    assert!(
        compiled
            .authoring_world()
            .iter_entities()
            .all(|entity| entity.get::<Mesh3d>().is_none())
    );
    let viewport = world.spawn_empty().id();
    first.update(&mut world, viewport, Vec2::ZERO, false)?;
    assert!(world.resource::<Assets<Image>>().is_empty());
    first.update(&mut world, viewport, Vec2::new(9000., 4500.), true)?;
    let target = &world
        .get::<ImageNode>(viewport)
        .ok_or("Target not displayed")?
        .image;
    let image = world
        .resource::<Assets<Image>>()
        .get(target)
        .ok_or("Image missing")?;
    assert_eq!((image.width(), image.height()), (2048, 1024));
    assert!(
        !first.frame_ready(&world),
        "CPU image allocation is not renderer readiness"
    );
    first.retire(&mut world);
    first.retire(&mut world);
    assert_eq!(world.query::<&Mesh3d>().iter(&world).count(), 1);
    assert!(world.resource::<Assets<Image>>().is_empty());
    second.retire(&mut world);
    assert!(world.resource::<Assets<Mesh>>().is_empty());
    assert!(world.resource::<Assets<StandardMaterial>>().is_empty());
    Ok(())
}
#[test]
fn unsupported_projection_is_rejected_instead_of_silently_dropping_nodes() -> TestResult {
    let mut world = World::new();
    let compiled = compiled(&mut world)?;
    let mut graph = compiled.graph().clone();
    projection::supported(&graph)?;
    let mut mixed = graph.clone();
    let key = mixed.root.clone();
    let component = mixed.nodes[&key].components["scene.primitive"].clone();
    mixed
        .nodes
        .get_mut(&key)
        .ok_or("Root missing")?
        .components
        .insert("ui.layout".into(), component);
    assert!(
        projection::supported(&mixed).is_err(),
        "A single entity cannot act as both native layout and spatial presentation"
    );
    let root = graph.root.clone();
    graph
        .nodes
        .get_mut(&root)
        .ok_or("Root missing")?
        .components
        .remove("scene.surface");
    assert!(projection::supported(&graph).is_err());
    Ok(())
}

#[test]
fn authored_stage_replaces_scene_light_only_for_its_view_and_retires_without_source_edits()
-> TestResult {
    use kedra_scene::environment::Environment;
    let temp = tempfile::tempdir()?;
    std::fs::create_dir(temp.path().join("environments"))?;
    std::fs::write(
        temp.path().join("environments/studio.environment.ron"),
        include_str!("../../../../assets/editor/environments/studio.environment.ron"),
    )?;
    let mut app = App::new();
    let mut reader = bevy::asset::io::AssetSource::get_default_reader(
        temp.path()
            .to_str()
            .ok_or("Unicode fixture path")?
            .to_owned(),
    );
    app.register_asset_source(
        "editor",
        bevy::asset::io::AssetSourceBuilder::new(move || reader()),
    );
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        kedra_scene::assets::SceneAssetPlugin,
    ));
    let source = compiled(app.world_mut())?;
    let original = source.graph().clone();
    app.finish();
    app.cleanup();
    let mut server = app.world().resource::<AssetServer>().clone();
    let environment: Handle<Environment> =
        server.load("editor://environments/studio.environment.ron");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !server.is_loaded_with_dependencies(environment.id()) {
        app.update();
        assert!(!matches!(
            server.get_load_state(environment.id()),
            Some(bevy::asset::LoadState::Failed(_))
        ));
        assert!(
            std::time::Instant::now() < deadline,
            "Environment fixture timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let world = app.world_mut();
    let stage = PrefabDocument::parse(include_str!(
        "../../../../assets/editor/scenes/model-preview.scene.ron"
    ))?;
    let id = stage.id;
    let mut catalog = Catalog::default();
    catalog.insert(stage)?;
    let registry = world.resource::<AppTypeRegistry>().clone();
    let mut policies = ComponentPolicies::default();
    let mut types = bevy::reflect::TypeRegistry::default();
    kedra_scene::spatial::register(&mut types, &mut policies)?;
    let stage = Arc::new(CompiledPrefab::compile(
        catalog.resolve(id)?,
        &registry.read(),
        &policies,
        &mut server,
        &BTreeMap::new(),
    )?);
    let mut first = projection::Projection::pending(1);
    let mut second = projection::Projection::pending(2);
    first.mount(world, source.clone())?;
    second.mount(world, source.clone())?;
    first.mount_stage(world, stage.clone(), false)?;
    second.mount_stage(world, stage, false)?;
    let a = first.camera().ok_or("Camera A missing")?;
    let b = second.camera().ok_or("Camera B missing")?;
    first.set_inspection(world, true)?;
    assert_eq!(
        world.get::<AmbientLight>(a).map(|v| v.brightness),
        Some(80.)
    );
    assert_eq!(world.get::<AmbientLight>(b).map(|v| v.brightness), Some(0.));
    for entity in first
        .nodes
        .values()
        .filter(|entity| world.get::<DirectionalLight>(**entity).is_some())
    {
        assert_eq!(world.get::<Visibility>(*entity), Some(&Visibility::Hidden));
    }
    for entity in second
        .nodes
        .values()
        .filter(|entity| world.get::<DirectionalLight>(**entity).is_some())
    {
        assert_eq!(
            world.get::<Visibility>(*entity),
            Some(&Visibility::Inherited)
        );
    }
    first.set_inspection(world, false)?;
    assert_eq!(world.get::<AmbientLight>(a).map(|v| v.brightness), Some(0.));
    assert_eq!(
        world.get::<Visibility>(first.inspection.ok_or("Stage missing")?),
        Some(&Visibility::Hidden)
    );
    assert_eq!(source.graph(), &original);
    first.retire(world);
    second.retire(world);
    assert_eq!(world.query::<&DirectionalLight>().iter(world).count(), 0);
    assert_eq!(
        world.resource::<Assets<Environment>>().len(),
        1,
        "Shared environment must survive view cleanup"
    );
    Ok(())
}
