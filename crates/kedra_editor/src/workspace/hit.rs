//! Manual gesture origins must honor the committed projection and renderer clips.
use super::*;
use kedra_ui::docking::{Divider, TabLabel, rect};

pub(super) fn group(
    world: &World,
    state: &EditorWorkspace,
    host: HostId,
    group: NodeId,
    point: Vec2,
) -> bool {
    let snapshot = state.layout.snapshot();
    state.rendered == Some(state.layout.revision())
        && snapshot.host_of(group) == Some(host)
        && snapshot.hosts[&host]
            .maximized
            .is_none_or(|max| max == group)
        && state
            .surfaces
            .get(&host)
            .is_some_and(|s| s.staged.is_none())
        && state.renderer.group(group).is_some_and(|entity| {
            state
                .surfaces
                .get(&host)
                .is_some_and(|s| under(world, entity, s.root))
                && visible_at(world, entity, point, 0.)
        })
}

pub(super) fn label(
    world: &World,
    state: &EditorWorkspace,
    entity: Entity,
    label: &TabLabel,
    point: Vec2,
) -> bool {
    state.renderer.label(label.view) == Some(entity)
        && state
            .layout
            .snapshot()
            .location(label.view)
            .is_some_and(|(group, _)| group == label.group)
        && group(world, state, label.host, label.group, point)
        && state
            .renderer
            .rail(label.group)
            .is_some_and(|rail| under(world, entity, rail) && visible_at(world, rail, point, 0.))
        && visible_at(world, entity, point, 0.)
}

pub(super) fn divider(
    world: &World,
    state: &EditorWorkspace,
    entity: Entity,
    divider: &Divider,
    point: Vec2,
) -> bool {
    let snapshot = state.layout.snapshot();
    state.rendered == Some(state.layout.revision())
        && snapshot.host_of(divider.split) == Some(divider.host)
        && snapshot.hosts[&divider.host].maximized.is_none()
        && state
            .surfaces
            .get(&divider.host)
            .is_some_and(|s| s.staged.is_none())
        && state
            .renderer
            .group(divider.split)
            .is_some_and(|root| under(world, entity, root))
        && visible_at(world, entity, point, 3.)
}

fn under(world: &World, mut entity: Entity, root: Entity) -> bool {
    for _ in 0..128 {
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

pub(super) fn visible_at(world: &World, mut entity: Entity, point: Vec2, margin: f32) -> bool {
    if !point.is_finite()
        || !rect(world, entity)
            .is_some_and(|r| r.size().min_element() > 0. && r.inflate(margin).contains(point))
    {
        return false;
    }
    for _ in 0..128 {
        if world
            .get::<Node>(entity)
            .is_some_and(|node| node.display == Display::None)
            || world.get::<Visibility>(entity) == Some(&Visibility::Hidden)
            || world
                .get::<InheritedVisibility>(entity)
                .is_some_and(|v| !v.get())
            || world.get::<bevy::ui::InteractionDisabled>(entity).is_some()
            || world
                .get::<bevy_flair::style::components::ClassList>(entity)
                .is_some_and(|c| c.contains("hidden"))
        {
            return false;
        }
        if let Some(clip) = world.get::<CalculatedClip>(entity) {
            let Some(node) = world.get::<ComputedNode>(entity) else {
                return false;
            };
            // CalculatedClip uses renderer coordinates; gesture points are logical.
            if !clip.contains_point(point / node.inverse_scale_factor()) {
                return false;
            }
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return true;
        };
        entity = parent.parent();
    }
    false
}
