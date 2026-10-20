//! Dock menus use Bevy's anchor/flip implementation, not host-relative offsets.
use bevy::prelude::*;
use bevy::ui_widgets::popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide};
use bevy_flair::{prelude::InlineStyle, style::components::ClassList};

#[derive(Component)]
pub(super) struct DockPopup;
const MARGIN: f32 = 4.;
const GAP: f32 = 4.;

pub(super) fn attach(world: &mut World, anchor: Entity) -> Result<Entity, String> {
    if world.get::<Node>(anchor).is_none() {
        return Err("Menu button was retired".into());
    }
    let positions = [PopoverAlign::End, PopoverAlign::Start, PopoverAlign::Center]
        .into_iter()
        .flat_map(|align| {
            [PopoverSide::Bottom, PopoverSide::Top].map(|side| PopoverPlacement {
                side,
                align,
                gap: GAP,
            })
        })
        .collect();
    Ok(world
        .spawn((
            Node::default(),
            ClassList::new_with_classes(["dock-menu"]),
            DockPopup,
            bevy::input_focus::tab_navigation::TabGroup::modal(),
            Popover {
                positions,
                window_margin: MARGIN,
            },
            OverrideClip,
            InlineStyle::default(),
            Visibility::Hidden,
            GlobalZIndex(200),
            kedra_ui::ElementId::new("dock-menu"),
            ChildOf(anchor),
        ))
        .id())
}

// Bevy owns outside-click/focus-loss detection and keyboard traversal. Kedra
// only retires its popup stack and restores the invoking control's focus.
pub(super) fn menu_event(
    mut event: On<bevy::ui_widgets::MenuEvent>,
    parents: Query<&ChildOf>,
    popups: Query<(), With<DockPopup>>,
    mut commands: Commands,
) {
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

// Constrain size, then let native Popover choose placement and update transforms.
// This runs each frame so an open menu follows window/anchor size changes and DPR.
type PopupLayout = (
    Entity,
    &'static ComputedNode,
    Option<&'static bevy::ui_widgets::MenuPopup>,
    &'static ChildOf,
    &'static ComputedUiRenderTargetInfo,
    &'static mut InlineStyle,
    &'static mut Visibility,
);
pub(super) fn constrain(
    mut commands: Commands,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
    navigation: bevy::input_focus::tab_navigation::TabNavigation,
    mut menus: Query<PopupLayout, With<DockPopup>>,
    anchors: Query<(&ComputedNode, &UiGlobalTransform), Without<DockPopup>>,
) {
    for (entity, computed, native, parent, target, mut style, mut visibility) in &mut menus {
        let Ok((node, transform)) = anchors.get(parent.parent()) else {
            continue;
        };
        let viewport = target.logical_size();
        if node.size().min_element() <= 0. || viewport.min_element() <= 2. * MARGIN {
            continue;
        }
        let [x0, y0, x1, y1] = kedra_ui::logical_rect(node, transform);
        let maximum = limits(
            Rect::from_corners(Vec2::new(x0, y0), Vec2::new(x1, y1)),
            viewport,
        );
        let mut next = style.clone();
        next.set("max-width", format!("{}px", maximum.x));
        next.set("max-height", format!("{}px", maximum.y));
        if *style != next {
            *style = next;
        }
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
        // Native Opening also participates in focus-loss dismissal. Enable it
        // only after layout, when a child can actually acquire keyboard focus.
        if native.is_none()
            && computed.size().min_element() > 0.
            && let Ok(item) =
                navigation.initialize(entity, bevy::input_focus::tab_navigation::NavAction::First)
        {
            focus.set(item, bevy::input_focus::FocusCause::Navigated);
            commands.entity(entity).insert((
                bevy::ui_widgets::MenuPopup::default(),
                bevy::ui_widgets::MenuFocusState::Open,
            ));
        }
    }
}
fn limits(anchor: Rect, viewport: Vec2) -> Vec2 {
    let start = viewport.x - MARGIN - anchor.min.x;
    let end = anchor.max.x - MARGIN;
    let center = 2. * (anchor.center().x - MARGIN).min(viewport.x - MARGIN - anchor.center().x);
    let above = anchor.min.y - MARGIN - GAP;
    let below = viewport.y - MARGIN - anchor.max.y - GAP;
    Vec2::new(
        start
            .max(end)
            .max(center)
            .min(viewport.x - 2. * MARGIN)
            .max(0.),
        above.max(below).max(0.),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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
    #[test]
    fn constraints_allow_native_placement_at_both_edges_and_in_small_floating_hosts() {
        for size in [Vec2::new(1200., 800.), Vec2::new(240., 192.)] {
            for x in [4., size.x / 2. - 13., size.x - 30.] {
                for y in [34., size.y / 2., size.y - 30.] {
                    let button = Rect::from_corners(Vec2::new(x, y), Vec2::new(x + 26., y + 26.));
                    let max = limits(button, size);
                    assert!(max.x > 0. && max.y > 0.);
                    let w = max.x.min(224.);
                    let h = max.y.min(240.);
                    assert!(
                        button.min.y - GAP - h >= MARGIN
                            || button.max.y + GAP + h <= size.y - MARGIN
                    );
                    assert!(
                        button.max.x - w >= MARGIN
                            || button.min.x + w <= size.x - MARGIN
                            || (button.center().x - w / 2. >= MARGIN
                                && button.center().x + w / 2. <= size.x - MARGIN)
                    );
                }
            }
        }
    }
}
