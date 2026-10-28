use bevy::{camera::primitives::Aabb, prelude::*};
use kedra_preview::bounds;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn mesh_bounds_fallback_is_measurement_local_and_rejects_invalid_native_bounds() -> TestResult {
    let mut world = World::new();
    world.init_resource::<Assets<Mesh>>();
    let mesh = world
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::from_length(2.));
    let owner = world.spawn(Transform::IDENTITY).id();
    let first = world
        .spawn((
            Mesh3d(mesh.clone()),
            Transform::from_xyz(-3., 0., 0.),
            ChildOf(owner),
        ))
        .id();
    world.spawn((
        Mesh3d(mesh.clone()),
        Transform::from_xyz(3., 0., 0.).with_scale(Vec3::new(2., 1., 1.)),
        ChildOf(owner),
    ));
    assert_eq!(
        bounds::measure(&world, owner)?,
        Some((Vec3::new(-4., -1., -1.), Vec3::new(5., 1., 1.)))
    );
    world
        .resource_mut::<Assets<Mesh>>()
        .insert(mesh.id(), Mesh::from(Cuboid::from_length(4.)))?;
    assert_eq!(
        bounds::measure(&world, owner)?,
        Some((Vec3::new(-5., -2., -2.), Vec3::new(7., 2., 2.)))
    );
    for native in [
        Aabb {
            center: Vec3::splat(f32::NAN).into(),
            half_extents: Vec3::ONE.into(),
        },
        Aabb::from_min_max(Vec3::splat(-2_000_000.), Vec3::splat(2_000_000.)),
    ] {
        world.entity_mut(first).insert(native);
        assert!(
            bounds::measure(&world, owner).is_err(),
            "Invalid native bounds must not fall back to the valid mesh"
        );
    }
    world.entity_mut(first).remove::<Aabb>();
    world.resource_mut::<Assets<Mesh>>().remove(mesh.id());
    assert!(bounds::measure(&world, owner).is_err());
    assert!(
        world.get::<Aabb>(first).is_none(),
        "Measurement must not write cached bounds into the world"
    );
    Ok(())
}

#[test]
fn native_bounds_do_not_bypass_presentation_node_budget() {
    let mut world = World::new();
    let root = world.spawn_empty().id();
    for _ in 0..4096 {
        world.spawn(ChildOf(root));
    }
    assert!(bounds::measure(&world, root).is_err_and(|error| error.contains("4096-node budget")));
}
