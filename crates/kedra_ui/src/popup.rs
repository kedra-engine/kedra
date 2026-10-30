//! Shared native anchored menu placement. Consumers own actions and dismissal.
use bevy::prelude::*;
use bevy::ui_widgets::popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide};
use bevy_flair::{prelude::InlineStyle, style::components::ClassList};
pub mod cascade;
mod entries;
pub use entries::{Entry, from_paths};

/// An owned native anchored surface. Despawning its anchor retires the surface.
#[derive(Component)]
pub struct AnchoredMenu;
const MARGIN: f32 = 4.;
const GAP: f32 = 4.;

/// A pointer-anchored menu and its owned positioning node. Consumers put menu
/// items under `menu` and retire `root` to remove both. The original parent owns
/// the lifetime; native Popover still performs viewport edge flipping.
pub struct PointMenu {
    /// Owned positioning node; despawn this to close the menu.
    pub root: Entity,
    /// Native menu surface for item children.
    pub menu: Entity,
}

/// Mount a context menu at a logical pointer position in the parent's render target.
/// Valid finite layout and a live parent are required; failure allocates nothing.
/// Unlike a toolbar menu, its first placement starts at the pointer's left edge.
pub fn attach_at(
    world: &mut World,
    parent: Entity,
    position: Vec2,
    class: &str,
    scope: &str,
) -> Result<PointMenu, String> {
    let node = world
        .get::<ComputedNode>(parent)
        .ok_or("Context menu parent has no layout")?;
    let transform = world
        .get::<UiGlobalTransform>(parent)
        .ok_or("Context menu parent has no transform")?;
    let scale = node.inverse_scale_factor();
    let local = (transform
        .affine()
        .inverse()
        .transform_point2(position / scale)
        + node.size() / 2.
        - node.border().min_inset)
        * scale;
    if !position.is_finite() || !local.is_finite() || scale <= 0. || node.size().min_element() <= 0.
    {
        return Err("Context menu position is unavailable".into());
    }
    let root = world
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(local.x),
                top: Val::Px(local.y),
                width: Val::Px(1.),
                height: Val::Px(1.),
                ..default()
            },
            bevy::picking::Pickable::IGNORE,
            OverrideClip,
            ChildOf(parent),
        ))
        .id();
    let menu = attach(world, root, class, scope)?;
    world.entity_mut(menu).insert(Popover {
        positions: [PopoverSide::Bottom, PopoverSide::Top]
            .into_iter()
            .flat_map(|side| {
                [PopoverAlign::Start, PopoverAlign::End].map(|align| PopoverPlacement {
                    side,
                    align,
                    gap: 0.,
                })
            })
            .collect(),
        window_margin: MARGIN,
    });
    Ok(PointMenu { root, menu })
}

/// Mount hidden until renderer geometry permits native menu focus. No actions,
/// source authority or external state are installed. A retired anchor allocates nothing.
pub fn attach(
    world: &mut World,
    anchor: Entity,
    class: &str,
    scope: &str,
) -> Result<Entity, String> {
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
            ClassList::new_with_classes([class.to_owned()]),
            AnchoredMenu,
            bevy::input_focus::tab_navigation::TabGroup::modal(),
            Popover {
                positions,
                window_margin: MARGIN,
            },
            OverrideClip,
            InlineStyle::default(),
            Visibility::Hidden,
            GlobalZIndex(200),
            crate::ElementId::new(scope),
            ChildOf(anchor),
        ))
        .id())
}
type PopupLayout = (
    Entity,
    &'static ComputedNode,
    Option<&'static bevy::ui_widgets::MenuPopup>,
    &'static ChildOf,
    &'static ComputedUiRenderTargetInfo,
    &'static mut InlineStyle,
    &'static mut Visibility,
    Option<&'static cascade::Submenu>,
);

/// Installed once by the native authored-UI host; also usable by explicit UI hosts.
pub(crate) fn constrain(
    mut commands: Commands,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
    navigation: bevy::input_focus::tab_navigation::TabNavigation,
    mut menus: Query<PopupLayout, With<AnchoredMenu>>,
    anchors: Query<(&ComputedNode, &UiGlobalTransform), Without<AnchoredMenu>>,
) {
    for (entity, computed, native, parent, target, mut style, mut visibility, submenu) in &mut menus
    {
        let Ok((node, transform)) = anchors.get(parent.parent()) else {
            continue;
        };
        let viewport = target.logical_size();
        if node.size().min_element() <= 0. || viewport.min_element() <= 2. * MARGIN {
            continue;
        }
        let [x0, y0, x1, y1] = crate::logical_rect(node, transform);
        let maximum = if submenu.is_some() {
            Vec2::new(
                (viewport.x - x1).max(x0).max(0.) - MARGIN,
                viewport.y - 2. * MARGIN,
            )
        } else {
            limits(
                Rect::from_corners(Vec2::new(x0, y0), Vec2::new(x1, y1)),
                viewport,
            )
        };
        let mut next = style.clone();
        next.set("max-width", format!("{}px", maximum.x));
        next.set("max-height", format!("{}px", maximum.y));
        if *style != next {
            *style = next;
        }
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
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
    fn placement_constraints_keep_native_candidates_inside_small_and_large_hosts() {
        for size in [Vec2::new(1200., 800.), Vec2::new(240., 192.)] {
            for x in [4., size.x / 2. - 13., size.x - 30.] {
                for y in [34., size.y / 2., size.y - 30.] {
                    let anchor = Rect::from_corners(Vec2::new(x, y), Vec2::new(x + 26., y + 26.));
                    let max = limits(anchor, size);
                    let w = max.x.min(224.);
                    let h = max.y.min(240.);
                    assert!(max.x > 0. && max.y > 0.);
                    assert!(
                        anchor.min.y - GAP - h >= MARGIN
                            || anchor.max.y + GAP + h <= size.y - MARGIN
                    );
                    assert!(
                        anchor.max.x - w >= MARGIN
                            || anchor.min.x + w <= size.x - MARGIN
                            || (anchor.center().x - w / 2. >= MARGIN
                                && anchor.center().x + w / 2. <= size.x - MARGIN)
                    );
                }
            }
        }
    }
}
