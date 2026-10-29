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
    Zoom,
}
struct Capture {
    view: ViewId,
    camera: Entity,
    window: Entity,
    revision: u64,
    mode: Mode,
    last: Vec2,
    cursor: Option<bevy::window::CursorOptions>,
    button: MouseButton,
    origin: Option<Entity>,
    start: Vec2,
    dragged: bool,
    axis: Option<camera::Command>,
    area: Rect,
    focus: Entity,
}
impl Capture {
    fn owns_focus(&self, focused: Option<Entity>, viewport: Option<Entity>) -> bool {
        focused.is_some() && (focused == viewport || focused == self.origin)
    }
}
struct Click {
    view: ViewId,
    camera: Entity,
    window: Entity,
    revision: u64,
    start: Vec2,
}
#[cfg(test)]
mod ownership_tests {
    use super::*;
    #[cfg(debug_assertions)]
    #[test]
    fn eligible_viewport_window_identities_remain_strings_through_workspace_luau()
    -> Result<(), Box<dyn std::error::Error>> {
        use kedra_app::RemoteAppExt;
        let (mut app, _, view_id) = super::super::test_support::setup()?;
        app.add_plugins(kedra_script::LuauPlugin)
            .init_resource::<kedra_app::CommandCatalog>()
            .insert_resource(bevy_remote::RemoteMethods::new());
        app.register_remote(
            "editor.workspace",
            "Workspace observation",
            serde_json::json!({}),
            crate::remote::workspace_state,
        );
        app.finish();
        app.cleanup();
        let world = app.world_mut();
        let view = crate::scene_view::test_support::ready(world)?;
        let viewport = view.focus();
        super::super::test_support::box_at(world, viewport, Vec2::splat(100.), Vec2::splat(200.));
        world
            .entity_mut(viewport)
            .insert((Visibility::Inherited, InheritedVisibility::VISIBLE));
        let camera = view.navigation(world).ok_or("Camera missing")?;
        world
            .get_mut::<Camera>(camera)
            .ok_or("Camera missing")?
            .is_active = true;
        let window = world.resource::<EditorWorkspace>().window;
        world
            .get_mut::<Window>(window)
            .ok_or("Window missing")?
            .focused = true;
        let original = {
            let mut state = world.resource_mut::<EditorWorkspace>();
            state.rendered = Some(state.layout.revision());
            state.panels.scenes.insert(view_id, view);
            state.layout.snapshot().clone()
        };
        *world.resource_mut::<input::CursorSample>() = input::CursorSample {
            window: Some(window),
            position: Vec2::splat(100.),
        };
        world
            .resource_mut::<InputFocus>()
            .set(viewport, FocusCause::Pressed);
        // Eligibility and pointer-hit are real owner predicates over synthetic
        // geometry. No live entity generation is forged to bypass those checks.
        assert!(under_pointer(world, world.resource::<EditorWorkspace>(), window).is_some());
        let raw = crate::remote::workspace_state(In(None), world).map_err(|e| e.message)?;
        assert_eq!(
            raw["navigation_input"]["viewports"][0]["eligible_windows"],
            serde_json::json!([window.to_bits().to_string()])
        );
        let run = kedra_script::start(
            world,
            kedra_script::Program::sequence(
                "workspace observation",
                "workspace.luau",
                "function run(ctx) return ctx.workspace() end",
                |lua, scope, ctx| {
                    ctx.set(
                        "workspace",
                        kedra_app::scripting::bind(lua, scope, "editor.workspace", false)?,
                    )
                },
            ),
            serde_json::Value::Null,
        )?;
        app.update();
        let Some(kedra_script::Status::Completed(value)) = kedra_script::status(app.world(), run)
        else {
            return Err(format!(
                "Expected workspace observation, got {:?}",
                kedra_script::status(app.world(), run)
            )
            .into());
        };
        let navigation = &value["navigation_input"];
        let expected = serde_json::json!([window.to_bits().to_string()]);
        assert_eq!(navigation["viewports"][0]["eligible_windows"], expected);
        assert_eq!(navigation["viewports"][0]["pointer_windows"], expected);
        assert_eq!(
            serde_json::from_value::<[f64; 4]>(navigation["viewports"][0]["rect"].clone())?,
            [0., 0., 200., 200.]
        );
        assert!(navigation["revision"].is_number());
        assert!(navigation["menu_nodes"].is_number());
        assert_eq!(value["layout"], serde_json::to_value(&original)?);
        assert!(app.world().resource::<EditorWorkspace>().layout.snapshot() == &original);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(viewport));
        assert!(
            app.world()
                .get::<Window>(window)
                .ok_or("Window missing")?
                .focused
        );
        Ok(())
    }

    #[test]
    fn escape_and_axis_release_in_one_update_cancel_instead_of_snapping()
    -> Result<(), Box<dyn std::error::Error>> {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, _, view_id) = super::super::test_support::setup()?;
        app.add_message::<MouseButtonInput>()
            .add_message::<MouseMotion>()
            .add_message::<KeyboardInput>()
            .add_message::<MouseWheel>()
            .init_resource::<Input>();
        let world = app.world_mut();
        let view = crate::scene_view::test_support::ready(world)?;
        let viewport = view.focus();
        super::super::test_support::box_at(world, viewport, Vec2::splat(100.), Vec2::splat(200.));
        world
            .entity_mut(viewport)
            .insert((Visibility::Inherited, InheritedVisibility::VISIBLE));
        let camera = view.navigation(world).ok_or("Ready camera missing")?;
        world
            .get_mut::<Camera>(camera)
            .ok_or("Camera missing")?
            .is_active = true;
        let before = *world.get::<Transform>(camera).ok_or("Camera missing")?;
        let window = world.resource::<EditorWorkspace>().window;
        world
            .get_mut::<Window>(window)
            .ok_or("Window missing")?
            .focused = true;
        let revision = {
            let mut state = world.resource_mut::<EditorWorkspace>();
            state.rendered = Some(state.layout.revision());
            state.panels.scenes.insert(view_id, view);
            state.layout.revision()
        };
        let point = Vec2::splat(100.);
        *world.resource_mut::<input::CursorSample>() = input::CursorSample {
            window: Some(window),
            position: point,
        };
        world
            .resource_mut::<InputFocus>()
            .set(viewport, FocusCause::Pressed);
        world.resource_mut::<Input>().capture = Some(Capture {
            view: view_id,
            camera,
            window,
            revision,
            mode: Mode::Orbit,
            last: point,
            cursor: None,
            button: MouseButton::Left,
            origin: Some(viewport),
            start: point,
            dragged: false,
            axis: Some(camera::Command::Right),
            area: Rect::from_corners(Vec2::ZERO, Vec2::splat(200.)),
            focus: viewport,
        });
        assert!(
            eligible(world, world.resource::<EditorWorkspace>(), view_id, window).is_some(),
            "Fixture must reach the real release path"
        );
        assert!(
            hit::visible_at(world, viewport, point, 0.),
            "Axis release origin must be a valid rendered hit target"
        );
        world
            .resource_mut::<Messages<MouseButtonInput>>()
            .write(MouseButtonInput {
                button: MouseButton::Left,
                state: ButtonState::Released,
                window,
            });
        world
            .resource_mut::<Messages<KeyboardInput>>()
            .write(KeyboardInput {
                key_code: KeyCode::Escape,
                logical_key: bevy::input::keyboard::Key::Escape,
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window,
            });
        world
            .run_system_once(events)
            .map_err(|error| error.to_string())?;
        assert_eq!(world.get::<Transform>(camera), Some(&before));
        assert!(world.resource::<Input>().capture.is_none());
        assert_eq!(
            world.resource::<EditorWorkspace>().panels.scenes[&view_id].snapshot(world)["aligning"],
            false,
            "Escape must not leave a delayed axis snap"
        );
        world.resource_mut::<Messages<MouseButtonInput>>().clear();
        world.resource_mut::<Messages<KeyboardInput>>().clear();
        world
            .resource_mut::<Messages<MouseMotion>>()
            .write(MouseMotion {
                delta: Vec2::splat(20.),
            });
        world
            .run_system_once(events)
            .map_err(|error| error.to_string())?;
        assert_eq!(world.get::<Transform>(camera), Some(&before));
        Ok(())
    }
    #[test]
    fn foreign_window_escape_does_not_retire_a_captured_navigation_lease() {
        let mut world = World::new();
        let window = world.spawn_empty().id();
        let foreign = world.spawn_empty().id();
        let mut input = Input {
            capture: Some(Capture {
                view: ViewId::new(),
                camera: world.spawn_empty().id(),
                window,
                revision: 0,
                mode: Mode::Pan,
                last: Vec2::ZERO,
                cursor: None,
                button: MouseButton::Left,
                origin: None,
                start: Vec2::ZERO,
                dragged: false,
                axis: None,
                area: Rect::default(),
                focus: window,
            }),
            ..default()
        };
        escape(&mut world, &mut input, foreign);
        assert!(input.capture.is_some());
        escape(&mut world, &mut input, window);
        assert!(input.capture.is_none());
    }
    #[test]
    fn native_focus_on_the_originating_tool_does_not_cancel_its_own_drag() {
        let mut world = World::new();
        let viewport = world.spawn_empty().id();
        let origin = world.spawn_empty().id();
        let field = world.spawn_empty().id();
        let capture = Capture {
            view: ViewId::new(),
            camera: world.spawn_empty().id(),
            window: world.spawn_empty().id(),
            revision: 0,
            mode: Mode::Orbit,
            last: Vec2::ZERO,
            cursor: None,
            button: MouseButton::Left,
            origin: Some(origin),
            start: Vec2::ZERO,
            dragged: false,
            axis: None,
            area: Rect::default(),
            focus: viewport,
        };
        assert!(capture.owns_focus(Some(origin), Some(viewport)));
        assert!(capture.owns_focus(Some(viewport), Some(viewport)));
        assert!(!capture.owns_focus(Some(field), Some(viewport)));
        assert!(!capture.owns_focus(None, Some(viewport)));
    }
}
fn escape(world: &mut World, input: &mut Input, window: Entity) {
    crate::scene_view::camera_motion::cancel_window(world, Some(window));
    if input
        .capture
        .as_ref()
        .is_some_and(|capture| capture.window == window)
        || input
            .click
            .as_ref()
            .is_some_and(|click| click.window == window)
    {
        release(world, input);
    }
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
#[cfg(debug_assertions)]
pub(super) fn diagnostics(world: &World, state: &EditorWorkspace) -> serde_json::Value {
    serde_json::json!({"gesture":state.gesture.is_some(),"pending_host":state.pending.is_some(),"menu_nodes":state.menu.len(),"measuring":state.measurements.is_some(),"preview_ready":super::input::preview_ready(state),"rendered":state.rendered,"revision":state.layout.revision(),"capture":world.get_resource::<Input>().and_then(|input|input.capture.as_ref()).map(|capture|serde_json::json!({"view":capture.view,"window":capture.window.to_bits()})),"viewports":state.panels.scenes.iter().map(|(view,scene)|serde_json::json!({"view":view,"rect":scene.area(world).map(|rect|[rect.min.x,rect.min.y,rect.max.x,rect.max.y]),"eligible_windows":state.surfaces.values().filter_map(|surface|eligible(world,state,*view,surface.window).map(|_|surface.window.to_bits().to_string())).collect::<Vec<_>>(),"pointer_windows":state.surfaces.values().filter_map(|surface|under_pointer(world,state,surface.window).filter(|(id,..)|id==view).map(|_|surface.window.to_bits().to_string())).collect::<Vec<_>>()})).collect::<Vec<_>>()})
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
        if let Some(origin) = capture
            .origin
            .filter(|entity| world.entities().contains(*entity))
        {
            kedra_ui::docking::class(world, origin, "captured", false);
            if world.entities().contains(capture.focus)
                && let Some(mut focus) = world.get_resource_mut::<InputFocus>()
                && focus.get() == Some(origin)
            {
                focus.set(capture.focus, FocusCause::Pressed);
            }
        }
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
    crate::scene_view::camera_motion::cancel_window(world, None);
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
                // Escape wins over every release/press from its window in this
                // batch, regardless of the independent message queues' order.
                let cancelled = keys
                    .iter()
                    .filter(|key| key.key_code == KeyCode::Escape && key.state.is_pressed())
                    .map(|key| key.window)
                    .collect::<HashSet<_>>();
                for window in &cancelled {
                    escape(world, &mut input, *window);
                }
                if input.capture.as_ref().is_some_and(|capture| {
                    capture.revision != state.layout.revision()
                        || eligible(world, &state, capture.view, capture.window).is_none_or(
                            |(camera, area)| camera != capture.camera || area != capture.area,
                        )
                        || !capture.owns_focus(
                            world
                                .get_resource::<InputFocus>()
                                .and_then(|focus| focus.get()),
                            state
                                .panels
                                .scenes
                                .get(&capture.view)
                                .map(|scene| scene.focus()),
                        )
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
                    if cancelled.contains(&event.window) {
                        continue;
                    }
                    if event.state == ButtonState::Released {
                        if event.button == MouseButton::Left
                            && input
                                .click
                                .as_ref()
                                .is_some_and(|click| click.window == event.window)
                            && let Some(click) = input.click.take()
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
                            capture.window == event.window && capture.button == event.button
                        }) {
                            let alignment = if let Some(capture) = &input.capture
                                && !capture.dragged
                                && let Some(axis) = capture.axis
                                && let Some(point) = position(world, event.window)
                                && point.distance(capture.start) < 4.
                                && let Some(origin) = capture.origin
                                && hit::visible_at(world, origin, point, 0.)
                            {
                                Some((capture.view, capture.camera, axis))
                            } else {
                                None
                            };
                            release(world, &mut input);
                            if let Some((view, camera, axis)) = alignment
                                && let Err(error) = state.panels.scenes[&view].align(
                                    world,
                                    camera,
                                    event.window,
                                    axis,
                                )
                            {
                                notice(world, &state, &error);
                            }
                        }
                        continue;
                    }
                    if input.capture.is_some() {
                        continue;
                    }
                    let Some((view, camera, area, point)) =
                        under_pointer(world, &state, event.window)
                    else {
                        continue;
                    };
                    let alt = world
                        .resource::<ButtonInput<KeyCode>>()
                        .any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
                    let tool = super::scene_tools::at(world, &state.panels.scenes[&view], point);
                    if tool.is_some_and(|(tool, _)| tool == super::scene_tools::Tool::Block)
                        || (tool.is_some() && event.button != MouseButton::Left)
                    {
                        continue;
                    }
                    let mode = match (event.button, tool) {
                        (MouseButton::Left, Some((super::scene_tools::Tool::Pan, _))) => {
                            Some(Mode::Pan)
                        }
                        (MouseButton::Left, Some((super::scene_tools::Tool::Zoom, _))) => {
                            Some(Mode::Zoom)
                        }
                        (
                            MouseButton::Left,
                            Some((
                                super::scene_tools::Tool::Orbit | super::scene_tools::Tool::Axis(_),
                                _,
                            )),
                        ) => Some(Mode::Orbit),
                        (MouseButton::Right, _) => Some(Mode::Fly),
                        (MouseButton::Middle, _) => Some(Mode::Pan),
                        (MouseButton::Left, _) if alt => Some(Mode::Orbit),
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
                    if event.button == MouseButton::Left && !alt && !modified && tool.is_none() {
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
                        let axis = tool.and_then(|(tool, _)| {
                            if let super::scene_tools::Tool::Axis(axis) = tool {
                                crate::scene_view::camera_motion::target(world, camera, axis).ok()
                            } else {
                                None
                            }
                        });
                        camera::cancel(world, camera);
                        if mode != Mode::Fly && mode != Mode::Zoom {
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
                            button: event.button,
                            origin: tool.map(|(_, entity)| entity),
                            start: point,
                            dragged: tool.is_none(),
                            axis,
                            area,
                            focus: state.panels.scenes[&view].focus(),
                        });
                        if let Some((_, origin)) = tool {
                            kedra_ui::docking::class(world, origin, "captured", true);
                        }
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
                        escape(world, &mut input, event.window);
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
                        if focused == Some(state.panels.scenes[&view].focus())
                            && position(world, event.window).is_some_and(|point| {
                                super::scene_tools::at(world, &state.panels.scenes[&view], point)
                                    .is_none()
                            })
                        {
                            camera::zoom(world, camera, -amount);
                        }
                    }
                }
                if let Some(capture) = input.capture.as_mut() {
                    if let Some(mut demand) = world.get_resource_mut::<kedra_wakeup::WakeDemand>() {
                        demand.active();
                    }
                    let position = position(world, capture.window).unwrap_or(capture.last);
                    let delta = position - capture.last;
                    capture.last = position;
                    if !capture.dragged {
                        if position.distance(capture.start) < 4. {
                            return;
                        }
                        capture.dragged = true;
                    }
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
                    } else if mode == Mode::Zoom {
                        camera::zoom(world, camera, delta.y * 0.08);
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
    fn reduced_motion_axis_alignment_updates_camera_and_gizmo_in_the_same_input_frame()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut app, _, _) = super::super::test_support::setup()?;
        let world = app.world_mut();
        super::super::test_support::sources(world)?;
        let parent = world.resource::<EditorWorkspace>().root;
        let mut view =
            crate::scene_view::View::mount(world, parent, ViewId::new(), "Alignment fixture")?;
        let camera = crate::scene_view::test_support::spatial(world, &mut view)?;
        world
            .get_mut::<Camera>(camera)
            .ok_or("Camera missing")?
            .is_active = true;
        *world.get_mut::<Transform>(camera).ok_or("Pose missing")? =
            Transform::from_xyz(4., 3., 5.).looking_at(Vec3::ZERO, Vec3::Y);
        world.insert_resource(kedra_ui::authored::components::UiMotion { reduced: true });
        let window = world
            .spawn(Window {
                focused: true,
                ..default()
            })
            .id();
        view.align(world, camera, window, camera::Command::Front)?;
        let axis = view.control("axis-x").ok_or("X endpoint missing")?;
        assert_eq!(
            world.get::<UiTransform>(axis),
            Some(&UiTransform::from_translation(Val2::px(34., 0.)))
        );
        assert!(
            world
                .get::<Transform>(camera)
                .ok_or("Camera missing")?
                .rotation
                .angle_between(Quat::IDENTITY)
                < 0.001
        );
        Ok(())
    }
    #[test]
    fn native_focus_belongs_to_the_viewport_not_its_decorative_render_image()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut app, _, _) = super::super::test_support::setup()?;
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
                button: MouseButton::Right,
                origin: None,
                start: Vec2::ZERO,
                dragged: true,
                axis: None,
                area: Rect::default(),
                focus: camera,
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
