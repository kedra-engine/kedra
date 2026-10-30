//! Constrained transform gestures without global input, windows or authoring authority.
//!
//! A host freezes the parent matrix and local baseline when acquiring a gesture.
//! Updates return candidates; dropping the gesture cancels without side effects.
//! The host owns selection, revision checks, preview, publication and history.
use bevy::{gizmos::transform_gizmo::translation_plane_normal, prelude::*};
mod rotation;
mod scale;
pub use rotation::RotateDrag;
pub use scale::ScaleDrag;

fn validate(baseline: Transform, parent: Mat4, step: Option<f32>) -> Result<(), &'static str> {
    if !baseline.to_matrix().is_finite()
        || !parent.is_finite()
        || !parent.inverse().is_finite()
        || parent.determinant().abs() < 1e-8
        || (baseline.rotation.length_squared() - 1.).abs() > 1e-4
        || step.is_some_and(|s| !s.is_finite() || s <= 0.)
    {
        return Err(
            "Manipulation requires finite transforms, an invertible parent, a unit rotation and a positive snap step.",
        );
    }
    Ok(())
}

/// A single-axis move with a frozen world-space constraint and parent inverse.
#[derive(Clone)]
pub struct MoveDrag {
    baseline: Transform,
    inverse_parent: Mat4,
    axis: Vec3,
    origin: Vec3,
    normal: Dir3,
    start: f32,
    step: Option<f32>,
}

impl MoveDrag {
    /// Acquire a world-axis constraint from the press ray.
    ///
    /// Rejects nonfinite transforms, a singular parent, invalid snapping increments,
    /// degenerate axes and rays too nearly aligned with the axis. No values are mutated.
    /// Snap is a distance along the axis relative to the gesture's initial position.
    pub fn begin(
        baseline: Transform,
        parent: Mat4,
        axis: Vec3,
        ray: Ray3d,
        step: Option<f32>,
    ) -> Result<Self, &'static str> {
        if !baseline.to_matrix().is_finite()
            || !parent.is_finite()
            || parent.determinant().abs() < 1e-8
            || step.is_some_and(|step| !step.is_finite() || step <= 0.)
        {
            return Err(
                "Move requires finite transforms, an invertible parent and a positive snap step.",
            );
        }
        let axis = axis.try_normalize().ok_or("Move axis is degenerate.")?;
        if ray.direction.cross(axis).length_squared() < 1e-4 {
            return Err("Move axis is aligned with the camera; choose another view.");
        }
        let normal = Dir3::new(translation_plane_normal(ray, axis))
            .map_err(|_| "Move constraint is degenerate.")?;
        let origin = parent.transform_point3(baseline.translation);
        let distance = ray
            .intersect_plane(origin, InfinitePlane3d::new(normal))
            .ok_or("Pointer does not intersect the move constraint.")?;
        Ok(Self {
            baseline,
            inverse_parent: parent.inverse(),
            axis,
            origin,
            normal,
            start: (ray.get_point(distance) - origin).dot(axis),
            step,
        })
    }

    /// Evaluate a candidate using the original constraint, never accumulating deltas.
    /// Returns an error for a parallel/behind-camera ray or nonfinite result. Rotation,
    /// scale (including negative scale), and the stored baseline remain unchanged.
    pub fn update(&self, ray: Ray3d) -> Result<Transform, &'static str> {
        let distance = ray
            .intersect_plane(self.origin, InfinitePlane3d::new(self.normal))
            .ok_or("Pointer does not intersect the move constraint.")?;
        let mut delta = (ray.get_point(distance) - self.origin).dot(self.axis) - self.start;
        if let Some(step) = self.step {
            delta = (delta / step).round() * step;
        }
        let translation =
            self.baseline.translation + self.inverse_parent.transform_vector3(self.axis * delta);
        if !translation.is_finite() {
            return Err("Move produced a nonfinite translation.");
        }
        Ok(Transform {
            translation,
            ..self.baseline
        })
    }
}
