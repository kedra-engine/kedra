//! Owned asset refresh and failure atomicity; native rendering is a separate scenario.
use super::*;
use kedra_scene::{document::ComponentData, reflection::ComponentPolicies};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn candidate(
    world: &mut World,
    previous: &CompiledPrefab,
) -> Result<CompiledPrefab, Box<dyn std::error::Error>> {
    let registry = world.resource::<AppTypeRegistry>().clone();
    let mut policies = ComponentPolicies::default();
    kedra_scene::spatial::register(&mut registry.write(), &mut policies)?;
    let mut graph = previous.graph().clone();
    let root = graph.root.clone();
    for (key, text) in [
        ("scene.primitive", "Cuboid(size:(4.0,2.0,2.0))"),
        (
            "scene.surface",
            "(color:Srgba((red:1.0,green:0.0,blue:0.0,alpha:1.0)),texture:None,metallic:0.25,roughness:0.75)",
        ),
    ] {
        graph
            .nodes
            .get_mut(&root)
            .ok_or("root")?
            .components
            .get_mut(key)
            .ok_or("component")?
            .data = ComponentData::ron(1, text)?;
    }
    Ok(previous.recompile(graph, &registry.read(), &policies)?)
}

#[test]
fn spatial_value_refresh_keeps_entities_camera_and_asset_handles_and_updates_mesh_bounds()
-> TestResult {
    let mut world = World::new();
    let previous = crate::scene_view::test_support::compiled(&mut world)?;
    let mut projection = Projection::pending(1);
    projection.mount(&mut world, previous.clone())?;
    let entity = projection.nodes[&previous.graph().root];
    let camera = projection.camera().ok_or("camera")?;
    let camera_before = *world.get::<Transform>(camera).ok_or("camera Transform")?;
    let mesh = world.get::<Mesh3d>(entity).ok_or("mesh")?.0.clone();
    let material = world
        .get::<MeshMaterial3d<StandardMaterial>>(entity)
        .ok_or("material")?
        .0
        .clone();
    let next = candidate(&mut world, &previous)?;
    let epoch = projection.epoch;
    projection.refresh_spatial_values(&mut world, &next, previous.graph())?;
    assert_eq!(projection.camera(), Some(camera));
    assert_eq!(world.get::<Transform>(camera), Some(&camera_before));
    assert_eq!(world.get::<Mesh3d>(entity).map(|mesh| &mesh.0), Some(&mesh));
    assert_eq!(
        world
            .get::<MeshMaterial3d<StandardMaterial>>(entity)
            .map(|value| &value.0),
        Some(&material)
    );
    let value = world
        .resource::<Assets<StandardMaterial>>()
        .get(&material)
        .ok_or("material")?;
    assert_eq!(value.base_color, Color::srgb(1., 0., 0.));
    assert_eq!(value.metallic, 0.25);
    assert_eq!(value.perceptual_roughness, 0.75);
    let bounds = world
        .get::<bevy::camera::primitives::Aabb>(entity)
        .ok_or("bounds")?;
    assert_eq!(Vec3::from(bounds.half_extents), Vec3::new(2., 1., 1.));
    assert!(projection.epoch > epoch);
    Ok(())
}

#[test]
fn missing_owned_material_refuses_before_changing_mesh_bounds_or_frame_epoch() -> TestResult {
    let mut world = World::new();
    let previous = crate::scene_view::test_support::compiled(&mut world)?;
    let mut projection = Projection::pending(1);
    projection.mount(&mut world, previous.clone())?;
    let entity = projection.nodes[&previous.graph().root];
    let material = world
        .get::<MeshMaterial3d<StandardMaterial>>(entity)
        .ok_or("material")?
        .0
        .clone();
    let mesh = world.get::<Mesh3d>(entity).ok_or("mesh")?.0.clone();
    let before = world
        .resource::<Assets<Mesh>>()
        .get(&mesh)
        .ok_or("mesh")?
        .get_aabb()
        .ok_or("bounds")?;
    let next = candidate(&mut world, &previous)?;
    world
        .resource_mut::<Assets<StandardMaterial>>()
        .remove(material.id());
    let epoch = projection.epoch;
    assert!(
        projection
            .refresh_spatial_values(&mut world, &next, previous.graph())
            .is_err()
    );
    assert_eq!(projection.epoch, epoch);
    let after = world
        .resource::<Assets<Mesh>>()
        .get(&mesh)
        .ok_or("mesh")?
        .get_aabb()
        .ok_or("bounds")?;
    assert_eq!(before.half_extents, after.half_extents);
    assert_eq!(world.get::<Mesh3d>(entity).map(|mesh| &mesh.0), Some(&mesh));
    assert!(
        matches!(world.get::<Primitive>(entity), Some(Primitive::Cuboid { size }) if *size == Vec3::splat(2.))
    );
    Ok(())
}

#[test]
fn environment_refresh_retains_cameras_isolates_views_and_preserves_studio_lighting() -> TestResult
{
    use bevy::asset::{AssetApp, AssetMetaCheck, AssetPlugin};
    use kedra_scene::{
        composition::Catalog,
        document::{DocumentKind, PrefabDocument},
        environment::{Environment, SceneEnvironment},
    };
    let directory = tempfile::tempdir()?;
    let day = Environment {
        version: 1,
        background: Color::WHITE,
        ambient_color: Color::WHITE,
        ambient_brightness: 100.,
    };
    let night = Environment {
        background: Color::BLACK,
        ambient_brightness: 5.,
        ..day.clone()
    };
    for (name, value) in [("day", &day), ("night", &night)] {
        std::fs::write(
            directory.path().join(format!("{name}.environment.ron")),
            ron::to_string(value)?,
        )?;
    }
    let mut app = App::new();
    let root = directory.path().to_str().ok_or("fixture path")?.to_owned();
    app.register_asset_source(
        "game",
        bevy::asset::io::AssetSourceBuilder::new(move || {
            Box::new(bevy::asset::io::file::FileAssetReader::new(root.clone()))
        }),
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            meta_check: AssetMetaCheck::Never,
            ..default()
        },
        kedra_scene::assets::SceneAssetPlugin,
    ));
    crate::scene_view::test_support::compiled(app.world_mut())?;
    app.finish();
    app.cleanup();
    let mut loader = app.world().resource::<AssetServer>().clone();
    let day_handle = loader.load::<Environment>("game://day.environment.ron");
    let night_handle = loader.load::<Environment>("game://night.environment.ron");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !loader.is_loaded_with_dependencies(day_handle.id())
        || !loader.is_loaded_with_dependencies(night_handle.id())
    {
        assert!(
            std::time::Instant::now() < deadline,
            "Environment loader deadline"
        );
        app.update();
        std::thread::yield_now();
    }
    let world = app.world_mut();
    let registry = world.resource::<AppTypeRegistry>().clone();
    let mut policies = ComponentPolicies::default();
    kedra_scene::spatial::register(&mut registry.write(), &mut policies)?;
    let mut document = PrefabDocument::new("Environment source");
    document.kind = DocumentKind::Scene;
    document.nodes[0].components.insert(
        "scene.environment".into(),
        ComponentData::ron(1, "(asset:Path(\"game://day.environment.ron\"))")?,
    );
    let compile = |document: &PrefabDocument,
                   loader: &mut AssetServer|
     -> Result<Arc<CompiledPrefab>, Box<dyn std::error::Error>> {
        let mut catalog = Catalog::default();
        catalog.insert(document.clone())?;
        Ok(Arc::new(CompiledPrefab::compile(
            catalog.resolve(document.id)?,
            &registry.read(),
            &policies,
            loader,
            &BTreeMap::new(),
        )?))
    };
    let before = compile(&document, &mut loader)?;
    let mut first = Projection::pending(1);
    let mut second = Projection::pending(2);
    first.mount(world, before.clone())?;
    second.mount(world, before.clone())?;
    let camera = first.camera().ok_or("camera")?;
    let foreign = second.camera().ok_or("foreign camera")?;
    let pose = *world.get::<Transform>(camera).ok_or("pose")?;
    let entity = first.nodes[&before.graph().root];
    document.nodes[0].components.insert(
        "scene.environment".into(),
        ComponentData::ron(1, "(asset:Path(\"game://night.environment.ron\"))")?,
    );
    let next = compile(&document, &mut loader)?;
    let epoch = first.epoch;
    world
        .resource_mut::<Assets<Environment>>()
        .remove(night_handle.id());
    assert!(
        first
            .refresh_spatial_values(world, &next, before.graph())
            .is_err()
    );
    assert_eq!(first.epoch, epoch);
    assert_eq!(
        world
            .get::<AmbientLight>(camera)
            .map(|light| light.brightness),
        Some(100.)
    );
    assert_eq!(
        world
            .get::<SceneEnvironment>(entity)
            .map(|reference| &reference.asset),
        Some(&day_handle)
    );
    world
        .resource_mut::<Assets<Environment>>()
        .insert(night_handle.id(), night)?;
    first.refresh_spatial_values(world, &next, before.graph())?;
    assert_eq!(first.camera(), Some(camera));
    assert_eq!(world.get::<Transform>(camera), Some(&pose));
    assert_eq!(
        world
            .get::<AmbientLight>(camera)
            .map(|light| light.brightness),
        Some(5.)
    );
    assert_eq!(
        world
            .get::<AmbientLight>(foreign)
            .map(|light| light.brightness),
        Some(100.)
    );
    assert_eq!(
        world
            .get::<SceneEnvironment>(entity)
            .map(|reference| &reference.asset),
        Some(&night_handle)
    );
    assert!(first.epoch > epoch);
    // Simulate the existing stage visibility contract without a second scene loader.
    first.inspection = Some(world.spawn(Visibility::Inherited).id());
    first.studio_environment = Some(Environment {
        version: 1,
        background: Color::WHITE,
        ambient_color: Color::WHITE,
        ambient_brightness: 80.,
    });
    first.set_inspection(world, true)?;
    first.refresh_spatial_values(world, &before, next.graph())?;
    assert_eq!(
        world
            .get::<AmbientLight>(camera)
            .map(|light| light.brightness),
        Some(80.)
    );
    first.set_inspection(world, false)?;
    assert_eq!(
        world
            .get::<AmbientLight>(camera)
            .map(|light| light.brightness),
        Some(100.)
    );
    assert_eq!(
        world
            .get::<AmbientLight>(foreign)
            .map(|light| light.brightness),
        Some(100.)
    );
    assert_eq!(world.get::<Transform>(camera), Some(&pose));
    Ok(())
}
