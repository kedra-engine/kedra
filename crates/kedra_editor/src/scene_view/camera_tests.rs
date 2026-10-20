use super::camera::Command;
use super::camera::*;
use bevy::{
    camera::Projection, camera_controller::pan_orbit_camera::prelude::PanOrbitCamera, prelude::*,
};
type Result = std::result::Result<(), String>;
fn camera(world: &mut World) -> Entity {
    let camera = world
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(4., 3., 5.).looking_at(Vec3::ZERO, Vec3::Y),
        ))
        .id();
    attach(world, camera, Some((Vec3::splat(-1.), Vec3::ONE)));
    camera
}
#[test]
fn framing_and_all_standard_views_are_finite_and_do_not_change_source_bounds() -> Result {
    let mut world = World::new();
    let camera = camera(&mut world);
    let original = world.get::<Navigation>(camera).ok_or("navigation")?.bounds;
    for command in [
        Command::Front,
        Command::Back,
        Command::Left,
        Command::Right,
        Command::Top,
        Command::Bottom,
        Command::FrameAll,
        Command::Projection,
        Command::FrameAll,
        Command::Projection,
    ] {
        super::camera::command(&mut world, camera, command)?;
        assert!(
            world
                .get::<Transform>(camera)
                .ok_or("pose")?
                .to_matrix()
                .is_finite()
        );
        assert_eq!(
            world.get::<Navigation>(camera).ok_or("nav")?.bounds,
            original
        );
    }
    let before = *world.get::<Transform>(camera).ok_or("pose")?;
    assert!(super::camera::command(&mut world, camera, Command::FrameSelection).is_err());
    assert_eq!(*world.get::<Transform>(camera).ok_or("pose")?, before);
    frame(&mut world, camera, Some((Vec3::ONE, Vec3::ONE)))?;
    assert!(
        world
            .get::<Transform>(camera)
            .ok_or("pose")?
            .to_matrix()
            .is_finite()
    );
    assert!(frame(&mut world, camera, Some((Vec3::NAN, Vec3::ONE))).is_err());
    Ok(())
}
#[test]
fn flight_speed_is_bounded_diagonals_normalized_and_cancel_stops_native_motion() -> Result {
    let mut world = World::new();
    let camera = camera(&mut world);
    speed(&mut world, camera, f32::INFINITY);
    for _ in 0..100 {
        speed(&mut world, camera, 100.);
    }
    assert_eq!(
        world.get::<Navigation>(camera).ok_or("navigation")?.speed,
        100_000.
    );
    for _ in 0..100 {
        speed(&mut world, camera, -100.);
    }
    assert_eq!(
        world.get::<Navigation>(camera).ok_or("navigation")?.speed,
        0.001
    );
    start(&mut world, camera, false);
    assert!(
        world
            .get::<PanOrbitCamera>(camera)
            .ok_or("controller")?
            .is_actively_controlled()
    );
    cancel(&mut world, camera);
    assert!(
        !world
            .get::<PanOrbitCamera>(camera)
            .ok_or("controller")?
            .is_actively_controlled()
    );
    fly(&mut world, camera, Vec2::splat(1e6), Vec3::ONE, false, 10.);
    assert!(
        world
            .get::<Transform>(camera)
            .ok_or("pose")?
            .to_matrix()
            .is_finite()
    );
    assert!(matches!(
        world.get::<Projection>(camera),
        Some(Projection::Perspective(_))
    ));
    Ok(())
}
