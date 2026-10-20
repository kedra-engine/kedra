//! Consumer-supplied window headers, borrowed content slots and native input regions.
//!
//! This module never creates windows, invokes OS operations or knows editor commands.
//! Call `Header::unbind` before retiring its authored tree to park borrowed content.
//! Directly despawning an owner bypasses this lifecycle contract, as with any UI tree.
mod captions;
mod hit;
mod slots;
use bevy::prelude::*;
pub use captions::set_title;
pub use hit::{hit, visible_at};
pub use slots::Header;

/// Semantic input ownership; only explicit `Drag` backgrounds may move a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionRole {
    /// Feature input, including disabled buttons and docking labels.
    Client,
    /// Empty background that may initiate a native window gesture.
    Drag,
    /// Space reserved for real native controls; never initiate client gestures here.
    NativeControls,
}

/// Input region owned by one live header. Geometry comes from native Bevy layout.
#[derive(Component, Clone, Copy, Debug)]
pub struct HeaderRegion {
    /// Root registered through `Header::bind`, not a persisted identity.
    pub header: Entity,
    /// Region role. Client/native controls override overlapping drag backgrounds.
    pub role: RegionRole,
}

/// A consumer-authored content destination; header lifetime owns only this slot.
#[derive(Component, Clone, Copy, Debug)]
pub struct HeaderSlot {
    /// Header root owning the slot, assigned by `Header::bind`.
    pub header: Entity,
}

#[derive(Component)]
pub(crate) struct BoundHeader {
    pub window: Entity,
    pub generation: Entity,
}

pub(crate) fn under(world: &World, mut entity: Entity, root: Entity) -> bool {
    for _ in 0..256 {
        if entity == root {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
    false
}
