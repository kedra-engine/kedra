//! One native input arbiter; semantic UI regions beat OS drag and frame resize.
use super::NativeChromeState;
use bevy::{
    ecs::message::MessageCursor, input::mouse::MouseButtonInput, math::CompassOctant, prelude::*,
};
use kedra_ui::window_chrome::{RegionRole, hit};
use std::collections::BTreeMap;

pub(super) fn native_state(
    _main: bevy::ecs::system::NonSendMarker,
    mut windows: Query<(Entity, &mut NativeChromeState)>,
) {
    bevy::winit::WINIT_WINDOWS.with_borrow(|native| {
        for (entity, mut state) in &mut windows {
            if let Some(window) = native.get_window(entity) {
                let next = NativeChromeState {
                    ready: true,
                    focused: window.has_focus(),
                    maximized: window.is_maximized(),
                    fullscreen: window.fullscreen().is_some(),
                };
                if state.ready != next.ready
                    || state.focused != next.focused
                    || state.maximized != next.maximized
                    || state.fullscreen != next.fullscreen
                {
                    *state = next;
                }
            }
        }
    });
}
#[derive(Debug, PartialEq)]
pub(super) enum PointerAction {
    Move,
    Resize(CompassOctant),
}

pub(super) fn pointer_action(
    window: &Window,
    state: &NativeChromeState,
    point: Vec2,
    role: Option<RegionRole>,
    blocked: bool,
) -> Option<PointerAction> {
    if blocked
        || state.fullscreen
        || !matches!(window.mode, bevy::window::WindowMode::Windowed)
        || matches!(role, Some(RegionRole::Client | RegionRole::NativeControls))
    {
        return None;
    }
    if window.resizable
        && !state.maximized
        && let Some(edge) = resize_edge(point, window.size())
    {
        return Some(PointerAction::Resize(edge));
    }
    (role == Some(RegionRole::Drag)).then_some(PointerAction::Move)
}
pub(super) fn resize_edge(point: Vec2, size: Vec2) -> Option<CompassOctant> {
    if !point.is_finite()
        || !size.is_finite()
        || !Rect::from_corners(Vec2::ZERO, size).contains(point)
    {
        return None;
    }
    let left = point.x < 6.;
    let right = point.x >= size.x - 6.;
    let top = point.y < 6.;
    let bottom = point.y >= size.y - 6.;
    match (
        point.x < 12.,
        point.x >= size.x - 12.,
        point.y < 12.,
        point.y >= size.y - 12.,
    ) {
        (true, _, true, _) if left || top => Some(CompassOctant::NorthWest),
        (_, true, true, _) if right || top => Some(CompassOctant::NorthEast),
        (true, _, _, true) if left || bottom => Some(CompassOctant::SouthWest),
        (_, true, _, true) if right || bottom => Some(CompassOctant::SouthEast),
        _ if left => Some(CompassOctant::West),
        _ if right => Some(CompassOctant::East),
        _ if top => Some(CompassOctant::North),
        _ if bottom => Some(CompassOctant::South),
        _ => None,
    }
}

#[derive(Default)]
pub(super) struct PointerSamples {
    moves: MessageCursor<bevy::window::CursorMoved>,
    positions: BTreeMap<Entity, Vec2>,
}
pub(super) fn input(
    world: &mut World,
    mut cursor: Local<MessageCursor<MouseButtonInput>>,
    mut samples: Local<PointerSamples>,
) {
    if let Some(messages) = world.get_resource::<Messages<bevy::window::CursorMoved>>() {
        let events: Vec<_> = samples.moves.read(messages).cloned().collect();
        for event in events {
            samples.positions.insert(event.window, event.position);
        }
        samples
            .positions
            .retain(|window, _| world.get::<Window>(*window).is_some());
    }
    let Some(messages) = world.get_resource::<Messages<MouseButtonInput>>() else {
        return;
    };
    let events: Vec<_> = cursor
        .read(messages)
        .filter(|e| e.button == MouseButton::Left && e.state.is_pressed())
        .copied()
        .collect();
    for event in events {
        let Some(window) = world.get::<Window>(event.window) else {
            continue;
        };
        let Some(state) = world.get::<NativeChromeState>(event.window) else {
            continue;
        };
        if !window.focused {
            continue;
        }
        let Some(point) = samples
            .positions
            .get(&event.window)
            .copied()
            .or_else(|| window.cursor_position())
        else {
            continue;
        };
        let role = hit(world, event.window, point);
        // Shared hit testing scopes modal and client regions to this window.
        let action = pointer_action(window, state, point, role, false);
        let Some(mut window) = world.get_mut::<Window>(event.window) else {
            continue;
        };
        match action {
            Some(PointerAction::Move) => window.start_drag_move(),
            // AppKit owns its native edge tracking; Winit cannot drag_resize there.
            Some(PointerAction::Resize(edge)) if !cfg!(target_os = "macos") => {
                window.start_drag_resize(edge)
            }
            _ => {}
        }
    }
}
