//! Parent-space correctness and rejected-gesture immutability.
use bevy::prelude::*;
use kedra_gizmo::{MoveDrag, RotateDrag, ScaleDrag};

fn ray(x: f32) -> Ray3d {
    Ray3d::new(Vec3::new(x, 0., 10.), Dir3::NEG_Z)
}

#[test]
fn world_move_under_rotated_nonuniform_parent_preserves_rotation_scale_and_baseline()
-> Result<(), &'static str> {
    let parent = Mat4::from_scale_rotation_translation(
        Vec3::new(2., 3., -4.),
        Quat::from_rotation_z(0.7),
        Vec3::ZERO,
    );
    let start =
        Transform::from_rotation(Quat::from_rotation_y(0.4)).with_scale(Vec3::new(-2., 1., 0.5));
    let drag = MoveDrag::begin(start, parent, Vec3::X, ray(0.), Some(0.5))?;
    let moved = drag.update(ray(1.3))?;
    assert!(
        parent
            .transform_point3(moved.translation)
            .abs_diff_eq(Vec3::new(1.5, 0., 0.), 1e-5)
    );
    assert_eq!(moved.rotation, start.rotation);
    assert_eq!(moved.scale, start.scale);
    assert_eq!(drag.update(ray(0.))?, start);
    assert!(drag.update(Ray3d::new(Vec3::ZERO, Dir3::X)).is_err());
    assert_eq!(drag.update(ray(1.3))?, moved);
    Ok(())
}

#[test]
fn invalid_parent_axis_and_snap_never_acquire_a_gesture() {
    let baseline = Transform::IDENTITY;
    for parent in [Mat4::ZERO, Mat4::from_scale(Vec3::new(1., 0., 1.))] {
        assert!(MoveDrag::begin(baseline, parent, Vec3::X, ray(0.), None).is_err());
    }
    for axis in [Vec3::ZERO, Vec3::Z] {
        assert!(MoveDrag::begin(baseline, Mat4::IDENTITY, axis, ray(0.), None).is_err());
    }
    for step in [0., -1., f32::NAN] {
        assert!(MoveDrag::begin(baseline, Mat4::IDENTITY, Vec3::X, ray(0.), Some(step)).is_err());
    }
}

#[test]
fn local_rotation_under_reflected_nonuniform_parent_preserves_pose_and_recovers_from_invalid_rays()
-> Result<(), &'static str> {
    let parent = Mat4::from_scale_rotation_translation(
        Vec3::new(-2., 3., 4.),
        Quat::from_rotation_y(0.7),
        Vec3::new(1., 2., 3.),
    );
    let baseline = Transform::from_xyz(1., 2., 0.)
        .with_rotation(Quat::from_rotation_x(0.3))
        .with_scale(Vec3::new(-1., 2., 0.5));
    let frame = parent * Mat4::from_rotation_translation(baseline.rotation, baseline.translation);
    let ray = |p: Vec3, d: Vec3| -> Result<Ray3d, &'static str> {
        Ok(Ray3d::new(
            frame.transform_point3(p),
            Dir3::new(frame.transform_vector3(d)).map_err(|_| "Ray")?,
        ))
    };
    let press = ray(Vec3::new(1., 0., 10.), Vec3::NEG_Z)?;
    let moved = ray(Vec3::new(0.6_f32.cos(), 0.6_f32.sin(), 10.), Vec3::NEG_Z)?;
    let drag = RotateDrag::begin(baseline, parent, 2, press, Some(15_f32.to_radians()))?;
    let result = drag.update(moved)?;
    assert!(result.rotation.abs_diff_eq(
        baseline.rotation * Quat::from_rotation_z(30_f32.to_radians()),
        1e-5
    ));
    assert_eq!(result.translation, baseline.translation);
    assert_eq!(result.scale, baseline.scale);
    assert!(drag.update(ray(Vec3::Z * 10., Vec3::NEG_Z)?).is_err());
    assert!(drag.update(ray(Vec3::Z * 10., Vec3::X)?).is_err());
    assert_eq!(drag.update(moved)?, result);
    assert_eq!(drag.update(press)?, baseline);
    for (matrix, axis, step) in [
        (Mat4::ZERO, 2, None),
        (parent, 3, None),
        (parent, 2, Some(0.)),
    ] {
        assert!(RotateDrag::begin(baseline, matrix, axis, press, step).is_err());
    }
    Ok(())
}

#[test]
fn axis_and_uniform_scale_keep_signs_ratios_and_pose_and_reject_zero_crossing()
-> Result<(), &'static str> {
    let baseline =
        Transform::from_rotation(Quat::from_rotation_z(0.4)).with_scale(Vec3::new(-2., 3., 0.5));
    let parent = Mat4::from_scale(Vec3::new(3., -2., 1.));
    for axis in [Some(0), None] {
        let drag = ScaleDrag::begin(baseline, parent, axis, Vec3::X, 2., ray(0.), Some(0.1))?;
        let result = drag.update(ray(1.1))?;
        let expected = if axis.is_some() {
            Vec3::new(-3.2, 3., 0.5)
        } else {
            baseline.scale * 1.6
        };
        assert!(result.scale.abs_diff_eq(expected, 1e-5));
        assert_eq!(result.translation, baseline.translation);
        assert_eq!(result.rotation, baseline.rotation);
        assert!(drag.update(ray(-2.)).is_err());
        assert!(drag.update(ray(-3.)).is_err());
        assert!(drag.update(Ray3d::new(Vec3::ZERO, Dir3::X)).is_err());
        assert_eq!(drag.update(ray(1.1))?, result);
        assert_eq!(drag.update(ray(0.))?, baseline);
    }
    for length in [0., -1., f32::NAN] {
        assert!(ScaleDrag::begin(baseline, parent, None, Vec3::X, length, ray(0.), None).is_err());
    }
    assert!(
        ScaleDrag::begin(
            baseline.with_scale(Vec3::ZERO),
            parent,
            None,
            Vec3::X,
            1.,
            ray(0.),
            None
        )
        .is_err()
    );
    assert!(ScaleDrag::begin(baseline, Mat4::ZERO, None, Vec3::X, 1., ray(0.), None).is_err());
    assert!(ScaleDrag::begin(baseline, parent, Some(3), Vec3::X, 1., ray(0.), None).is_err());
    Ok(())
}
