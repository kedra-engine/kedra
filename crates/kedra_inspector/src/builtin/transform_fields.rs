//! Native Transform axis semantics. Source transactions remain entirely host-owned.
use super::*;
use crate::binding::BindingToken;

/// Native property represented by one inline Transform vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformPart {
    /// Native local position.
    Translation,
    /// Euler degree projection of the native quaternion (YXZ order).
    Rotation,
    /// Native local scale.
    Scale,
}
/// Retained input metadata emitted by the built-in drawer for opt-in host editing.
/// This is an immutable source-value baseline, not authority to change that source.
#[derive(Component, Clone)]
pub struct TransformField {
    /// Presentation subtree owning the native input.
    pub owner: Entity,
    /// Shared row diagnostic, not a parallel notification surface.
    pub diagnostic: Entity,
    /// Original host target identity.
    pub token: BindingToken,
    /// Native property represented by this input.
    pub part: TransformPart,
    /// X/Y/Z index in the declared presentation.
    pub axis: usize,
    /// Effective native value from the last drawer refresh.
    pub value: Transform,
}
impl TransformField {
    /// Canonical source display; no quaternion mutation occurs during projection.
    pub fn display(&self) -> Result<String, String> {
        let vector = match self.part {
            TransformPart::Translation => self.value.translation,
            TransformPart::Scale => self.value.scale,
            TransformPart::Rotation => rotation_degrees(self.value.rotation)?,
        };
        vector
            .to_array()
            .get(self.axis)
            .map(ToString::to_string)
            .ok_or("Transform axis is unavailable".into())
    }
    /// Parse one finite native f32 and preserve all untouched native properties.
    /// Full component/domain bounds still require host/source policy validation.
    /// Re-entering an unchanged Euler projection preserves the original quaternion.
    pub fn propose(&self, text: &str) -> Result<Transform, String> {
        let value = text
            .parse::<f32>()
            .map_err(|_| "Enter a finite number, or press Escape to cancel.")?;
        if !value.is_finite() || self.axis >= 3 {
            return Err("Enter a finite number, or press Escape to cancel.".into());
        }
        let mut result = self.value;
        match self.part {
            TransformPart::Translation => result.translation[self.axis] = value,
            TransformPart::Scale => result.scale[self.axis] = value,
            TransformPart::Rotation => {
                let mut angles = rotation_degrees(result.rotation)?;
                if angles[self.axis] != value {
                    angles[self.axis] = value;
                    result.rotation = Quat::from_euler(
                        EulerRot::YXZ,
                        angles.y.to_radians(),
                        angles.x.to_radians(),
                        angles.z.to_radians(),
                    );
                }
            }
        }
        Ok(result)
    }
}
