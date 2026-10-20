//! Opt-in bounded input evidence for the isolated fixture; no product input policy.
use super::*;
use bevy::{ecs::message::MessageCursor, window::WindowEvent};
use std::collections::VecDeque;

const CAPACITY: usize = 128;
const EVENT_CAPACITY: usize = 32;

#[derive(Resource, Default)]
struct Trace {
    cursor: MessageCursor<WindowEvent>,
    frames: VecDeque<Value>,
    last_gesture: String,
}

pub(super) fn install(app: &mut App) {
    if std::env::var("KEDRA_DOCK_TRACE").is_ok_and(|value| value == "1") {
        app.init_resource::<Trace>().add_systems(PostUpdate, record);
        app.register_remote(
            "dock.trace",
            "Read bounded native fixture input diagnostics",
            json!({}),
            read,
        );
    }
}

fn read(_: In<Option<Value>>, world: &mut World) -> bevy_remote::BrpResult {
    Ok(json!({"capacity":CAPACITY,"frames":world.resource::<Trace>().frames}))
}

fn record(world: &mut World) {
    let mut trace = world.remove_resource::<Trace>().unwrap_or_default();
    let (event_count, events) = trace.cursor.read(world.resource::<Messages<WindowEvent>>())
        .filter_map(|event| match event {
            WindowEvent::CursorMoved(e) => Some(json!({"kind":"cursor","window":e.window.to_bits(),"position":e.position.to_array(),"delta":e.delta.map(|v|v.to_array())})),
            WindowEvent::MouseButtonInput(e) => Some(json!({"kind":"button","window":e.window.to_bits(),"button":format!("{:?}",e.button),"state":format!("{:?}",e.state)})),
            WindowEvent::KeyboardInput(e) => Some(json!({"kind":"key","window":e.window.to_bits(),"code":format!("{:?}",e.key_code),"state":format!("{:?}",e.state)})),
            WindowEvent::WindowFocused(e) => Some(json!({"kind":"focus","window":e.window.to_bits(),"focused":e.focused})),
            _ => None,
        }).fold((0usize, Vec::new()), |(count, mut events), event| {
            if count < EVENT_CAPACITY {
                events.push(event);
            }
            (count + 1, events)
        });
    let state = world.get_resource::<EditorWorkspace>();
    let gesture = state.and_then(|s| s.gesture.as_ref()).map_or_else(
        || "none".into(),
        |g| match g {
            input::Gesture::Pending { selection, .. } => format!("pending {selection:?}"),
            input::Gesture::Drag {
                selection,
                destination,
                ..
            } => format!("drag {selection:?} -> {destination:?}"),
            input::Gesture::Resize { split, .. } => format!("resize {split:?}"),
        },
    );
    if !events.is_empty() || gesture != trace.last_gesture {
        let sample = world.get_resource::<input::CursorSample>();
        let mouse = world.resource::<ButtonInput<MouseButton>>();
        let keys = world.resource::<ButtonInput<KeyCode>>();
        let frame = json!({
            "frame":world.resource::<bevy::diagnostic::FrameCount>().0,
            "dropped_events":event_count.saturating_sub(events.len()),
            "events":events,"gesture":gesture,"revision":state.map(|s|s.layout.revision()),
            "pointer_window":sample.and_then(|s|s.window).map(Entity::to_bits),
            "pointer":sample.map(|s|s.position.to_array()),
            "mouse_held":mouse.pressed(MouseButton::Left),
            "mouse_pressed":mouse.just_pressed(MouseButton::Left),
            "mouse_released":mouse.just_released(MouseButton::Left),
            "shift":keys.any_pressed([KeyCode::ShiftLeft,KeyCode::ShiftRight]),
            "windows":state.map(|s|s.surfaces.values().filter_map(|surface|world.get::<Window>(surface.window).map(|w|json!({
                "id":surface.window.to_bits(),"focused":w.focused,
                "backend_focused":bevy::winit::WINIT_WINDOWS.with_borrow(|native|native.get_window(surface.window).map(|window|window.has_focus())),
            }))).collect::<Vec<_>>()),
        });
        if trace.frames.len() == CAPACITY {
            trace.frames.pop_front();
        }
        trace.frames.push_back(frame);
        trace.last_gesture = gesture;
    }
    world.insert_resource(trace);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_is_bounded_drains_truncated_batches_and_does_not_mutate_input()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut world = World::new();
        world.init_resource::<Trace>();
        world.init_resource::<Messages<WindowEvent>>();
        world.init_resource::<bevy::diagnostic::FrameCount>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        let window = world.spawn_empty().id();
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        for frame in 0..CAPACITY + 2 {
            world.resource_mut::<bevy::diagnostic::FrameCount>().0 = frame as u32;
            world.write_message(WindowEvent::CursorMoved(bevy::window::CursorMoved {
                window,
                position: Vec2::splat(frame as f32),
                delta: None,
            }));
            record(&mut world);
        }
        let trace = world.resource::<Trace>();
        assert_eq!(trace.frames.len(), CAPACITY);
        assert_eq!(trace.frames[0]["frame"], 2);
        for _ in 0..EVENT_CAPACITY + 5 {
            world.write_message(WindowEvent::CursorMoved(bevy::window::CursorMoved {
                window,
                position: Vec2::ZERO,
                delta: None,
            }));
        }
        record(&mut world);
        let last = world
            .resource::<Trace>()
            .frames
            .back()
            .ok_or("last trace frame")?;
        assert_eq!(
            last["events"].as_array().ok_or("events")?.len(),
            EVENT_CAPACITY
        );
        assert_eq!(last["dropped_events"], 5);
        let before = read(In(None), &mut world).map_err(|e| format!("{e:?}"))?;
        record(&mut world);
        assert_eq!(
            before,
            read(In(None), &mut world).map_err(|e| format!("{e:?}"))?
        );
        assert!(
            world
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );
        Ok(())
    }
}
