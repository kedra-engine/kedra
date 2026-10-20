//! Frame-ordered native Bevy input injection, adapted from the Kedra prototype.
//! No direct Activate event, field assignment or model mutation masquerades as input.
use bevy::{
    input::{
        ButtonState, InputSystems,
        keyboard::{Key, KeyboardInput, NativeKeyCode},
        mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel},
    },
    prelude::*,
    window::{CursorMoved, WindowEvent},
};
use bevy_remote::BrpResult;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::VecDeque;
use uuid::Uuid;
mod held;
use held::HeldInputs;

#[cfg(test)]
#[path = "input_tests.rs"]
mod lifecycle_tests;

/// Shared input adapter, installed by the engine application host.
/// Injected holds belong to one window until released or retired, independently
/// of operation receipts. Pointer movement, wheel events and owned pointer release
/// may target another live window for cross-host gestures; their event metadata
/// uses that destination. A new overlapping hold or keyboard input in another
/// window is rejected. Resizing another window is allowed. Hardware-held inputs
/// cannot be borrowed. Unowned releases are successful no-ops, never aggregate
/// hardware-clearing events; owned modifier releases stay scoped to their owner.
/// Retirement emits releases on both Bevy message surfaces for unshared holds.
/// If subsequent hardware messages share an identity, cleanup preserves that
/// hardware state instead of emitting an aggregate release for it. Hosts must
/// deliver hardware input through Bevy's standard messages, not mutate aggregate
/// `ButtonInput` resources to express overlapping ownership.
pub struct InputPlugin;
impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputQueue>()
            .init_resource::<HeldInputs>()
            .add_systems(PreUpdate, inject.before(InputSystems))
            .add_systems(PreUpdate, held::release_unshared.after(InputSystems));
    }
}
#[derive(Clone)]
enum Step {
    Move(Vec2),
    Button(bool),
    Wheel(Vec2),
    Keys(Vec<KeyboardInput>),
    Resize {
        width: f32,
        height: f32,
        dpr: Option<f64>,
    },
    Complete,
}
#[derive(Resource, Default)]
pub(crate) struct InputQueue {
    steps: VecDeque<Step>,
    window: Option<Entity>,
    operation: Option<Uuid>,
}
impl InputQueue {
    pub(crate) fn busy(&self) -> bool {
        !self.steps.is_empty()
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Input {
    Modifier {
        modifier: Modifier,
        pressed: bool,
    },
    Button {
        pressed: bool,
    },
    Click {
        element: Option<String>,
        position: Option<[f32; 2]>,
    },
    Move {
        position: [f32; 2],
    },
    Drag {
        from: [f32; 2],
        to: [f32; 2],
    },
    Wheel {
        delta: [f32; 2],
    },
    Key {
        key: String,
    },
    Text {
        text: String,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Modifier {
    Shift,
    Control,
    Alt,
    Super,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    session: String,
    id: Uuid,
    window: u64,
    input: Input,
}

/// Validate process and window generations before admitting an input effect.
pub fn target(world: &World, session: &str, bits: u64) -> Result<Entity, String> {
    if world.resource::<crate::AppSession>().id != session {
        return Err("Stale application session".into());
    }
    Entity::try_from_bits(bits)
        .filter(|entity| world.get::<Window>(*entity).is_some())
        .ok_or_else(|| "Stale or unavailable window".into())
}
fn point(world: &World, window: Entity, point: [f32; 2]) -> Result<Vec2, String> {
    let size = world.get::<Window>(window).ok_or("Window unavailable")?;
    if !point.iter().all(|v| v.is_finite())
        || point[0] < 0.
        || point[1] < 0.
        || point[0] >= size.width()
        || point[1] >= size.height()
    {
        return Err("Pointer position is outside window".into());
    }
    Ok(Vec2::from_array(point))
}
pub(crate) fn request(In(value): In<Option<Value>>, world: &mut World) -> BrpResult {
    let original = value.clone().unwrap_or(Value::Null);
    let request: Request = crate::params(value)?;
    if let Some(receipt) = retry(world, &request.session, request.id, &original)? {
        return Ok(receipt);
    }
    let window = target(world, &request.session, request.window).map_err(crate::invalid)?;
    let held = world.resource::<HeldInputs>();
    match &request.input {
        Input::Move { .. }
        | Input::Wheel { .. }
        | Input::Button { pressed: false }
        | Input::Modifier { pressed: false, .. } => {}
        _ => held.validate_window(window).map_err(crate::invalid)?,
    }
    let mut steps = VecDeque::new();
    match request.input {
        Input::Modifier { modifier, pressed } => {
            let (key_code, logical_key) = match modifier {
                Modifier::Shift => (KeyCode::ShiftLeft, Key::Shift),
                Modifier::Control => (KeyCode::ControlLeft, Key::Control),
                Modifier::Alt => (KeyCode::AltLeft, Key::Alt),
                Modifier::Super => (KeyCode::SuperLeft, Key::Super),
            };
            if !pressed && held.key_held(key_code) {
                held.validate_window(window).map_err(crate::invalid)?;
            }
            steps.push_back(Step::Keys(vec![KeyboardInput {
                key_code,
                logical_key,
                state: if pressed {
                    ButtonState::Pressed
                } else {
                    ButtonState::Released
                },
                text: None,
                repeat: false,
                window,
            }]));
        }
        Input::Button { pressed } => {
            steps.push_back(Step::Button(pressed));
        }
        Input::Click { element, position } => {
            if held.mouse_held() {
                return Err(crate::invalid(
                    "Release the injected pointer hold before clicking",
                ));
            }
            let at = match (element, position) {
                (Some(id), None) => {
                    crate::observe::element_center(world, window, &id).map_err(crate::invalid)?
                }
                (None, Some(at)) => point(world, window, at).map_err(crate::invalid)?,
                _ => return Err(crate::invalid("Provide exactly one of element or position")),
            };
            steps.extend([Step::Move(at), Step::Button(true), Step::Button(false)]);
        }
        Input::Move { position } => steps.push_back(Step::Move(
            point(world, window, position).map_err(crate::invalid)?,
        )),
        Input::Drag { from, to } => {
            if held.mouse_held() {
                return Err(crate::invalid(
                    "Release the injected pointer hold before dragging",
                ));
            }
            let from = point(world, window, from).map_err(crate::invalid)?;
            let to = point(world, window, to).map_err(crate::invalid)?;
            steps.extend([Step::Move(from), Step::Button(true)]);
            for n in 1..=8 {
                steps.push_back(Step::Move(from.lerp(to, n as f32 / 8.)));
            }
            steps.push_back(Step::Button(false));
        }
        Input::Wheel { delta } => {
            if !delta.iter().all(|v| v.is_finite() && v.abs() <= 10000.) {
                return Err(crate::invalid("Invalid wheel delta"));
            }
            steps.push_back(Step::Wheel(Vec2::from_array(delta)));
        }
        Input::Text { text } => {
            if text.len() > 16384 {
                return Err(crate::invalid("Text input exceeds 16 KiB"));
            }
            let down = KeyboardInput {
                key_code: KeyCode::Unidentified(NativeKeyCode::Unidentified),
                logical_key: Key::Character(text.clone().into()),
                state: ButtonState::Pressed,
                text: Some(text.into()),
                repeat: false,
                window,
            };
            steps.push_back(Step::Keys(vec![down.clone()]));
            let mut up = down;
            up.state = ButtonState::Released;
            up.text = None;
            steps.push_back(Step::Keys(vec![up]));
        }
        Input::Key { key } => {
            // A chord must not release a modifier explicitly held by an earlier
            // operation. Its key-up batch owns only newly pressed identities.
            let events: Vec<_> = keys(&key, window)
                .map_err(crate::invalid)?
                .into_iter()
                .filter(|event| !held.key_held(event.key_code))
                .collect();
            steps.push_back(Step::Keys(events.clone()));
            steps.push_back(Step::Keys(
                events
                    .into_iter()
                    .rev()
                    .map(|mut event| {
                        event.state = ButtonState::Released;
                        event
                    })
                    .collect(),
            ));
        }
    }
    admit(world, request.id, &original, window, steps)
}
fn keys(key: &str, window: Entity) -> Result<Vec<KeyboardInput>, String> {
    let mut modifiers = Vec::new();
    let (code, key) = match key {
        "enter" => (KeyCode::Enter, Key::Enter),
        "escape" => (KeyCode::Escape, Key::Escape),
        "tab" => (KeyCode::Tab, Key::Tab),
        "shift_tab" => {
            modifiers.push((KeyCode::ShiftLeft, Key::Shift));
            (KeyCode::Tab, Key::Tab)
        }
        "space" => (KeyCode::Space, Key::Space),
        "backspace" => (KeyCode::Backspace, Key::Backspace),
        "delete" => (KeyCode::Delete, Key::Delete),
        "left" => (KeyCode::ArrowLeft, Key::ArrowLeft),
        "right" => (KeyCode::ArrowRight, Key::ArrowRight),
        "up" => (KeyCode::ArrowUp, Key::ArrowUp),
        "down" => (KeyCode::ArrowDown, Key::ArrowDown),
        "home" => (KeyCode::Home, Key::Home),
        "end" => (KeyCode::End, Key::End),
        "f1" => (KeyCode::F1, Key::F1),
        "f2" => (KeyCode::F2, Key::F2),
        "f3" => (KeyCode::F3, Key::F3),
        "f4" => (KeyCode::F4, Key::F4),
        "f5" => (KeyCode::F5, Key::F5),
        "f6" => (KeyCode::F6, Key::F6),
        "f7" => (KeyCode::F7, Key::F7),
        "f8" => (KeyCode::F8, Key::F8),
        "f12" => (KeyCode::F12, Key::F12),
        "select_all" | "copy" | "paste" | "cut" => {
            modifiers.push(if cfg!(target_os = "macos") {
                (KeyCode::SuperLeft, Key::Super)
            } else {
                (KeyCode::ControlLeft, Key::Control)
            });
            match key {
                "select_all" => (KeyCode::KeyA, Key::Character("a".into())),
                "copy" => (KeyCode::KeyC, Key::Character("c".into())),
                "paste" => (KeyCode::KeyV, Key::Character("v".into())),
                _ => (KeyCode::KeyX, Key::Character("x".into())),
            }
        }
        _ => return Err("Unsupported keyboard chord".into()),
    };
    modifiers.push((code, key));
    Ok(modifiers
        .into_iter()
        .map(|(key_code, logical_key)| KeyboardInput {
            key_code,
            logical_key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        })
        .collect())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Resize {
    session: String,
    id: Uuid,
    window: u64,
    width: f32,
    height: f32,
    dpr: Option<f64>,
}
pub(crate) fn resize(In(value): In<Option<Value>>, world: &mut World) -> BrpResult {
    let original = value.clone().unwrap_or(Value::Null);
    let request: Resize = crate::params(value)?;
    if let Some(receipt) = retry(world, &request.session, request.id, &original)? {
        return Ok(receipt);
    }
    let window = target(world, &request.session, request.window).map_err(crate::invalid)?;
    if ![request.width, request.height]
        .into_iter()
        .all(|v| v.is_finite() && (320.0..=3840.).contains(&v))
        || request
            .dpr
            .is_some_and(|v| !v.is_finite() || !(1.0..=4.).contains(&v))
    {
        return Err(crate::invalid("Invalid logical viewport or test DPR"));
    }
    admit(
        world,
        request.id,
        &original,
        window,
        VecDeque::from([Step::Resize {
            width: request.width,
            height: request.height,
            dpr: request.dpr,
        }]),
    )
}
pub(crate) fn retry(
    world: &mut World,
    session: &str,
    id: Uuid,
    request: &Value,
) -> Result<Option<Value>, bevy_remote::BrpError> {
    if world.resource::<crate::AppSession>().id != session {
        return Err(crate::invalid("Stale application session"));
    }
    let previous = world
        .resource::<crate::operations::Operations>()
        .get(id)
        .cloned();
    if let Some(previous) = previous {
        world
            .resource_mut::<crate::operations::Operations>()
            .admit(id, request)
            .map_err(crate::invalid)?;
        return serde_json::to_value(previous)
            .map(Some)
            .map_err(bevy_remote::BrpError::internal);
    }
    Ok(None)
}
fn admit(
    world: &mut World,
    id: Uuid,
    request: &Value,
    window: Entity,
    mut steps: VecDeque<Step>,
) -> BrpResult {
    if let Some(old) = world.resource::<crate::operations::Operations>().get(id) {
        let old = serde_json::to_value(old).map_err(bevy_remote::BrpError::internal)?;
        world
            .resource_mut::<crate::operations::Operations>()
            .admit(id, request)
            .map_err(crate::invalid)?;
        return Ok(old);
    }
    if world.resource::<InputQueue>().busy() {
        return Err(crate::invalid("Input transaction is busy"));
    }
    world
        .resource_mut::<crate::operations::Operations>()
        .admit(id, request)
        .map_err(crate::invalid)?;
    steps.push_back(Step::Complete);
    *world.resource_mut::<InputQueue>() = InputQueue {
        steps,
        window: Some(window),
        operation: Some(id),
    };
    Ok(json!({"id":id,"state":"pending"}))
}
fn inject(world: &mut World) {
    let mut held = world.remove_resource::<HeldInputs>().unwrap_or_default();
    held.observe_external(world);
    held.retire(world);
    let mut queue = world.remove_resource::<InputQueue>().unwrap_or_default();
    if let Some(step) = queue.steps.pop_front() {
        let target = queue
            .window
            .filter(|entity| world.get::<Window>(*entity).is_some());
        if let Some(window) = target {
            match step {
                Step::Move(position) => {
                    if let Some(mut target) = world.get_mut::<Window>(window) {
                        target.set_cursor_position(Some(position));
                    }
                    let event = CursorMoved {
                        window,
                        position,
                        delta: None,
                    };
                    world.write_message(event.clone());
                    world.write_message(WindowEvent::CursorMoved(event));
                }
                Step::Button(down) => {
                    if let Err(error) = held.button(world, window, down) {
                        fail(world, &mut queue, error);
                    }
                }
                Step::Keys(events) => {
                    if let Err(error) = held.validate_keys(world, &events) {
                        fail(world, &mut queue, error);
                    } else {
                        for event in events {
                            held.key(world, event);
                        }
                    }
                }
                Step::Wheel(delta) => {
                    let event = MouseWheel {
                        unit: MouseScrollUnit::Pixel,
                        x: delta.x,
                        y: delta.y,
                        window,
                        phase: bevy::input::touch::TouchPhase::Moved,
                    };
                    world.write_message(event);
                    world.write_message(WindowEvent::MouseWheel(event));
                }
                Step::Resize { width, height, dpr } => {
                    if let Some(mut target) = world.get_mut::<Window>(window) {
                        target
                            .resolution
                            .set_scale_factor_override(dpr.map(|v| v as f32));
                        target.resolution.set(width, height);
                    }
                    let event = bevy::window::WindowResized {
                        window,
                        width,
                        height,
                    };
                    world.write_message(event.clone());
                    world.write_message(WindowEvent::WindowResized(event));
                    if let Some(scale_factor) = dpr {
                        let event = bevy::window::WindowScaleFactorChanged {
                            window,
                            scale_factor,
                        };
                        world.write_message(event.clone());
                        world.write_message(WindowEvent::WindowScaleFactorChanged(event));
                    }
                }
                Step::Complete => {
                    if let Some(id) = queue.operation.take() {
                        world
                            .resource_mut::<crate::operations::Operations>()
                            .finish(id, Ok(json!({"delivered":true})));
                    }
                }
            }
        } else {
            fail(
                world,
                &mut queue,
                "Input target was retired; adapter-owned holds were released".into(),
            );
        }
    }
    held.skip_injected(world);
    world.insert_resource(held);
    world.insert_resource(queue);
}

fn fail(world: &mut World, queue: &mut InputQueue, error: String) {
    queue.steps.clear();
    if let Some(id) = queue.operation.take() {
        world
            .resource_mut::<crate::operations::Operations>()
            .finish(id, Err(error));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wheel_and_resize_reach_both_native_bevy_message_surfaces() {
        let mut app = App::new();
        app.add_message::<WindowEvent>()
            .add_message::<MouseWheel>()
            .add_message::<bevy::window::WindowResized>()
            .add_message::<bevy::window::WindowScaleFactorChanged>();
        let window = app.world_mut().spawn(Window::default()).id();
        app.insert_resource(InputQueue {
            steps: VecDeque::from([
                Step::Wheel(Vec2::new(0., 20.)),
                Step::Resize {
                    width: 800.,
                    height: 600.,
                    dpr: Some(2.),
                },
            ]),
            window: Some(window),
            operation: None,
        });
        inject(app.world_mut());
        inject(app.world_mut());
        let world = app.world();
        assert_eq!(world.resource::<Messages<MouseWheel>>().len(), 1);
        assert_eq!(
            world
                .resource::<Messages<bevy::window::WindowResized>>()
                .len(),
            1
        );
        assert_eq!(
            world
                .resource::<Messages<bevy::window::WindowScaleFactorChanged>>()
                .len(),
            1
        );
        assert_eq!(world.resource::<Messages<WindowEvent>>().len(), 3);
    }
}
