//! Retained native docking projection over the renderer-independent model.
//!
//! Consumers own content roots and native surfaces. This module mounts only
//! authored chrome, reparents registered content, and lets Bevy UI/Taffy lay out
//! the hierarchy. It never creates windows, saves documents or approves closure.
mod project;
mod title;
use bevy::prelude::*;
use kedra_docking::model::{HostId, NodeId, ViewId};
pub use project::{Geometry, Projection, Skin, class, geometry};

/// Input binding on authored controls; events remain ordinary Bevy UI events.
#[derive(Component, Clone, Copy, Debug)]
pub enum Control {
    /// Select a view. Drag originates only from this label, never group actions.
    Tab(ViewId),
    /// Request feature-owned close approval.
    Close(ViewId),
    /// Open the owning group's action menu.
    Menu(NodeId),
    /// Keyboard activation restores a split's application-supplied preferred ratio.
    ResetSplit(NodeId),
}

/// Native horizontal tab scrolling surface.
#[derive(Component, Clone, Copy)]
pub struct TabRail {
    /// Owning logical host.
    pub host: HostId,
    /// Owning tab group, used to distinguish committed and measurement surfaces.
    pub group: NodeId,
}

/// A tab label used by drag targeting and keyboard/overflow navigation.
#[derive(Component, Clone, Copy)]
pub struct TabLabel {
    /// Stable instance identity.
    pub view: ViewId,
    /// Owning group in the currently projected topology.
    pub group: NodeId,
    /// Logical host, not a native entity.
    pub host: HostId,
}

/// A laid-out group used for direct edge/tab targeting.
#[derive(Component, Clone, Copy)]
pub struct GroupTarget {
    /// Group identity.
    pub group: NodeId,
    /// Owning logical host.
    pub host: HostId,
}

/// An authored divider's enlarged native hit target.
#[derive(Component, Clone, Copy)]
pub struct Divider {
    /// Split identity.
    pub split: NodeId,
    /// Owning logical host.
    pub host: HostId,
}

/// Bevy logical-UI AABB from native layout/transform. As with
/// [`crate::logical_rect`], conversion to logical window coordinates (`UiScale`
/// and camera viewport origin) is caller-owned.
pub fn rect(world: &World, entity: Entity) -> Option<Rect> {
    let node = world.get::<ComputedNode>(entity)?;
    let transform = world.get::<UiGlobalTransform>(entity)?;
    let [x0, y0, x1, y1] = crate::logical_rect(node, transform);
    Some(Rect::from_corners(Vec2::new(x0, y0), Vec2::new(x1, y1)))
}
