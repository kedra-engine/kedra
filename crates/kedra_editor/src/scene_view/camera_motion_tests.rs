use super::{camera, camera_motion, orientation::Axis};
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use kedra_ui::authored::components::UiMotion;
use std::time::Duration;

fn fixture() -> (World, Entity, Entity) {
    let mut world = World::new();
    world.init_resource::<Time<Real>>();
    let window = world
        .spawn(Window {
            focused: true,
            ..default()
        })
        .id();
    let camera = world
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(4., 3., 5.).looking_at(Vec3::ZERO, Vec3::Y),
        ))
        .id();
    camera::attach(&mut world, camera, None);
    (world, camera, window)
}
fn step(world: &mut World, seconds: f32) -> Result<(), Box<dyn std::error::Error>> {
    world
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_secs_f32(seconds));
    world
        .run_system_once(camera_motion::advance)
        .map_err(|error| error.to_string())?;
    Ok(())
}
fn pose(world: &World, camera: Entity) -> Result<Transform, String> {
    world
        .get::<Transform>(camera)
        .copied()
        .ok_or("Camera missing".into())
}
fn align(world: &mut World, camera: Entity, window: Entity, axis: Axis) -> Result<(), String> {
    let target = camera_motion::target(world, camera, axis)?;
    // Production pointer acquisition and release both interrupt controller motion.
    camera::cancel(world, camera);
    camera_motion::request(world, camera, window, target)
}

#[test]
fn alignment_moves_through_intermediate_poses_on_a_constant_radius_then_stops()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut world, camera, window) = fixture();
    let before = pose(&world, camera)?;
    let radius = before.translation.length();
    align(&mut world, camera, window, Axis::Z)?;
    assert_eq!(pose(&world, camera)?, before, "Request must not teleport");
    step(&mut world, 0.05)?;
    let during = pose(&world, camera)?;
    assert_ne!(during.rotation, before.rotation);
    assert!(
        during.translation.x.abs() > 0.01,
        "The first frame must not already be the final view"
    );
    assert!((during.translation.length() - radius).abs() < 0.001);
    step(&mut world, 0.2)?;
    let after = pose(&world, camera)?;
    assert!(after.translation.distance(Vec3::Z * radius) < 0.001);
    step(&mut world, 1.)?;
    assert_eq!(pose(&world, camera)?, after);
    Ok(())
}

#[test]
fn repeated_axis_click_retargets_from_current_pose_and_flips_the_requested_side()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut world, camera, window) = fixture();
    let radius = pose(&world, camera)?.translation.length();
    align(&mut world, camera, window, Axis::X)?;
    step(&mut world, 0.08)?;
    let current = pose(&world, camera)?;
    align(&mut world, camera, window, Axis::X)?;
    assert_eq!(pose(&world, camera)?, current);
    step(&mut world, 0.2)?;
    assert!(
        pose(&world, camera)?
            .translation
            .distance(Vec3::NEG_X * radius)
            < 0.001
    );
    Ok(())
}

#[test]
fn reduced_motion_and_direct_camera_commands_do_not_leave_a_delayed_alignment()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut world, camera, window) = fixture();
    let radius = pose(&world, camera)?.translation.length();
    world.insert_resource(UiMotion { reduced: true });
    align(&mut world, camera, window, Axis::Z)?;
    assert!(pose(&world, camera)?.translation.distance(Vec3::Z * radius) < 0.001);
    world.resource_mut::<UiMotion>().reduced = false;
    align(&mut world, camera, window, Axis::X)?;
    step(&mut world, 0.05)?;
    camera::command(&mut world, camera, camera::Command::Top)?;
    let direct = pose(&world, camera)?;
    step(&mut world, 0.2)?;
    assert_eq!(pose(&world, camera)?, direct);
    Ok(())
}

#[test]
fn cancellation_focus_loss_and_inactive_targets_freeze_the_current_pose()
-> Result<(), Box<dyn std::error::Error>> {
    for reason in 0..3 {
        let (mut world, camera, window) = fixture();
        align(&mut world, camera, window, Axis::X)?;
        step(&mut world, 0.05)?;
        let stopped = pose(&world, camera)?;
        match reason {
            0 => camera::cancel(&mut world, camera),
            1 => {
                world
                    .get_mut::<Window>(window)
                    .ok_or("Window missing")?
                    .focused = false
            }
            _ => {
                world
                    .get_mut::<Camera>(camera)
                    .ok_or("Camera missing")?
                    .is_active = false
            }
        }
        step(&mut world, 0.2)?;
        assert_eq!(pose(&world, camera)?, stopped);
    }
    Ok(())
}

#[test]
fn invalid_alignment_requests_do_not_mutate_the_camera() -> Result<(), Box<dyn std::error::Error>> {
    for reason in 0..4 {
        let (mut world, camera, window) = fixture();
        match reason {
            0 => {
                world
                    .get_mut::<Window>(window)
                    .ok_or("Window missing")?
                    .focused = false
            }
            1 => {
                world
                    .get_mut::<Camera>(camera)
                    .ok_or("Camera missing")?
                    .is_active = false
            }
            2 => {
                world
                    .get_mut::<Transform>(camera)
                    .ok_or("Pose missing")?
                    .rotation = Quat::from_xyzw(0., 0., 0., 0.)
            }
            _ => {
                world.entity_mut(camera).remove::<camera::Navigation>();
            }
        }
        let before = pose(&world, camera)?;
        assert!(align(&mut world, camera, window, Axis::X).is_err());
        assert_eq!(pose(&world, camera)?, before);
        assert!(!camera_motion::active(&world, camera));
    }
    Ok(())
}

#[test]
fn enabling_reduced_motion_finishes_an_in_progress_turn() -> Result<(), Box<dyn std::error::Error>>
{
    let (mut world, camera, window) = fixture();
    let radius = pose(&world, camera)?.translation.length();
    align(&mut world, camera, window, Axis::Z)?;
    step(&mut world, 0.05)?;
    world.insert_resource(UiMotion { reduced: true });
    step(&mut world, 0.)?;
    assert!(pose(&world, camera)?.translation.distance(Vec3::Z * radius) < 0.001);
    assert!(!camera_motion::active(&world, camera));
    Ok(())
}
