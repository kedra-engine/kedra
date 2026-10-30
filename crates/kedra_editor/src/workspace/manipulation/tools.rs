//! Editor tool choice and typed geometry candidates share one gesture lifecycle.
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Move,
    Rotate,
    Scale,
}
impl Mode {
    #[cfg(debug_assertions)]
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Rotate => "rotate",
            Self::Scale => "scale",
        }
    }
}
pub(super) enum Constraint {
    Move(kedra_gizmo::MoveDrag),
    Rotate(kedra_gizmo::RotateDrag),
    Scale(kedra_gizmo::ScaleDrag),
}
impl Constraint {
    pub(super) fn update(&self, ray: Ray3d) -> Result<Transform, &'static str> {
        match self {
            Self::Move(d) => d.update(ray),
            Self::Rotate(d) => d.update(ray),
            Self::Scale(d) => d.update(ray),
        }
    }
}
