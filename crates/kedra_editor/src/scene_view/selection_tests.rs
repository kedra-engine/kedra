use super::*;
use bevy::{
    camera::{
        primitives::Aabb,
        visibility::{InheritedVisibility, RenderLayers, ViewVisibility},
    },
    gizmos::GizmoAsset,
};
use kedra_scene::document::{InstanceId, NodeAddress, NodeId};
type Result = std::result::Result<(), String>;

#[test]
fn selected_model_publishes_marker_when_native_geometry_arrives() -> Result {
    let mut world = World::new();
    world.init_resource::<Assets<Mesh>>();
    world.init_resource::<Assets<GizmoAsset>>();
    let root = world.spawn(Transform::from_xyz(4., 0., 0.)).id();
    let address: NodeAddress = NodeId::new().into();
    let mut projection = projection::Projection::pending(1);
    projection.nodes.insert(address.clone(), root);
    let mut marker = selection::Marker::default();
    marker.update(&mut world, &projection, Some(&address))?;
    assert!(world.resource::<Assets<GizmoAsset>>().is_empty());
    let mesh = world.resource_mut::<Assets<Mesh>>().add(Cuboid::default());
    world.spawn((Mesh3d(mesh), Transform::IDENTITY, ChildOf(root)));
    marker.update(&mut world, &projection, Some(&address))?;
    assert_eq!(world.resource::<Assets<GizmoAsset>>().len(), 1);
    marker.update(&mut world, &projection, None)?;
    assert!(world.resource::<Assets<GizmoAsset>>().is_empty());
    marker.update(&mut world, &projection, Some(&address))?;
    marker.clear(&mut world);
    assert!(world.resource::<Assets<GizmoAsset>>().is_empty());
    assert!(world.entities().contains(root));
    Ok(())
}

#[test]
fn failed_bounds_measurement_does_not_cache_unchanged_selection() -> Result {
    let mut world = World::new();
    world.init_resource::<Assets<Mesh>>();
    world.init_resource::<Assets<GizmoAsset>>();
    let mesh = world.resource_mut::<Assets<Mesh>>().add(Cuboid::default());
    let root = world
        .spawn((
            Mesh3d(mesh),
            Transform::from_translation(Vec3::splat(f32::NAN)),
        ))
        .id();
    let address: NodeAddress = NodeId::new().into();
    let mut projection = projection::Projection::pending(1);
    projection.nodes.insert(address.clone(), root);
    let mut marker = selection::Marker::default();
    assert!(
        marker
            .update(&mut world, &projection, Some(&address))
            .is_err()
    );
    assert!(world.resource::<Assets<GizmoAsset>>().is_empty());
    world.entity_mut(root).insert(Transform::IDENTITY);
    marker.update(&mut world, &projection, Some(&address))?;
    assert_eq!(world.resource::<Assets<GizmoAsset>>().len(), 1);
    marker.clear(&mut world);
    marker.clear(&mut world);
    assert!(world.resource::<Assets<GizmoAsset>>().is_empty());
    Ok(())
}

#[test]
fn native_triangle_hits_map_nested_parts_and_do_not_hit_other_views_or_helpers() -> Result {
    bevy::tasks::ComputeTaskPool::get_or_init(Default::default);
    let mut world = World::new();
    world.init_resource::<Assets<Mesh>>();
    world.init_resource::<Assets<StandardMaterial>>();
    world.init_resource::<Assets<GizmoAsset>>();
    let mesh = world.resource_mut::<Assets<Mesh>>().add(Cuboid::default());
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    let root = world.spawn(Transform::IDENTITY).id();
    let visible = ViewVisibility::VISIBLE;
    let native = world
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            GlobalTransform::IDENTITY,
            Transform::IDENTITY,
            Aabb::from_min_max(Vec3::splat(-0.5), Vec3::splat(0.5)),
            InheritedVisibility::VISIBLE,
            visible,
            RenderLayers::layer(1),
            ChildOf(root),
        ))
        .id();
    world.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material.clone()),
        GlobalTransform::from_translation(Vec3::Z * 2.),
        Aabb::from_min_max(Vec3::splat(-0.5), Vec3::splat(0.5)),
        InheritedVisibility::VISIBLE,
        visible,
        RenderLayers::layer(2),
    ));
    let address = NodeAddress {
        instances: vec![InstanceId::new()],
        node: NodeId::new(),
    };
    let mut projection = projection::Projection::pending(1);
    projection.nodes.insert(address.clone(), root);
    let ray = Ray3d::new(Vec3::Z * 5., Dir3::NEG_Z);
    assert_eq!(
        selection::cast(&mut world, &projection, ray)?,
        Some(address.clone())
    );
    let mut marker = selection::Marker::default();
    marker.update(&mut world, &projection, Some(&address))?;
    assert_eq!(world.resource::<Assets<GizmoAsset>>().len(), 1);
    assert_eq!(
        world
            .get::<MeshMaterial3d<StandardMaterial>>(native)
            .ok_or("material")?
            .0,
        material
    );
    marker.update(&mut world, &projection, None)?;
    marker.clear(&mut world);
    assert_eq!(world.resource::<Assets<GizmoAsset>>().len(), 0);
    world.entity_mut(native).insert(InheritedVisibility::HIDDEN);
    assert_eq!(selection::cast(&mut world, &projection, ray)?, None);
    Ok(())
}
