//! Opt-in cascading menus over Bevy focus, menu items and native popover placement.
//! Hosts supply commands and populate requested branches; this module owns only
//! navigation, delayed hover and popup lifetime. No source/application state lives here.
use bevy::ui_widgets::popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide};
use bevy::{
    input::keyboard::KeyboardInput,
    input_focus::{FocusCause, FocusedInput, InputFocus},
    prelude::*,
    ui_widgets::{Activate, MenuAction, MenuEvent},
};

/// Marks a menu whose whole descendant chain uses shared cascading dismissal.
#[derive(Component)]
pub struct Cascade;
/// A submenu's invoking item; both are retired with their parent menu.
#[derive(Component)]
pub struct Submenu(pub Entity);
/// An item that opens a submenu when activated. Hosts handle ordinary `Activate`
/// to populate it with `attach`. A branch must not also be a native `MenuItem`.
#[derive(Component)]
pub struct Branch;
/// Emitted before a cascade root is retired so hosts can release their popup
/// bookkeeping and any owned pointer-positioning anchor. No command is executed.
#[derive(EntityEvent)]
pub struct Closed {
    /// Root menu being closed.
    pub entity: Entity,
}
#[derive(Component)]
struct Hover(std::time::Instant);

/// Shared interaction adapter. Install once in native UI consumers.
pub struct CascadePlugin;
impl Plugin for CascadePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(entered)
            .add_observer(left)
            .add_observer(key)
            .add_observer(menu_event)
            .add_systems(Update, hover);
    }
}

#[cfg(test)]
#[path = "cascade_tests.rs"]
mod tests;
/// Configure an authored button as an accessible branch rather than a leaf.
/// Existing text, accessible name, tab index and host action remain authoritative.
pub fn branch(world: &mut World, item: Entity) {
    let menu = ancestors(world, item).find(|e| world.get::<super::AnchoredMenu>(*e).is_some());
    if let Some(menu) = menu {
        world.entity_mut(menu).insert(Cascade);
    }
    world
        .entity_mut(item)
        .remove::<bevy::ui_widgets::MenuItem>()
        .insert((Branch, bevy::ui_widgets::Button));
}
/// Mount a submenu to the right of its invoking branch, flipping left at the
/// window edge and bottom-aligning when necessary. Reopening retains its live menu. A foreign or
/// disabled branch fails without allocation. Hosts populate returned empty menus.
pub fn attach(world: &mut World, item: Entity, class: &str, scope: &str) -> Result<Entity, String> {
    if world.get::<Branch>(item).is_none()
        || world.get::<bevy::ui::InteractionDisabled>(item).is_some()
    {
        return Err("Submenu branch is unavailable".into());
    }
    let parent = ancestors(world, item)
        .find(|e| world.get::<super::AnchoredMenu>(*e).is_some())
        .ok_or("Submenu has no parent menu")?;
    let existing = world
        .query::<(Entity, &Submenu)>()
        .iter(world)
        .find_map(|(e, s)| (s.0 == item).then_some(e));
    if let Some(existing) = existing {
        return Ok(existing);
    }
    let siblings: Vec<_> = world
        .query::<(Entity, &Submenu)>()
        .iter(world)
        .filter(|(_, s)| {
            ancestors(world, s.0).find(|e| world.get::<super::AnchoredMenu>(*e).is_some())
                == Some(parent)
        })
        .map(|(e, _)| e)
        .collect();
    for sibling in siblings {
        close(world, sibling, false);
    }
    let menu = super::attach(world, item, class, scope)?;
    let depth = ancestors(world, item)
        .filter(|e| world.get::<Submenu>(*e).is_some())
        .count() as i32;
    world.entity_mut(menu).insert((
        Cascade,
        Submenu(item),
        GlobalZIndex(201 + depth),
        Popover {
            positions: [
                (PopoverSide::Right, PopoverAlign::Start),
                (PopoverSide::Right, PopoverAlign::End),
                (PopoverSide::Left, PopoverAlign::Start),
                (PopoverSide::Left, PopoverAlign::End),
            ]
            .into_iter()
            .map(|(side, align)| PopoverPlacement {
                side,
                align,
                gap: 0.,
            })
            .collect(),
            window_margin: 4.,
        },
    ));
    Ok(menu)
}
fn ancestors(world: &World, entity: Entity) -> impl Iterator<Item = Entity> + '_ {
    std::iter::successors(Some(entity), |e| {
        world.get::<ChildOf>(*e).map(ChildOf::parent)
    })
}
fn root(world: &World, entity: Entity) -> Option<Entity> {
    ancestors(world, entity)
        .filter(|e| world.get::<Cascade>(*e).is_some())
        .last()
}
fn close(world: &mut World, menu: Entity, restore: bool) {
    if world.get::<Submenu>(menu).is_none() {
        world.trigger(Closed { entity: menu });
    }
    let anchor = world.get::<ChildOf>(menu).map(ChildOf::parent);
    if restore
        && let Some(anchor) = anchor
        && let Some(mut focus) = world.get_resource_mut::<InputFocus>()
    {
        focus.set(anchor, FocusCause::Navigated);
    }
    if let Ok(entity) = world.get_entity_mut(menu) {
        entity.despawn();
    }
}
fn entered(
    event: On<bevy::picking::events::PointerEnter>,
    branches: Query<(), With<Branch>>,
    mut commands: Commands,
) {
    if branches.contains(event.event_target()) {
        commands
            .entity(event.event_target())
            .insert(Hover(std::time::Instant::now()));
    }
}
fn left(
    event: On<bevy::picking::events::PointerLeave>,
    branches: Query<(), With<Branch>>,
    mut commands: Commands,
) {
    if branches.contains(event.event_target()) {
        commands.entity(event.event_target()).remove::<Hover>();
    }
}
fn hover(
    mut commands: Commands,
    branches: Query<(Entity, &Hover), Without<bevy::ui::InteractionDisabled>>,
    children: Query<&Submenu>,
) {
    for (entity, timer) in &branches {
        if timer.0.elapsed() >= std::time::Duration::from_millis(180) {
            commands.entity(entity).remove::<Hover>();
            if !children.iter().any(|s| s.0 == entity) {
                commands.trigger(Activate { entity });
            }
        }
    }
}
fn key(
    mut event: On<FocusedInput<KeyboardInput>>,
    branches: Query<(), (With<Branch>, Without<bevy::ui::InteractionDisabled>)>,
    submenus: Query<&Submenu>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    if event.input.state != bevy::input::ButtonState::Pressed || event.input.repeat {
        return;
    }
    let entity = event.focused_entity;
    if branches.contains(entity) && event.input.key_code == KeyCode::ArrowRight {
        event.propagate(false);
        commands.trigger(Activate { entity });
    } else if event.input.key_code == KeyCode::ArrowLeft
        && let Some(menu) = std::iter::once(entity)
            .chain(parents.iter_ancestors(entity))
            .find(|e| submenus.contains(*e))
    {
        event.propagate(false);
        commands.queue(move |world: &mut World| close(world, menu, true));
    }
}
fn menu_event(
    mut event: On<MenuEvent>,
    menus: Query<(), With<Cascade>>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    let nearest = std::iter::once(event.source)
        .chain(parents.iter_ancestors(event.source))
        .find(|e| menus.contains(*e));
    let Some(menu) = nearest else {
        return;
    };
    event.propagate(false);
    let source = event.source;
    let action = event.action;
    commands.queue(move |world: &mut World| {
        let Some(root) = root(world, menu) else {
            return;
        };
        match action {
            MenuAction::FocusRoot => {
                if let Some(anchor) = world.get::<ChildOf>(root).map(ChildOf::parent)
                    && let Some(mut focus) = world.get_resource_mut::<InputFocus>()
                {
                    focus.set(anchor, FocusCause::Navigated);
                }
            }
            MenuAction::CloseAll => {
                let focused = world.get_resource::<InputFocus>().and_then(InputFocus::get);
                let in_root = focused.is_some_and(|e| ancestors(world, e).any(|e| e == root));
                if menu != root && source == menu && in_root {
                    let restore = focused.is_some_and(|e| ancestors(world, e).any(|e| e == menu));
                    close(world, menu, restore);
                } else {
                    close(world, root, in_root);
                }
            }
            _ => {}
        }
    });
}
