//! Window-aware ownership adapter. Never rebind global input resources or run a
//! project's input systems. One editor navigation gesture has one view/host epoch.
use super::*;
use crate::scene_view::camera;
use bevy::input::{
    ButtonState,
    keyboard::KeyboardInput,
    mouse::{MouseButtonInput, MouseMotion, MouseScrollUnit, MouseWheel},
};
use bevy::input_focus::{FocusCause, InputFocus};
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Fly,
    Orbit,
    Pan,
}
struct Capture {
    view: ViewId,
    camera: Entity,
    window: Entity,
    revision: u64,
    mode: Mode,
    last: Vec2,
    cursor: Option<bevy::window::CursorOptions>,
}
struct Click {
    view: ViewId,
    camera: Entity,
    window: Entity,
    revision: u64,
    start: Vec2,
}
#[derive(Resource, Default)]
pub(super) struct Input {
    capture: Option<Capture>,
    click: Option<Click>,
    keys: HashSet<KeyCode>,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Input>()
        .add_systems(Update, events.after(tick));
}
pub(super) fn diagnostics(world: &World, state: &EditorWorkspace) -> serde_json::Value {
    serde_json::json!({"gesture":state.gesture.is_some(),"pending_host":state.pending.is_some(),"menu_nodes":state.menu.len(),"measuring":state.measurements.is_some(),"rendered":state.rendered,"revision":state.layout.revision(),"capture":world.get_resource::<Input>().and_then(|input|input.capture.as_ref()).map(|capture|serde_json::json!({"view":capture.view,"window":capture.window.to_bits()})),"viewports":state.panels.scenes.iter().map(|(view,scene)|serde_json::json!({"view":view,"rect":scene.area(world).map(|rect|[rect.min.x,rect.min.y,rect.max.x,rect.max.y]),"eligible_windows":state.surfaces.values().filter_map(|surface|eligible(world,state,*view,surface.window).map(|_|surface.window.to_bits())).collect::<Vec<_>>(),"pointer_windows":state.surfaces.values().filter_map(|surface|under_pointer(world,state,surface.window).filter(|(id,..)|id==view).map(|_|surface.window.to_bits())).collect::<Vec<_>>()})).collect::<Vec<_>>()})
}
fn eligible(
    world: &World,
    state: &EditorWorkspace,
    view: ViewId,
    window: Entity,
) -> Option<(Entity, Rect)> {
    if state.gesture.is_some()
        || state.pending.is_some()
        || !state.menu.is_empty()
        || state.measurements.is_some()
        || state.rendered != Some(state.layout.revision())
    {
        return None;
    }
    let (group, _) = state.layout.snapshot().location(view)?;
    if !matches!(&state.layout.snapshot().nodes[&group],kedra_docking::model::Node::Group{active,..}if *active==view)
    {
        return None;
    }
    let host = state.layout.snapshot().host_of(group)?;
    let surface = state.surfaces.get(&host)?;
    if surface.window != window || surface.staged.is_some() || !world.get::<Window>(window)?.focused
    {
        return None;
    }
    if state.layout.snapshot().hosts[&host]
        .maximized
        .is_some_and(|max| max != group)
    {
        return None;
    }
    let scene = state.panels.scenes.get(&view)?;
    let area = scene.area(world)?;
    if area.size().min_element() < 1. {
        return None;
    }
    Some((scene.navigation(world)?, area))
}
fn under_pointer(
    world: &World,
    state: &EditorWorkspace,
    window: Entity,
) -> Option<(ViewId, Entity, Rect, Vec2)> {
    let point = position(world, window)?;
    state.panels.scenes.keys().find_map(|view| {
        let (camera, area) = eligible(world, state, *view, window)?;
        (area.contains(point)
            && hit::visible_at(world, state.panels.scenes[view].focus(), point, 0.))
        .then_some((*view, camera, area, point))
    })
}
fn position(world: &World, window: Entity) -> Option<Vec2> {
    let sample = world.get_resource::<input::CursorSample>()?;
    (sample.window == Some(window)).then_some(sample.position)
}
fn release(world: &mut World, input: &mut Input) {
    input.click = None;
    if let Some(capture) = input.capture.take() {
        camera::cancel(world, capture.camera);
        if let Some(cursor) = capture.cursor
            && let Some(mut current) = world.get_mut::<bevy::window::CursorOptions>(capture.window)
        {
            *current = cursor;
        }
    }
    input.keys.clear();
}
pub(super) fn cancel(world: &mut World) {
    if world.contains_resource::<Input>() {
        world.resource_scope(|world, mut input: Mut<Input>| release(world, &mut input));
    }
}
fn events(
    mut buttons: MessageReader<MouseButtonInput>,
    mut motion: MessageReader<MouseMotion>,
    mut keys: MessageReader<KeyboardInput>,
    mut wheel: MessageReader<MouseWheel>,
    mut commands: Commands,
) {
    let buttons = buttons.read().cloned().collect::<Vec<_>>();
    let relative = motion.read().map(|motion| motion.delta).sum::<Vec2>();
    let keys = keys.read().cloned().collect::<Vec<_>>();
    let wheel = wheel.read().cloned().collect::<Vec<_>>();
    commands.queue(move |world: &mut World| {
        if !world.contains_resource::<EditorWorkspace>() {
            cancel(world);
            return;
        }
        world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            world.resource_scope(|world, mut input: Mut<Input>| {
                if input.capture.as_ref().is_some_and(|capture| {
                    capture.revision != state.layout.revision()
                        || eligible(world, &state, capture.view, capture.window)
                            .is_none_or(|(camera, _)| camera != capture.camera)
                        || world
                            .get_resource::<InputFocus>()
                            .and_then(|focus| focus.get())
                            != state
                                .panels
                                .scenes
                                .get(&capture.view)
                                .map(|scene| scene.focus())
                }) {
                    debug!(
                        "Viewport capture cancelled: focus={:?}, expected={:?}",
                        world.get_resource::<InputFocus>().and_then(InputFocus::get),
                        input
                            .capture
                            .as_ref()
                            .and_then(|capture| state.panels.scenes.get(&capture.view))
                            .map(|scene| scene.focus())
                    );
                    release(world, &mut input);
                }
                let mut acquired = false;
                for event in &buttons {
                    if event.state == ButtonState::Released {
                        if event.button == MouseButton::Left
                            && let Some(click) = input.click.take()
                            && click.window == event.window
                            && click.revision == state.layout.revision()
                            && eligible(world, &state, click.view, click.window)
                                .is_some_and(|(camera, _)| camera == click.camera)
                            && let Some((view, _, _, point)) =
                                under_pointer(world, &state, event.window)
                            && view == click.view
                            && point.distance(click.start) < 4.
                            && let Some(document) = state
                                .documents
                                .active
                                .filter(|document| state.documents.opened[document].view == view)
                        {
                            match state.panels.scenes[&view].pick(world, point) {
                                Ok(Some(address)) => {
                                    state.documents.select(document, address);
                                }
                                Ok(None) => {
                                    state.documents.clear_selection(view);
                                }
                                Err(error) => notice(world, &state, &error),
                            }
                        }
                        if input.capture.as_ref().is_some_and(|capture| {
                            capture.window == event.window
                                && match capture.mode {
                                    Mode::Fly => event.button == MouseButton::Right,
                                    Mode::Orbit => event.button == MouseButton::Left,
                                    Mode::Pan => event.button == MouseButton::Middle,
                                }
                        }) {
                            release(world, &mut input);
                        }
                        continue;
                    }
                    if input.capture.is_some() {
                        continue;
                    }
                    let Some((view, camera, _, point)) = under_pointer(world, &state, event.window)
                    else {
                        continue;
                    };
                    let alt = world
                        .resource::<ButtonInput<KeyCode>>()
                        .any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
                    let mode = match event.button {
                        MouseButton::Right => Some(Mode::Fly),
                        MouseButton::Middle => Some(Mode::Pan),
                        MouseButton::Left if alt => Some(Mode::Orbit),
                        _ => None,
                    };
                    debug!(
                        "Viewport press: {:?}, alt={alt}, navigation={}",
                        event.button,
                        mode.is_some()
                    );
                    let modified = world.resource::<ButtonInput<KeyCode>>().any_pressed([
                        KeyCode::ShiftLeft,
                        KeyCode::ShiftRight,
                        KeyCode::ControlLeft,
                        KeyCode::ControlRight,
                        KeyCode::SuperLeft,
                        KeyCode::SuperRight,
                    ]);
                    if event.button == MouseButton::Left && !alt && !modified {
                        input.click = Some(Click {
                            view,
                            camera,
                            window: event.window,
                            revision: state.layout.revision(),
                            start: point,
                        });
                    }
                    if let Some(mode) = mode {
                        input.click = None;
                        let cursor = if mode == Mode::Fly {
                            world
                                .get::<bevy::window::CursorOptions>(event.window)
                                .cloned()
                        } else {
                            None
                        };
                        // Relative device motion is routed only to this lease's
                        // focused native window, never to every editor camera.
                        if mode == Mode::Fly
                            && let Some(mut options) =
                                world.get_mut::<bevy::window::CursorOptions>(event.window)
                        {
                            options.grab_mode = bevy::window::CursorGrabMode::Locked;
                            options.visible = false;
                        }
                        camera::cancel(world, camera);
                        if mode != Mode::Fly {
                            camera::start(world, camera, mode == Mode::Pan);
                        }
                        input.capture = Some(Capture {
                            view,
                            camera,
                            window: event.window,
                            revision: state.layout.revision(),
                            mode,
                            last: point,
                            cursor,
                        });
                        acquired = true;
                    }
                    state.focused_group = state
                        .layout
                        .snapshot()
                        .location(view)
                        .map(|(group, _)| group);
                    activate_content(&mut state, view);
                    if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
                        focus.set(state.panels.scenes[&view].focus(), FocusCause::Pressed);
                    }
                }
                for event in &keys {
                    if event.key_code == KeyCode::Escape && event.state.is_pressed() {
                        release(world, &mut input);
                        continue;
                    }
                    if let Some(capture) = &input.capture {
                        if capture.window != event.window {
                            continue;
                        }
                        if event.state.is_pressed() {
                            input.keys.insert(event.key_code);
                        } else {
                            input.keys.remove(&event.key_code);
                        }
                    } else if event.state.is_pressed()
                        && !event.repeat
                        && event.key_code == KeyCode::KeyF
                        && !world.resource::<ButtonInput<KeyCode>>().any_pressed([
                            KeyCode::ControlLeft,
                            KeyCode::ControlRight,
                            KeyCode::AltLeft,
                            KeyCode::AltRight,
                            KeyCode::SuperLeft,
                            KeyCode::SuperRight,
                        ])
                    {
                        let focused = world
                            .get_resource::<InputFocus>()
                            .and_then(|focus| focus.get());
                        let target = state.panels.scenes.iter().find_map(|(view, scene)| {
                            (focused == Some(scene.focus())).then_some(*view)
                        });
                        if let Some((camera, _)) =
                            target.and_then(|view| eligible(world, &state, view, event.window))
                            && let Err(error) =
                                camera::command(world, camera, camera::Command::FrameSelection)
                        {
                            notice(world, &state, &error);
                        }
                    }
                }
                for event in &wheel {
                    let amount = match event.unit {
                        MouseScrollUnit::Line => event.y,
                        MouseScrollUnit::Pixel => event.y / 40.,
                    };
                    if let Some(capture) = &input.capture {
                        if capture.window == event.window && capture.mode == Mode::Fly {
                            camera::speed(world, capture.camera, amount);
                        }
                    } else if let Some((view, camera, _, _)) =
                        under_pointer(world, &state, event.window)
                    {
                        let focused = world
                            .get_resource::<InputFocus>()
                            .and_then(|focus| focus.get());
                        if focused == Some(state.panels.scenes[&view].focus()) {
                            camera::zoom(world, camera, -amount);
                        }
                    }
                }
                if let Some(capture) = input.capture.as_mut() {
                    let position = position(world, capture.window).unwrap_or(capture.last);
                    let delta = position - capture.last;
                    capture.last = position;
                    let (camera, mode, view) = (capture.camera, capture.mode, capture.view);
                    if mode == Mode::Fly {
                        let held = |key| f32::from(input.keys.contains(&key));
                        let axis = Vec3::new(
                            held(KeyCode::KeyD) - held(KeyCode::KeyA),
                            held(KeyCode::KeyE) - held(KeyCode::KeyQ),
                            held(KeyCode::KeyW) - held(KeyCode::KeyS),
                        );
                        let fast = input.keys.contains(&KeyCode::ShiftLeft)
                            || input.keys.contains(&KeyCode::ShiftRight);
                        let dt = world.resource::<Time>().delta_secs();
                        camera::fly(
                            world,
                            camera,
                            if acquired { Vec2::ZERO } else { relative },
                            axis,
                            fast,
                            dt,
                        );
                    } else {
                        let scale = world
                            .get::<Camera>(camera)
                            .and_then(Camera::logical_viewport_size)
                            .zip(
                                state
                                    .panels
                                    .scenes
                                    .get(&view)
                                    .and_then(|scene| scene.area(world)),
                            )
                            .map_or(Vec2::ONE, |(size, area)| size / area.size());
                        camera::drag(world, camera, delta * scale);
                    }
                }
            })
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_focus_belongs_to_the_viewport_not_its_decorative_render_image()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut app, _, _) = super::super::input::regression_tests::setup()?;
        app.add_observer(bevy::input_focus::tab_navigation::acquire_focus_tab_index);
        let world = app.world_mut();
        super::super::test_support::sources(world)?;
        let parent = world.resource::<EditorWorkspace>().root;
        let view = crate::scene_view::View::mount(world, parent, ViewId::new(), "Focus fixture")?;
        let focus = view.focus();
        assert!(
            world
                .get::<kedra_ui::ElementId>(focus)
                .is_some_and(|identity| identity.0.ends_with("/viewport"))
        );
        assert!(
            world
                .get::<bevy::input_focus::tab_navigation::TabIndex>(focus)
                .is_some()
        );
        let image = world
            .query::<(Entity, &kedra_ui::ElementId)>()
            .iter(world)
            .find_map(|(entity, identity)| identity.0.ends_with("/render").then_some(entity))
            .ok_or("Render image absent")?;
        assert!(
            world
                .get::<bevy::picking::Pickable>(image)
                .is_some_and(|pickable| !pickable.is_hoverable && !pickable.should_block_lower)
        );
        let window = world.resource::<EditorWorkspace>().window;
        world.trigger(bevy::input_focus::AcquireFocus {
            focused_entity: focus,
            window,
        });
        world.flush();
        assert_eq!(world.resource::<InputFocus>().get(), Some(focus));
        Ok(())
    }
    #[test]
    fn viewport_coordinates_follow_window_scoped_input_not_a_stale_window_component() {
        let mut world = World::new();
        let mut window = Window::default();
        window.set_cursor_position(Some(Vec2::splat(700.)));
        let window = world.spawn(window).id();
        let other = world.spawn(Window::default()).id();
        world.insert_resource(input::CursorSample {
            window: Some(window),
            position: Vec2::new(32., 64.),
        });
        assert_eq!(position(&world, window), Some(Vec2::new(32., 64.)));
        assert_eq!(position(&world, other), None);
    }
    #[test]
    fn retirement_restores_only_the_owned_cursor_and_clears_every_navigation_hold() {
        let mut world = World::new();
        let camera = world
            .spawn((Camera3d::default(), Transform::from_xyz(1., 2., 3.)))
            .id();
        camera::attach(&mut world, camera, None);
        camera::start(&mut world, camera, false);
        let window = world
            .spawn((Window::default(), bevy::window::CursorOptions::default()))
            .id();
        let other = world
            .spawn((
                Window::default(),
                bevy::window::CursorOptions {
                    visible: false,
                    ..default()
                },
            ))
            .id();
        let original = world.get::<bevy::window::CursorOptions>(window).cloned();
        world
            .get_mut::<bevy::window::CursorOptions>(window)
            .unwrap_or_else(|| panic!("cursor"))
            .grab_mode = bevy::window::CursorGrabMode::Confined;
        let mut input = Input {
            capture: Some(Capture {
                view: ViewId::new(),
                camera,
                window,
                revision: 1,
                mode: Mode::Fly,
                last: Vec2::ZERO,
                cursor: original,
            }),
            keys: HashSet::from([KeyCode::KeyW]),
            click: None,
        };
        release(&mut world, &mut input);
        release(&mut world, &mut input);
        assert!(input.capture.is_none() && input.keys.is_empty());
        assert_eq!(
            world
                .get::<bevy::window::CursorOptions>(window)
                .map(|cursor| cursor.grab_mode),
            Some(bevy::window::CursorGrabMode::None)
        );
        assert!(
            world
                .get::<bevy::window::CursorOptions>(other)
                .is_some_and(|cursor| !cursor.visible)
        );
    }
}
