//! Grip-owned keyboard/drag gestures over retained item identities. Only a completed
//! in-array drop proposes a source change; hover, Escape and outside drop never do.
use super::*;
use bevy::ecs::entity::ContainsEntity;
use bevy::{
    input::keyboard::{Key, KeyboardInput},
    input_focus::FocusedInput,
    picking::events::{Pointer, PointerDrag, PointerDragEnd, PointerDragStart},
};

#[derive(Component, Clone, Copy)]
pub(super) struct Grip {
    pub owner: Entity,
    pub id: u64,
    pub row: Entity,
}
#[derive(Component, Clone, Copy)]
pub(super) struct Row {
    pub owner: Entity,
    pub id: u64,
}
pub(super) struct Drag {
    revision: u64,
    binding: Arc<crate::controls::ControlBinding>,
    id: u64,
    pointer: Pointer,
}
#[derive(Clone, Copy)]
pub(super) enum Focus {
    Item(u64),
    Add,
}
pub(super) fn install(app: &mut App) {
    app.add_observer(start)
        .add_observer(motion)
        .add_observer(end)
        .add_observer(keyboard);
    app.add_systems(Update, drag_tick);
    app.add_systems(Last, focus);
}
fn focus(world: &mut World) {
    if !world.contains_resource::<bevy::input_focus::InputFocus>() {
        return;
    }
    let requests = world
        .query::<(Entity, &State)>()
        .iter(world)
        .filter_map(|(root, state)| {
            state
                .focus
                .filter(|_| state.focus_ready)
                .map(|focus| (root, focus))
        })
        .collect::<Vec<_>>();
    for (root, request) in requests {
        let target = match request {
            Focus::Add => world
                .query::<(Entity, &Action)>()
                .iter(world)
                .find(|(_, action)| action.owner == root && matches!(action.kind, Operation::Add))
                .map(|(entity, _)| entity),
            Focus::Item(id) => {
                let row = world
                    .query::<(Entity, &Row)>()
                    .iter(world)
                    .find(|(_, row)| row.owner == root && row.id == id)
                    .map(|(entity, _)| entity);
                row.and_then(|row| {
                    world
                        .iter_entities()
                        .find(|entity| {
                            entity.contains::<TextField>()
                                && writable(world, entity.id())
                                && within(world, entity.id(), row)
                        })
                        .map(|entity| entity.id())
                })
            }
        };
        let Some(target) = target else {
            continue;
        };
        world
            .resource_mut::<bevy::input_focus::InputFocus>()
            .set(target, bevy::input_focus::FocusCause::Navigated);
        if let Some(mut text) = world.get_mut::<bevy::text::EditableText>(target) {
            text.queue_edit(bevy::text::TextEdit::SelectAll);
        }
        if let Some(mut state) = world.get_mut::<State>(root) {
            state.focus = None;
        }
    }
}
fn start(event: On<PointerDragStart>, grips: Query<&Grip>, mut commands: Commands) {
    if event.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    let Ok(grip) = grips.get(event.entity).copied() else {
        return;
    };
    let pointer = event.pointer.clone();
    let entity = event.entity;
    commands.queue(move |world: &mut World| {
        if !writable(world, entity) || !same_window(world, entity, &pointer) {
            return;
        }
        let Some(binding) = world
            .get::<Control>(grip.owner)
            .map(|control| control.0.clone())
        else {
            return;
        };
        let Some(state) = world.get::<State>(grip.owner) else {
            return;
        };
        // Native pointer focus loss may still have queued a selection edit here.
        // The completed drop is processed after native editing and rechecks every
        // actual draft; pending caret work alone must not discard this gesture.
        if state.pending.is_some() || state.queued > 0 {
            return;
        }
        let revision = state.revision;
        if let Some(mut state) = world.get_mut::<State>(grip.owner) {
            state.selected = Some(grip.id);
            state.drag = Some(Drag {
                revision,
                binding,
                id: grip.id,
                pointer,
            });
        }
        world.trigger(Changed);
    });
}
fn motion(event: On<PointerDrag>, grips: Query<&Grip>, mut commands: Commands) {
    let Ok(grip) = grips.get(event.entity).copied() else {
        return;
    };
    let pointer = event.pointer.clone();
    let distance = event.distance;
    commands.queue(move |world: &mut World| {
        if distance.length_squared() < 16. {
            return;
        }
        let Some(state) = world.get::<State>(grip.owner) else {
            return;
        };
        let Some(drag) = &state.drag else {
            return;
        };
        if drag.pointer.id != pointer.id
            || drag.pointer.target != pointer.target
            || drag.revision != state.revision
        {
            return;
        }
        let to = destination(world, grip.owner, grip.id, pointer.position);
        mark(world, grip.owner, to);
        autoscroll(world, grip.row, pointer.position);
        if let Some(mut state) = world.get_mut::<State>(grip.owner)
            && let Some(drag) = &mut state.drag
        {
            drag.pointer.position = pointer.position;
        }
    });
}
fn end(event: On<PointerDragEnd>, grips: Query<&Grip>, mut commands: Commands) {
    let Ok(grip) = grips.get(event.entity).copied() else {
        return;
    };
    let pointer = event.pointer.clone();
    let distance = event.distance;
    let entity = event.entity;
    commands.queue(move |world: &mut World| {
        let Some(drag) = world
            .get_mut::<State>(grip.owner)
            .and_then(|mut state| state.drag.take())
        else {
            return;
        };
        mark(world, grip.owner, None);
        let Some(state) = world.get::<State>(grip.owner) else {
            return;
        };
        if distance.length_squared() >= 16.
            && drag.pointer.id == pointer.id
            && drag.pointer.target == pointer.target
            && drag.revision == state.revision
            && writable(world, entity)
            && same_window(world, entity, &pointer)
        {
            let from = state.ids.iter().position(|id| *id == drag.id);
            let to = destination(world, grip.owner, grip.id, pointer.position);
            if let (Some(from), Some(to)) = (from, to) {
                input::reorder(
                    world,
                    entity,
                    grip.owner,
                    drag.revision,
                    drag.binding,
                    from,
                    to,
                );
            }
        }
        world.trigger(Changed);
    });
}
fn same_window(world: &World, entity: Entity, pointer: &Pointer) -> bool {
    matches!(pointer.target, bevy::camera::NormalizedRenderTarget::Window(window)
        if kedra_ui::native_input::owner(world, entity) == Some(window.entity()))
}
fn bounds(world: &World, entity: Entity) -> Option<Rect> {
    let rect = kedra_ui::docking::rect(world, entity)?;
    let scale = world.get_resource::<UiScale>().map_or(1., |scale| scale.0);
    let camera = world
        .get::<ComputedUiTargetCamera>(entity)
        .and_then(|target| target.get())
        .and_then(|camera| world.get::<Camera>(camera));
    let origin = camera
        .and_then(Camera::logical_viewport_rect)
        .map_or(Vec2::ZERO, |rect| rect.min);
    Some(Rect::from_corners(
        rect.min * scale + origin,
        rect.max * scale + origin,
    ))
}
fn destination(world: &World, owner: Entity, id: u64, point: Vec2) -> Option<usize> {
    let state = world.get::<State>(owner)?;
    let area = visible_bounds(world, state.area)?;
    if !area.contains(point) {
        return None;
    }
    let mut rows = world
        .iter_entities()
        .filter_map(|entity| {
            let row = entity.get::<Row>()?;
            if row.owner != owner {
                return None;
            }
            let index = state.ids.iter().position(|id| *id == row.id)?;
            Some((index, bounds(world, entity.id())?))
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|(index, _)| *index);
    let insertion = rows
        .iter()
        .find(|(_, rect)| point.y < rect.center().y)
        .map_or(state.ids.len(), |(index, _)| *index);
    let from = state.ids.iter().position(|candidate| *candidate == id)?;
    Some(
        if insertion > from {
            insertion - 1
        } else {
            insertion
        }
        .min(state.ids.len().saturating_sub(1)),
    )
}
fn mark(world: &mut World, owner: Entity, destination: Option<usize>) {
    let selected = world
        .get::<State>(owner)
        .and_then(|state| destination.and_then(|index| state.ids.get(index).copied()));
    let rows = world
        .query::<(Entity, &Row)>()
        .iter(world)
        .filter(|(_, row)| row.owner == owner)
        .map(|(entity, row)| (entity, Some(row.id) == selected))
        .collect::<Vec<_>>();
    for (entity, active) in rows {
        kedra_ui::docking::class(world, entity, "drop-target", active);
    }
}
fn autoscroll(world: &mut World, mut entity: Entity, point: Vec2) {
    loop {
        if let Some(rect) = bounds(world, entity)
            && world
                .get::<Node>(entity)
                .is_some_and(|node| node.overflow.y == OverflowAxis::Scroll)
            && world.get::<ScrollPosition>(entity).is_some()
        {
            let speed = if point.y < rect.min.y + 20. {
                -10.
            } else if point.y > rect.max.y - 20. {
                10.
            } else {
                0.
            };
            let max = world.get::<ComputedNode>(entity).map_or(0., |node| {
                ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.)
            });
            if let Some(mut scroll) = world.get_mut::<ScrollPosition>(entity) {
                scroll.y = (scroll.y + speed).clamp(0., max);
            }
            return;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return;
        };
        entity = parent.parent();
    }
}
fn visible_bounds(world: &World, entity: Entity) -> Option<Rect> {
    let mut rect = bounds(world, entity)?;
    let mut ancestor = world.get::<ChildOf>(entity).map(ChildOf::parent);
    while let Some(parent) = ancestor {
        if let Some(node) = world.get::<Node>(parent) {
            if node.display == Display::None {
                return None;
            }
            if let Some(clip) = bounds(world, parent) {
                if node.overflow.x != OverflowAxis::Visible {
                    rect.min.x = rect.min.x.max(clip.min.x);
                    rect.max.x = rect.max.x.min(clip.max.x);
                }
                if node.overflow.y != OverflowAxis::Visible {
                    rect.min.y = rect.min.y.max(clip.min.y);
                    rect.max.y = rect.max.y.min(clip.max.y);
                }
            }
        }
        ancestor = world.get::<ChildOf>(parent).map(ChildOf::parent);
    }
    (rect.min.x <= rect.max.x && rect.min.y <= rect.max.y).then_some(rect)
}
fn drag_tick(world: &mut World) {
    let drags = world
        .query::<(Entity, &State)>()
        .iter(world)
        .filter_map(|(owner, state)| {
            state.drag.as_ref().map(|drag| {
                let current = drag.revision == state.revision
                    && world
                        .get::<Control>(owner)
                        .is_some_and(|control| Arc::ptr_eq(&control.0, &drag.binding));
                (owner, drag.id, drag.pointer.clone(), current)
            })
        })
        .collect::<Vec<_>>();
    for (owner, id, pointer, current) in drags {
        let grip = world
            .query::<(Entity, &Grip)>()
            .iter(world)
            .find(|(_, grip)| grip.owner == owner && grip.id == id)
            .map(|(entity, grip)| (entity, grip.row));
        let active = current
            && grip.is_some_and(|(entity, row)| {
                writable(world, owner)
                    && writable(world, entity)
                    && same_window(world, entity, &pointer)
                    && visible_bounds(world, row).is_some()
            });
        if !active {
            if let Some(mut state) = world.get_mut::<State>(owner) {
                state.drag = None;
            }
            mark(world, owner, None);
            world.trigger(Changed);
            continue;
        }
        if let Some((_, row)) = grip {
            autoscroll(world, row, pointer.position);
        }
        mark(
            world,
            owner,
            destination(world, owner, id, pointer.position),
        );
    }
}
fn keyboard(
    mut event: On<FocusedInput<KeyboardInput>>,
    grips: Query<&Grip>,
    keys: Res<ButtonInput<Key>>,
    mut commands: Commands,
) {
    let Ok(grip) = grips.get(event.focused_entity).copied() else {
        return;
    };
    let key = event.input.logical_key.clone();
    if key != Key::Escape
        && !(keys.pressed(Key::Alt) && matches!(key, Key::ArrowUp | Key::ArrowDown))
    {
        return;
    }
    event.propagate(false);
    if !event.input.state.is_pressed() || event.input.repeat {
        return;
    }
    let entity = event.focused_entity;
    commands.queue(move |world: &mut World| {
        if key == Key::Escape {
            if let Some(mut state) = world.get_mut::<State>(grip.owner) {
                state.drag = None;
            }
            mark(world, grip.owner, None);
            world.trigger(Changed);
            return;
        }
        if !writable(world, entity) {
            return;
        }
        let Some(state) = world.get::<State>(grip.owner) else {
            return;
        };
        let Some(from) = state.ids.iter().position(|id| *id == grip.id) else {
            return;
        };
        let to = if key == Key::ArrowUp {
            from.saturating_sub(1)
        } else {
            (from + 1).min(state.ids.len().saturating_sub(1))
        };
        let revision = state.revision;
        let Some(binding) = world
            .get::<Control>(grip.owner)
            .map(|control| control.0.clone())
        else {
            return;
        };
        input::reorder(world, entity, grip.owner, revision, binding, from, to);
    });
}
