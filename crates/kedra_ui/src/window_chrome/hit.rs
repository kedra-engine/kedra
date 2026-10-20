//! Resolve current renderer geometry, not a second native window layout.
use super::{BoundHeader, HeaderRegion, RegionRole, under};
use crate::native_input::owner;
use bevy::prelude::*;

fn ui_point(world: &World, mut entity: Entity, point: Vec2) -> Option<Vec2> {
    let mut camera = None;
    for _ in 0..256 {
        if let Some(target) = world.get::<UiTargetCamera>(entity) {
            camera = Some(target.0);
            break;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            break;
        };
        entity = parent.parent();
    }
    let camera = world.get::<Camera>(camera?)?;
    let origin = if camera.viewport.is_some() {
        camera.logical_viewport_rect()?.min
    } else {
        Vec2::ZERO
    };
    let scale = world.get_resource::<UiScale>().map_or(1., |s| s.0);
    (scale.is_finite() && scale > 0.).then_some((point - origin) / scale)
}

/// Whether the actual visible, clipped UI geometry contains a logical window point.
/// Disabled content still occupies its region. Missing/stale layout fails closed.
pub fn visible_at(world: &World, entity: Entity, point: Vec2) -> bool {
    if !point.is_finite() {
        return false;
    }
    let Some(point) = ui_point(world, entity, point) else {
        return false;
    };
    let Some(node) = world.get::<ComputedNode>(entity) else {
        return false;
    };
    let Some(transform) = world.get::<UiGlobalTransform>(entity) else {
        return false;
    };
    let [x0, y0, x1, y1] = crate::logical_rect(node, transform);
    if ![x0, y0, x1, y1].iter().all(|v| v.is_finite())
        || x1 <= x0
        || y1 <= y0
        || !Rect::new(x0, y0, x1, y1).contains(point)
    {
        return false;
    }
    let mut current = entity;
    for _ in 0..256 {
        if world
            .get::<Node>(current)
            .is_some_and(|n| n.display == Display::None)
            || world.get::<Visibility>(current) == Some(&Visibility::Hidden)
            || world
                .get::<InheritedVisibility>(current)
                .is_some_and(|v| !v.get())
            || world
                .get::<bevy_flair::style::components::ClassList>(current)
                .is_some_and(|c| c.contains("hidden"))
        {
            return false;
        }
        if let Some(clip) = world.get::<CalculatedClip>(current) {
            let Some(node) = world.get::<ComputedNode>(current) else {
                return false;
            };
            if !clip.contains_point(point / node.inverse_scale_factor()) {
                return false;
            }
        }
        let Some(parent) = world.get::<ChildOf>(current) else {
            return true;
        };
        current = parent.parent();
    }
    false
}

/// Classify one logical window point. Unbound, stale, ambiguous or unmeasured
/// headers return `None`. Explicit controls and native reserves beat drag regions.
/// Modal roots in this window block native gestures; other windows are unaffected.
pub fn hit(world: &World, window: Entity, point: Vec2) -> Option<RegionRole> {
    if !point.is_finite() {
        return None;
    }
    let mut headers = world
        .iter_entities()
        .filter(|e| e.get::<BoundHeader>().is_some_and(|b| b.window == window));
    let root = headers.next()?.id();
    if headers.next().is_some() || owner(world, root) != Some(window) {
        return None;
    }
    if world.iter_entities().any(|e| {
        e.contains::<bevy::ui_widgets::ModalDialog>() && owner(world, e.id()) == Some(window)
    }) {
        return Some(RegionRole::Client);
    }
    let mut client = false;
    let mut native = false;
    let mut drag = false;
    for entity in world.iter_entities() {
        if owner(world, entity.id()) != Some(window) {
            continue;
        }
        if !visible_at(world, entity.id(), point) {
            continue;
        }
        if entity.contains::<bevy::ui_widgets::Button>()
            || entity.contains::<bevy::ui_widgets::TextInput>()
            || entity.contains::<bevy::ui_widgets::MenuItem>()
            || entity.contains::<bevy::ui_widgets::MenuPopup>()
        {
            client = true;
        }
        if let Some(region) = entity.get::<HeaderRegion>()
            && region.header == root
            && under(world, entity.id(), root)
        {
            match region.role {
                RegionRole::NativeControls => native = true,
                RegionRole::Client => client = true,
                RegionRole::Drag => drag = true,
            }
        }
    }
    // Client exclusions cover the whole native frame, not only its title row.
    // This protects controls extending into a resize strip below the caption.
    if native {
        Some(RegionRole::NativeControls)
    } else if client {
        Some(RegionRole::Client)
    } else if !visible_at(world, root, point) {
        None
    } else if drag {
        Some(RegionRole::Drag)
    } else {
        Some(RegionRole::Client)
    }
}
