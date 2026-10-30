//! Multiplicative local scaling without changing signs or flattening other axes.
use crate::MoveDrag;
use bevy::prelude::*;

/// Frozen axis or uniform scale gesture. The pointer's world-axis distance is
/// divided by the visible handle length to obtain a factor relative to one.
/// Existing scale ratios and signs are preserved by uniform scaling.
#[derive(Clone)]
pub struct ScaleDrag {
    baseline: Transform,
    motion: MoveDrag,
    origin: Vec3,
    direction: Vec3,
    length: f32,
    axis: Option<usize>,
    step: Option<f32>,
}
impl ScaleDrag {
    /// Acquire scaling of one local axis (0..=2), or all axes with `None`.
    /// `direction` is the rendered handle's normalized world direction; for the
    /// uniform handle the host supplies camera-right. `length` is its world length.
    /// Snap is a factor increment (0.1 means ten percent of the baseline scale).
    /// Rejects invalid transforms, zero baseline scales, axes, lengths and steps.
    pub fn begin(
        baseline: Transform,
        parent: Mat4,
        axis: Option<usize>,
        direction: Vec3,
        length: f32,
        ray: Ray3d,
        step: Option<f32>,
    ) -> Result<Self, &'static str> {
        super::validate(baseline, parent, step)?;
        if axis.is_some_and(|a| a > 2)
            || !length.is_finite()
            || length <= 0.
            || baseline.scale.abs().min_element() < 1e-6
        {
            return Err(
                "Scale requires nonzero baseline scales, a valid axis and a positive handle length.",
            );
        }
        let direction = direction
            .try_normalize()
            .ok_or("Scale direction is degenerate.")?;
        let origin = parent.transform_point3(baseline.translation);
        let motion = MoveDrag::begin(
            Transform::from_translation(origin),
            Mat4::IDENTITY,
            direction,
            ray,
            None,
        )?;
        Ok(Self {
            baseline,
            motion,
            origin,
            direction,
            length,
            axis,
            step,
        })
    }
    /// Return a candidate without accumulating deltas. Zero/sign-crossing factors
    /// and nonfinite results are rejected, retaining the caller's last valid preview.
    /// The host can cancel by dropping this immutable gesture.
    pub fn update(&self, ray: Ray3d) -> Result<Transform, &'static str> {
        let delta =
            (self.motion.update(ray)?.translation - self.origin).dot(self.direction) / self.length;
        let delta = self.step.map_or(delta, |s| (delta / s).round() * s);
        let factor = 1. + delta;
        if !factor.is_finite() || factor <= 1e-4 {
            return Err("Scale cannot cross zero; reverse the drag or cancel.");
        }
        let mut scale = self.baseline.scale;
        if let Some(axis) = self.axis {
            scale[axis] *= factor;
        } else {
            scale *= factor;
        }
        if !scale.is_finite() || scale.abs().min_element() < 1e-6 {
            return Err("Scale result is nonfinite or too close to zero.");
        }
        Ok(Transform {
            scale,
            ..self.baseline
        })
    }
}
