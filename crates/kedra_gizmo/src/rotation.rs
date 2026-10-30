//! Rotation is evaluated in the frozen local orientation, before local scale.
use bevy::prelude::*;

/// Local-axis rotation preserving translation and signed scale exactly.
///
/// Rings live in `parent * translation * rotation` space, excluding the object's
/// scale. Under a nonuniform parent they are ellipses in world space. No affine
/// decomposition or shear approximation is performed. The host owns publication.
#[derive(Clone)]
pub struct RotateDrag {
    baseline: Transform,
    inverse_frame: Mat4,
    axis: Dir3,
    start: Vec3,
    step: Option<f32>,
}
impl RotateDrag {
    /// Acquire a local axis (0=X, 1=Y, 2=Z). Snap is in radians relative to press.
    /// Rejects invalid transforms/axes/steps and parallel or center-point rays.
    /// No input, scene or source state is modified.
    pub fn begin(
        baseline: Transform,
        parent: Mat4,
        axis: usize,
        ray: Ray3d,
        step: Option<f32>,
    ) -> Result<Self, &'static str> {
        super::validate(baseline, parent, step)?;
        let axis = [Dir3::X, Dir3::Y, Dir3::Z]
            .get(axis)
            .copied()
            .ok_or("Invalid rotation axis.")?;
        let frame =
            parent * Mat4::from_rotation_translation(baseline.rotation, baseline.translation);
        let mut drag = Self {
            baseline,
            inverse_frame: frame.inverse(),
            axis,
            start: Vec3::ZERO,
            step,
        };
        drag.start = drag.direction(ray)?;
        Ok(drag)
    }
    fn direction(&self, ray: Ray3d) -> Result<Vec3, &'static str> {
        let direction = Dir3::new(self.inverse_frame.transform_vector3(*ray.direction))
            .map_err(|_| "Rotation ray is degenerate.")?;
        if direction.dot(*self.axis).abs() < 1e-4 {
            return Err("Rotation ring is edge-on; choose another view.");
        }
        let ray = Ray3d::new(self.inverse_frame.transform_point3(ray.origin), direction);
        let distance = ray
            .intersect_plane(Vec3::ZERO, InfinitePlane3d::new(self.axis))
            .ok_or("Pointer does not intersect the rotation ring.")?;
        let point = ray.get_point(distance);
        if point.length_squared() < 1e-10 {
            return Err("Rotation direction is undefined at the center.");
        }
        point
            .try_normalize()
            .ok_or("Rotation direction is undefined at the center.")
    }
    /// Return an absolute candidate from the press baseline. Rejected rays leave
    /// this gesture unchanged; translation and scale are never altered.
    pub fn update(&self, ray: Ray3d) -> Result<Transform, &'static str> {
        let current = self.direction(ray)?;
        let mut angle = self
            .axis
            .dot(self.start.cross(current))
            .atan2(self.start.dot(current));
        if let Some(step) = self.step {
            angle = (angle / step).round() * step;
        }
        if angle.abs() < 1e-6 {
            return Ok(self.baseline);
        }
        let rotation =
            (self.baseline.rotation * Quat::from_axis_angle(*self.axis, angle)).normalize();
        if !rotation.is_finite() {
            return Err("Rotation produced a nonfinite result.");
        }
        Ok(Transform {
            rotation,
            ..self.baseline
        })
    }
}
