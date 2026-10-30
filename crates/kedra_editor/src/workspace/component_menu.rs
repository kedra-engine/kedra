//! Registry-discovered component operations scoped to the Inspector's retained target.
use super::*;
use kedra_ui::authored::components::{UiImage, UiImageDensity};

#[derive(Component)]
struct ComponentControl;
#[derive(Component)]
struct DockAdd(NodeId);

fn icon(
    world: &mut World,
    parent: Entity,
    scope: &str,
    name: &str,
    label: &str,
) -> Result<Entity, String> {
    let view = crate::views::mount_asset(
        world,
        parent,
        "editor://prefabs/browser-icon.prefab.ron",
        scope,
    )?;
    let image = world
        .query::<(Entity, &ChildOf, &UiImage)>()
        .iter(world)
        .find_map(|(e, p, _)| (p.parent() == view.root).then_some(e))
        .ok_or("Icon missing")?;
    let variants = (1..=4)
        .map(|density| UiImageDensity {
            density: density as f32,
            image: world
                .resource::<AssetServer>()
                .load(format!("editor://icons/{name}.svg#{density}x")),
        })
        .collect();
    world
        .get_mut::<UiImage>(image)
        .ok_or("Icon retired")?
        .variants = variants;
    world.entity_mut(view.root).insert((
        kedra_ui::tooltip::Tooltip(label.into()),
        kedra_ui::ElementId::new(scope),
    ));
    if let Some(mut a11y) = world.get_mut::<bevy::a11y::AccessibilityNode>(view.root) {
        a11y.set_label(label);
    }
    Ok(view.root)
}
pub(crate) fn bind_component(
    world: &mut World,
    root: Entity,
    scope: &str,
    command: Option<crate::documents::components::ComponentCommand>,
) -> Result<(), String> {
    let existing = world
        .query_filtered::<(Entity, &ChildOf), With<ComponentControl>>()
        .iter(world)
        .find_map(|(e, p)| (p.parent() == root).then_some(e));
    let button = if let Some(button) = existing {
        button
    } else {
        let button = icon(
            world,
            root,
            &format!("{scope}/menu"),
            "dock-more",
            "Component menu",
        )?;
        world.entity_mut(button).insert(ComponentControl);
        kedra_ui::docking::class(world, button, "component-menu", true);
        kedra_ui::docking::class(world, root, "component-section", true);
        button
    };
    if let Some(command) = command {
        world
            .entity_mut(button)
            .insert(input::Action::ComponentMenu(command))
            .remove::<bevy::ui::InteractionDisabled>();
    } else {
        world
            .entity_mut(button)
            .remove::<input::Action>()
            .insert(bevy::ui::InteractionDisabled);
    }
    Ok(())
}
pub(super) fn sync_docks(world: &mut World, state: &EditorWorkspace) -> Result<(), String> {
    let menus: Vec<_> = world
        .query::<(Entity, &kedra_ui::docking::Control, &ChildOf)>()
        .iter(world)
        .filter_map(|(e, c, p)| {
            if let kedra_ui::docking::Control::Menu(group) = c {
                Some((e, *group, p.parent()))
            } else {
                None
            }
        })
        .collect();
    for (menu, group, parent) in menus {
        let active = match state.layout.snapshot().nodes.get(&group) {
            Some(kedra_docking::model::Node::Group { active, .. })
                if state.panels.inspectors.contains_key(active) =>
            {
                Some(*active)
            }
            _ => None,
        };
        let existing = world
            .query::<(Entity, &DockAdd, &ChildOf)>()
            .iter(world)
            .find_map(|(e, owner, p)| (owner.0 == group && p.parent() == parent).then_some(e));
        let Some(active) = active else {
            if let Some(e) = existing {
                world.despawn(e);
            }
            continue;
        };
        let button = if let Some(e) = existing {
            e
        } else {
            let e = icon(
                world,
                parent,
                &format!("inspector/{active:?}/add"),
                "plus",
                "Add component",
            )?;
            world.entity_mut(e).insert(DockAdd(group));
            let mut children = world
                .get::<Children>(parent)
                .ok_or("Dock header missing")?
                .to_vec();
            children.retain(|child| *child != e);
            let index = children
                .iter()
                .position(|child| *child == menu)
                .unwrap_or(children.len());
            children.insert(index, e);
            world.entity_mut(parent).replace_children(&children);
            e
        };
        world
            .entity_mut(button)
            .insert(input::Action::Components(active, true));
        let enabled =
            state.panels.inspectors[&active].state.target.is_some() && !state.documents.save_busy();
        if enabled {
            world
                .entity_mut(button)
                .remove::<bevy::ui::InteractionDisabled>();
        } else {
            world
                .entity_mut(button)
                .insert(bevy::ui::InteractionDisabled);
        }
    }
    Ok(())
}
pub(super) fn open(
    world: &mut World,
    state: &mut EditorWorkspace,
    anchor: Entity,
    view: ViewId,
    add: bool,
) -> Result<(), String> {
    let target = state
        .panels
        .inspectors
        .get(&view)
        .and_then(|panel| panel.state.target.as_ref())
        .ok_or("Inspector target unavailable")?;
    let commands = state.documents.component_commands(
        target,
        world.resource::<kedra_scene::reflection::ComponentPolicies>(),
    )?;
    let popup = popup::attach(world, anchor)?;
    state.menu.push(popup);
    for (index, command) in commands
        .into_iter()
        .filter(|command| command.add == add)
        .enumerate()
    {
        let button = crate::views::mount_asset(
            world,
            popup,
            "editor://prefabs/button.prefab.ron",
            &format!("component-menu/{index}"),
        )?;
        let caption = world
            .query::<(Entity, &ChildOf, &Text)>()
            .iter(world)
            .find_map(|(id, parent, _)| (parent.parent() == button.root).then_some(id))
            .ok_or("Component caption missing")?;
        world
            .get_mut::<Text>(caption)
            .ok_or("Component caption retired")?
            .0 = command.key.clone();
        world
            .entity_mut(button.root)
            .remove::<(bevy::ui_widgets::Button, bevy::a11y::AccessibilityNode)>()
            .insert((
                bevy::ui_widgets::MenuItem,
                input::Action::Component(command),
                bevy::input_focus::tab_navigation::TabIndex(index as i32),
                kedra_ui::ElementId::new(format!("component-menu/{index}")),
            ));
        world
            .entity_mut(caption)
            .insert(kedra_ui::ElementId::new(format!(
                "component-menu/{index}/caption"
            )));
    }
    Ok(())
}
