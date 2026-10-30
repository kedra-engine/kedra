//! Dock menus use Bevy's anchor/flip implementation, not host-relative offsets.
use bevy::prelude::*;

#[derive(Component)]
pub(super) struct DockPopup;
pub(super) fn separator(world: &mut World, parent: Entity) {
    world.spawn((
        Node::default(),
        ChildOf(parent),
        bevy::picking::Pickable::IGNORE,
        bevy_flair::style::components::ClassList::new("menu-separator"),
    ));
}
pub(super) fn branch_indicator(world: &mut World, parent: Entity, caption: Entity) {
    kedra_ui::docking::class(world, caption, "menu-caption", true);
    world.spawn((
        Text::new("›"),
        Node::default(),
        ChildOf(parent),
        bevy::picking::Pickable::IGNORE,
        bevy_flair::style::components::ClassList::new("text menu-arrow"),
    ));
}
pub(super) fn cascade_closed(
    event: On<kedra_ui::popup::cascade::Closed>,
    parents: Query<&ChildOf>,
    mut state: Option<ResMut<super::EditorWorkspace>>,
    mut commands: Commands,
) {
    let Some(state) = state.as_mut() else {
        return;
    };
    let owned = std::iter::once(event.entity)
        .chain(parents.iter_ancestors(event.entity))
        .find(|e| state.menu.contains(e));
    if let Some(owned) = owned {
        state.menu.retain(|e| *e != owned);
        if owned != event.entity {
            commands.entity(owned).try_despawn();
        }
    }
}

pub(super) fn attach(world: &mut World, anchor: Entity) -> Result<Entity, String> {
    let menu = kedra_ui::popup::attach(world, anchor, "dock-menu", "dock-menu")?;
    world.entity_mut(menu).insert(DockPopup);
    Ok(menu)
}
pub(super) fn attach_at(
    world: &mut World,
    anchor: Entity,
    position: Vec2,
) -> Result<kedra_ui::popup::PointMenu, String> {
    let menu = kedra_ui::popup::attach_at(world, anchor, position, "dock-menu", "dock-menu")?;
    world.entity_mut(menu.root).insert(DockPopup);
    Ok(menu)
}

// Bevy owns outside-click/focus-loss detection and keyboard traversal. Kedra
// only retires its popup stack and restores the invoking control's focus.
pub(super) fn menu_event(
    mut event: On<bevy::ui_widgets::MenuEvent>,
    parents: Query<&ChildOf>,
    popups: Query<(), With<DockPopup>>,
    cascading: Query<(), With<kedra_ui::popup::cascade::Cascade>>,
    mut commands: Commands,
) {
    if std::iter::once(event.source)
        .chain(parents.iter_ancestors(event.source))
        .any(|entity| cascading.contains(entity))
    {
        return;
    }
    let popup = std::iter::once(event.source)
        .chain(parents.iter_ancestors(event.source))
        .find(|entity| popups.contains(*entity));
    let Some(popup) = popup else {
        return;
    };
    let action = event.action;
    let anchor = parents.get(popup).ok().map(ChildOf::parent);
    event.propagate(false);
    commands.queue(move |world: &mut World| match action {
        bevy::ui_widgets::MenuAction::FocusRoot => {
            if let Some(anchor) = anchor.filter(|anchor| world.entities().contains(*anchor))
                && let Some(mut focus) = world.get_resource_mut::<bevy::input_focus::InputFocus>()
            {
                focus.set(anchor, bevy::input_focus::FocusCause::Navigated);
            }
        }
        bevy::ui_widgets::MenuAction::CloseAll
            if world.contains_resource::<super::EditorWorkspace>() =>
        {
            world.resource_scope(|world, mut state: Mut<super::EditorWorkspace>| {
                if state.menu.contains(&popup) {
                    super::input::dismiss(world, &mut state);
                }
            });
        }
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ui_widgets::popover::{Popover, PopoverSide};
    #[test]
    fn native_menu_focus_loss_closes_owned_stack_and_restores_focus_on_request()
    -> Result<(), String> {
        use bevy::input_focus::{FocusCause, InputFocus};
        use bevy::ui_widgets::{MenuAction, MenuEvent, MenuFocusState, MenuPlugin};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, MenuPlugin))
            .init_resource::<InputFocus>()
            .add_observer(menu_event);
        let world = app.world_mut();
        let window = world.spawn(Window::default()).id();
        let root = world.spawn_empty().id();
        super::super::bind_root(world, window, root)?;
        let button = world.spawn((Node::default(), ChildOf(root))).id();
        let popup = attach(world, button)?;
        // This headless fixture has no layout pass; model the mounted native menu.
        world
            .entity_mut(popup)
            .insert((bevy::ui_widgets::MenuPopup::default(), MenuFocusState::Open));
        let item = world.spawn((Node::default(), ChildOf(popup))).id();
        world
            .resource_mut::<super::super::EditorWorkspace>()
            .menu
            .push(popup);
        world
            .resource_mut::<InputFocus>()
            .set(item, FocusCause::Navigated);
        app.update();
        assert!(app.world().entities().contains(popup));
        app.world_mut().trigger(MenuEvent {
            source: item,
            action: MenuAction::FocusRoot,
        });
        app.world_mut().flush();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(button));
        app.update();
        assert!(!app.world().entities().contains(popup));
        assert!(
            app.world()
                .resource::<super::super::EditorWorkspace>()
                .menu
                .is_empty()
        );
        let popup = attach(app.world_mut(), button)?;
        app.world_mut()
            .entity_mut(popup)
            .insert((bevy::ui_widgets::MenuPopup::default(), MenuFocusState::Open));
        app.world_mut()
            .resource_mut::<super::super::EditorWorkspace>()
            .menu
            .push(popup);
        app.world_mut().resource_mut::<InputFocus>().clear();
        app.update();
        assert!(
            !app.world().entities().contains(popup),
            "Blank-space focus loss must dismiss without a Dismiss item"
        );
        Ok(())
    }
    #[test]
    fn menu_is_parented_to_the_invoking_button_and_escapes_panel_clipping() {
        let mut world = World::new();
        let host = world.spawn(Node::default()).id();
        let button = world.spawn((Node::default(), ChildOf(host))).id();
        let menu = attach(&mut world, button).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            world.get::<ChildOf>(menu).map(ChildOf::parent),
            Some(button)
        );
        assert!(world.get::<OverrideClip>(menu).is_some());
        let popover = world
            .get::<Popover>(menu)
            .unwrap_or_else(|| panic!("Native popover is missing"));
        assert_eq!(popover.positions[0].side, PopoverSide::Bottom);
        assert_eq!(popover.positions[1].side, PopoverSide::Top);
        world.despawn(button);
        assert!(!world.entities().contains(menu));
        let count = world.entities().count_spawned();
        assert!(attach(&mut world, button).is_err());
        assert_eq!(world.entities().count_spawned(), count);
    }
}
