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
fn wheel_notch_changes_native_zoom_scale_and_invalid_input_preserves_pose() -> Result {
    use bevy::camera::{CameraProjection, ComputedCameraValues, RenderTargetInfo};
    use bevy::ecs::system::RunSystemOnce;
    let mut world = World::new();
    world.init_resource::<Time<Real>>();
    world.init_resource::<Messages<bevy::window::RequestRedraw>>();
    let camera = camera(&mut world);
    let mut projection = OrthographicProjection {
        scale: 1.,
        ..OrthographicProjection::default_3d()
    };
    projection.update(800., 600.);
    world.entity_mut(camera).insert((
        Camera {
            computed: ComputedCameraValues {
                clip_from_view: projection.get_clip_from_view(),
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::new(800, 600),
                    scale_factor: 1.,
                }),
                ..default()
            },
            ..default()
        },
        Projection::Orthographic(projection),
    ));
    let before = *world.get::<Transform>(camera).ok_or("pose")?;
    zoom(&mut world, camera, f32::NAN);
    assert_eq!(*world.get::<Transform>(camera).ok_or("pose")?, before);
    zoom(&mut world, camera, 1.);
    world
        .run_system_once(PanOrbitCamera::update_camera_positions)
        .map_err(|error| error.to_string())?;
    let Some(Projection::Orthographic(projection)) = world.get::<Projection>(camera) else {
        return Err("projection missing".into());
    };
    assert!(projection.scale < 0.96 && projection.scale > 0.7);
    cancel(&mut world, camera);
    let stopped = projection_scale(&world, camera)?;
    world
        .run_system_once(PanOrbitCamera::update_camera_positions)
        .map_err(|error| error.to_string())?;
    assert_eq!(projection_scale(&world, camera)?, stopped);
    Ok(())
}

fn projection_scale(world: &World, camera: Entity) -> std::result::Result<f32, String> {
    match world.get::<Projection>(camera) {
        Some(Projection::Orthographic(projection)) => Ok(projection.scale),
        _ => Err("projection missing".into()),
    }
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
