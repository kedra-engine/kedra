use super::orientation::Axis;
use super::orientation::*;
use bevy::prelude::*;

#[test]
fn front_projects_world_x_right_y_up_and_front_z_above_back_z() -> Result<(), String> {
    let endpoints = project(Quat::IDENTITY).ok_or("Finite camera")?;
    let at = |axis| {
        endpoints
            .iter()
            .find(|e| e.axis == axis)
            .ok_or("Signed axis")
    };
    assert_eq!(at(Axis::X)?.center, Vec2::new(82., 48.));
    assert_eq!(at(Axis::Y)?.center, Vec2::new(48., 14.));
    assert_eq!(at(Axis::Z)?.center, Vec2::splat(48.));
    assert_eq!(
        endpoints.last().map(|endpoint| endpoint.axis),
        Some(Axis::Z)
    );
    Ok(())
}

#[test]
fn top_is_y_up_with_negative_z_at_screen_top_and_invalid_geometry_cannot_hit() -> Result<(), String>
{
    let rotation = Transform::from_translation(Vec3::Y)
        .looking_at(Vec3::ZERO, Vec3::NEG_Z)
        .rotation;
    let endpoints = project(rotation).ok_or("Top camera")?;
    let negative_z = endpoints
        .iter()
        .find(|e| e.axis == Axis::NegZ)
        .ok_or("Negative Z")?;
    assert!(negative_z.center.distance(Vec2::new(48., 14.)) < 0.001);
    assert_eq!(
        endpoints.last().map(|endpoint| endpoint.axis),
        Some(Axis::Y)
    );
    assert!(project(Quat::from_xyzw(f32::NAN, 0., 0., 1.)).is_none());
    Ok(())
}

#[test]
fn repeated_axis_alignment_flips_without_changing_pivot_or_distance() -> Result<(), String> {
    let mut world = World::new();
    let camera = world
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(4., 3., 5.).looking_at(Vec3::ZERO, Vec3::Y),
        ))
        .id();
    super::camera::attach(&mut world, camera, None);
    let distance = world
        .get::<Transform>(camera)
        .ok_or("Camera")?
        .translation
        .length();
    align(&mut world, camera, Axis::Z)?;
    assert!(
        world
            .get::<Transform>(camera)
            .ok_or("Camera")?
            .translation
            .distance(Vec3::Z * distance)
            < 0.001
    );
    align(&mut world, camera, Axis::Z)?;
    assert!(
        world
            .get::<Transform>(camera)
            .ok_or("Camera")?
            .translation
            .distance(Vec3::NEG_Z * distance)
            < 0.001
    );
    assert_eq!(
        world
            .get::<super::camera::Navigation>(camera)
            .ok_or("Navigation")?
            .pivot,
        Vec3::ZERO
    );
    Ok(())
}
