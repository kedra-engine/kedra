use bevy::{camera::primitives::Aabb, prelude::*, world_serialization::WorldAsset};
use kedra_preview::{bounds, models};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn imported_native_bounds_survive_projection_for_shared_transformed_meshes() -> TestResult {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::transform::TransformPlugin))
        .register_type::<ChildOf>()
        .register_type::<Children>()
        .register_type::<Mesh3d>()
        .register_type::<MeshMaterial3d<StandardMaterial>>()
        .register_type::<Aabb>()
        .init_resource::<Assets<WorldAsset>>();
    // No mesh asset is materialized: native bounds must be sufficient.
    let mesh = Handle::<Mesh>::default();
    let native = Aabb::from_min_max(Vec3::splat(-1.), Vec3::splat(1.));
    let mut source = World::new();
    for transform in [
        Transform::from_xyz(-3., 0., 0.),
        Transform::from_xyz(3., 0., 0.).with_scale(Vec3::new(2., 1., 1.)),
    ] {
        source.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d::<StandardMaterial>::default(),
            native,
            transform,
        ));
    }
    let model = app
        .world_mut()
        .resource_mut::<Assets<WorldAsset>>()
        .add(WorldAsset::new(source));
    let owner = app.world_mut().spawn(Transform::from_xyz(0., 2., 0.)).id();
    assert_eq!(
        models::mount(app.world_mut(), owner, &model, 1, 4096)?.len(),
        2
    );
    let children = app
        .world()
        .get::<Children>(owner)
        .ok_or("Model roots missing")?;
    for child in children.iter() {
        let bounds = app
            .world()
            .get::<Aabb>(child)
            .ok_or("Native bounds were discarded")?;
        assert_eq!(bounds.center, native.center);
        assert_eq!(bounds.half_extents, native.half_extents);
    }
    assert_eq!(
        bounds::measure(app.world(), owner)?,
        Some((Vec3::new(-4., 1., -1.), Vec3::new(5., 3., 1.)))
    );
    let source = &app
        .world()
        .resource::<Assets<WorldAsset>>()
        .get(&model)
        .ok_or("Source missing")?
        .world;
    let primitives = source
        .iter_entities()
        .filter(|entity| entity.contains::<Mesh3d>())
        .collect::<Vec<_>>();
    assert_eq!(primitives.len(), 2);
    for entity in primitives {
        let bounds = entity.get::<Aabb>().ok_or("Source bounds missing")?;
        assert_eq!(bounds.center, native.center);
        assert_eq!(bounds.half_extents, native.half_extents);
        assert!(entity.get::<ChildOf>().is_none());
    }
    app.world_mut()
        .entity_mut(owner)
        .insert(Transform::from_xyz(f32::NAN, 0., 0.));
    assert!(bounds::measure(app.world(), owner).is_err());
    Ok(())
}

#[test]
fn imported_model_relationships_are_remapped_before_attaching_owned_roots() -> TestResult {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::transform::TransformPlugin))
        .register_type::<Name>()
        .register_type::<ChildOf>()
        .register_type::<Children>()
        .init_resource::<Assets<WorldAsset>>();
    let mut source = World::new();
    let root = source
        .spawn((Name::new("model root"), Transform::IDENTITY))
        .id();
    let child = source
        .spawn((
            Name::new("model child"),
            Transform::from_xyz(1., 0., 0.),
            ChildOf(root),
        ))
        .id();
    source.spawn((
        Name::new("model grandchild"),
        Transform::from_xyz(0., 2., 0.),
        ChildOf(child),
    ));
    let model = app
        .world_mut()
        .resource_mut::<Assets<WorldAsset>>()
        .add(WorldAsset::new(source));
    let owner = app.world_mut().spawn(Transform::IDENTITY).id();
    assert_eq!(
        models::mount(app.world_mut(), owner, &model, 1, 4096)?.len(),
        3
    );
    assert_eq!(
        app.world()
            .get::<Children>(owner)
            .ok_or("Owner children missing")?
            .len(),
        1,
        "Only model roots may attach to the presentation owner"
    );
    for entity in app.world().iter_entities() {
        if let Some(children) = entity.get::<Children>() {
            for child in children.iter() {
                assert_eq!(
                    app.world()
                        .get::<ChildOf>(child)
                        .ok_or("Back-link missing")?
                        .parent(),
                    entity.id()
                );
            }
        }
    }
    app.update();
    assert!(
        models::mount(app.world_mut(), owner, &model, 2, 2).is_err(),
        "Aggregate node budget must reject before allocating"
    );
    assert_eq!(
        app.world()
            .get::<Children>(owner)
            .ok_or("Owner children missing")?
            .len(),
        1
    );
    Ok(())
}
