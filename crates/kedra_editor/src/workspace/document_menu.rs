//! Creation kinds share the existing authored popup and button controls.
use super::*;
use kedra_scene::document::DocumentKind;
pub(super) fn entry_items(view: ViewId) -> Result<Vec<(String, input::Action)>, String> {
    Ok(items(kedra_ui::popup::from_paths([
        (
            "Create/Scene",
            input::Action::BrowserCreate(view, DocumentKind::Scene),
        ),
        (
            "Create/Prefab",
            input::Action::BrowserCreate(view, DocumentKind::Prefab),
        ),
        (
            "Create/Folder",
            input::Action::Folder(view, folders::Mode::Create, None),
        ),
        ("Create/Environment", input::Action::EnvironmentCreate(view)),
        ("Import files", input::Action::BrowserImport(view)),
    ])?))
}
fn items(entries: Vec<kedra_ui::popup::Entry<input::Action>>) -> Vec<(String, input::Action)> {
    entries
        .into_iter()
        .map(|entry| match entry {
            kedra_ui::popup::Entry::Command { label, command } => (label, command),
            kedra_ui::popup::Entry::Submenu { label, children } => {
                (label, input::Action::Submenu(children))
            }
        })
        .collect()
}
pub(super) fn open(
    world: &mut World,
    anchor: Entity,
    entries: Vec<kedra_ui::popup::Entry<input::Action>>,
) -> Result<(), String> {
    let scope = format!(
        "{}/submenu",
        world
            .get::<kedra_ui::ElementId>(anchor)
            .ok_or("Menu branch identity missing")?
            .0
    );
    let popup = kedra_ui::popup::cascade::attach(world, anchor, "dock-menu", &scope)?;
    if world
        .get::<Children>(popup)
        .is_some_and(|children| !children.is_empty())
    {
        return Ok(());
    }
    populate(world, popup, &scope, entries)
}
pub(super) fn populate(
    world: &mut World,
    popup: Entity,
    scope: &str,
    entries: Vec<kedra_ui::popup::Entry<input::Action>>,
) -> Result<(), String> {
    for (index, (title, action)) in items(entries).into_iter().enumerate() {
        let branch = matches!(action, input::Action::Submenu(_));
        let button = crate::views::mount_asset(
            world,
            popup,
            "editor://prefabs/button.prefab.ron",
            &format!("{scope}/{index}"),
        )?;
        let caption = world
            .query::<(Entity, &ChildOf, &Text)>()
            .iter(world)
            .find_map(|(entity, parent, _)| (parent.parent() == button.root).then_some(entity))
            .ok_or("Creation menu caption missing")?;
        world
            .get_mut::<Text>(caption)
            .ok_or("Creation menu caption retired")?
            .0 = title;
        world
            .entity_mut(button.root)
            .remove::<(bevy::ui_widgets::Button, bevy::a11y::AccessibilityNode)>()
            .insert((
                bevy::ui_widgets::MenuItem,
                action,
                bevy::input_focus::tab_navigation::TabIndex(index as i32),
                kedra_ui::ElementId::new(format!("{scope}/{index}")),
            ));
        if branch {
            super::popup::branch_indicator(world, button.root, caption);
            kedra_ui::popup::cascade::branch(world, button.root);
        }
        world
            .entity_mut(caption)
            .insert(kedra_ui::ElementId::new(format!("{scope}/{index}/caption")));
    }
    Ok(())
}
