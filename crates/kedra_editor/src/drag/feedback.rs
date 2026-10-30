//! One noninteractive identity preview shared by native editor payload producers.
use super::*;
#[derive(Component)]
struct Ghost {
    owner: Entity,
    caption: Entity,
}
pub(super) fn update(world: &mut World) {
    let active = super::active(world);
    let old: Vec<_> = world
        .query::<(Entity, &Ghost)>()
        .iter(world)
        .map(|(e, g)| (e, g.owner))
        .collect();
    for (entity, owner) in old {
        if active.as_ref().is_none_or(|(current, _)| *current != owner) {
            world.despawn(entity);
        }
    }
    let Some((owner, payload)) = active else {
        return;
    };
    let Some(window) = kedra_ui::native_input::owner(world, owner) else {
        return;
    };
    let Some(position) = world
        .get::<Window>(window)
        .and_then(Window::cursor_position)
    else {
        return;
    };
    let existing = world
        .query::<(Entity, &Ghost)>()
        .iter(world)
        .find_map(|(e, g)| (g.owner == owner).then_some((e, g.caption)));
    let (root, caption) = if let Some(existing) = existing {
        existing
    } else {
        let Some(parent) = crate::workspace::drag_surface(world, window) else {
            return;
        };
        let Ok(view) = crate::views::mount_asset(
            world,
            parent,
            "editor://prefabs/button.prefab.ron",
            "drag-preview",
        ) else {
            return;
        };
        let Some(caption) = world
            .query::<(Entity, &ChildOf, &Text)>()
            .iter(world)
            .find_map(|(e, p, _)| (p.parent() == view.root).then_some(e))
        else {
            world.despawn(view.root);
            return;
        };
        world
            .entity_mut(view.root)
            .remove::<(
                bevy::ui_widgets::Button,
                bevy::input_focus::tab_navigation::TabIndex,
            )>()
            .insert((Ghost { owner, caption }, GlobalZIndex(500), OverrideClip));
        let children = world
            .get::<Children>(view.root)
            .map(|c| c.to_vec())
            .unwrap_or_default();
        for entity in std::iter::once(view.root).chain(children) {
            world.entity_mut(entity).insert(Pickable::IGNORE);
        }
        kedra_ui::docking::class(world, view.root, "drag-preview", true);
        (view.root, caption)
    };
    let label = crate::workspace::drag_label(world, &payload);
    if let Some(mut text) = world.get_mut::<Text>(caption) {
        text.0 = label;
    }
    if let Some(mut node) = world.get_mut::<Node>(root) {
        node.position_type = PositionType::Absolute;
        node.left = Val::Px(position.x + 16.);
        node.top = Val::Px(position.y + 18.);
    }
}
