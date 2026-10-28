//! Shared UI observation contracts, independent of editor and automation transport.
//! IDs describe user-facing elements; ECS entity IDs are only ephemeral handles.
use bevy::prelude::*;
pub mod authored;
pub mod docking;
pub mod fields;
pub mod native_input;
pub mod popup;
/// Ordered opt-in native text editing and local history, independent of authoring.
pub use kedra_text_input as text_input;
pub mod styles;
#[cfg(all(feature = "text-capture", debug_assertions))]
pub mod text_capture;
mod text_fit;
pub mod tooltip;
pub mod window_chrome;

/// Stable feature-owned identifier, unique within a native window.
/// Duplicate visible IDs are an observation error, never resolved arbitrarily.
#[derive(Component, Clone, Debug, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct ElementId(pub String);
impl ElementId {
    /// Construct an identifier owned by the feature rendering this element.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Application-owned screen identity. Removing its root retires its descendants,
/// including scenes whose assets have not finished loading.
#[derive(Component, Clone, Debug, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct ScreenId(pub String);

/// Axis-aligned bounds in Bevy logical UI coordinates, including native UI
/// rotation/scale. Computed sizes/transforms are physical; the node's inverse
/// scale converts them once (it includes target DPR and `UiScale`). To obtain
/// logical window coordinates, callers multiply by `UiScale` and add the camera's
/// logical viewport origin. This preserves the renderer's native unit contract.
/// This does not approximate text widths, clipping or rounded-corner hit regions.
pub fn logical_rect(node: &ComputedNode, transform: &UiGlobalTransform) -> [f32; 4] {
    let scale = node.inverse_scale_factor();
    let half = node.size() * 0.5;
    let corners = [
        -half,
        Vec2::new(half.x, -half.y),
        half,
        Vec2::new(-half.x, half.y),
    ]
    .map(|point| transform.affine().transform_point2(point) * scale);
    let min = corners
        .into_iter()
        .fold(Vec2::splat(f32::INFINITY), Vec2::min);
    let max = corners
        .into_iter()
        .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max);
    [min.x, min.y, max.x, max.y]
}
