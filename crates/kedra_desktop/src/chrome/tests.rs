#![allow(
    clippy::unwrap_used,
    reason = "Isolated native chrome contract assertions"
)]
use super::*;
use bevy::{math::CompassOctant, ui_widgets::Activate};
use kedra_ui::window_chrome::RegionRole;

#[test]
fn caption_close_targets_its_bound_window_not_the_primary_surface() {
    let mut app = App::new();
    app.add_message::<bevy::window::WindowCloseRequested>()
        .add_observer(activate);
    let main = app
        .world_mut()
        .spawn((Window::default(), NativeChromeState::default()))
        .id();
    let other = app
        .world_mut()
        .spawn((Window::default(), NativeChromeState::default()))
        .id();
    let close = app
        .world_mut()
        .spawn(ChromeControl {
            window: other,
            action: ChromeAction::Close,
        })
        .id();
    app.world_mut().trigger(Activate { entity: close });
    let events = app
        .world()
        .resource::<Messages<bevy::window::WindowCloseRequested>>();
    assert_eq!(
        events
            .get_cursor()
            .read(events)
            .map(|e| e.window)
            .collect::<Vec<_>>(),
        vec![other]
    );
    assert!(app.world().entities().contains(main));
    assert!(app.world().entities().contains(other));
}
#[test]
fn disabled_controls_never_dispatch_native_actions() {
    let mut app = App::new();
    app.add_message::<bevy::window::WindowCloseRequested>()
        .add_observer(activate);
    let window = app
        .world_mut()
        .spawn((Window::default(), NativeChromeState::default()))
        .id();
    let close = app
        .world_mut()
        .spawn((
            ChromeControl {
                window,
                action: ChromeAction::Close,
            },
            bevy::ui::InteractionDisabled,
        ))
        .id();
    app.world_mut().trigger(Activate { entity: close });
    assert!(
        app.world()
            .resource::<Messages<bevy::window::WindowCloseRequested>>()
            .is_empty()
    );
}
#[test]
fn native_reserve_and_client_controls_beat_resize() {
    let window = Window::default();
    for role in [RegionRole::NativeControls, RegionRole::Client] {
        assert_eq!(
            pointer_action(
                &window,
                &NativeChromeState::default(),
                Vec2::new(2., 2.),
                Some(role),
                false
            ),
            None
        );
    }
}
#[test]
fn resize_uses_six_pixel_edges_twelve_pixel_corners() {
    let size = Vec2::new(800., 600.);
    for (point, want) in [
        (Vec2::new(5., 300.), Some(CompassOctant::West)),
        (Vec2::new(6., 300.), None),
        (Vec2::new(11., 2.), Some(CompassOctant::NorthWest)),
        (Vec2::new(788., 598.), Some(CompassOctant::SouthEast)),
        (Vec2::new(400., 599.), Some(CompassOctant::South)),
        (Vec2::new(400., 300.), None),
        (Vec2::new(-1., 20.), None),
    ] {
        assert_eq!(resize_edge(point, size), want, "{point:?}");
    }
}
#[test]
fn fixed_maximized_fullscreen_and_modal_capabilities_block_invalid_actions() {
    let state = NativeChromeState::default();
    let mut window = Window {
        resizable: false,
        ..default()
    };
    assert_eq!(
        pointer_action(&window, &state, Vec2::new(2., 2.), None, false),
        None
    );
    window.resizable = true;
    for state in [
        NativeChromeState {
            maximized: true,
            ..default()
        },
        NativeChromeState {
            fullscreen: true,
            ..default()
        },
    ] {
        assert_eq!(
            pointer_action(&window, &state, Vec2::new(2., 2.), None, false),
            None
        );
    }
    assert_eq!(
        pointer_action(
            &window,
            &state,
            Vec2::new(200., 16.),
            Some(RegionRole::Drag),
            true
        ),
        None
    );
    assert_eq!(
        pointer_action(
            &window,
            &state,
            Vec2::new(200., 16.),
            Some(RegionRole::Drag),
            false
        ),
        Some(PointerAction::Move)
    );
}
#[test]
fn native_sync_waits_for_window_backend_without_a_non_send_resource() {
    let mut app = App::new();
    app.add_systems(Update, native_state);
    let window = app
        .world_mut()
        .spawn((Window::default(), NativeChromeState::default()))
        .id();
    app.update();
    assert!(
        !app.world()
            .get::<NativeChromeState>(window)
            .unwrap()
            .maximized
    );
}

#[test]
fn resize_edges_do_not_capture_content() {
    assert_eq!(
        resize_edge(Vec2::new(40., 40.), Vec2::new(1120., 720.)),
        None
    );
    assert_eq!(
        resize_edge(Vec2::new(1119., 719.), Vec2::new(1120., 720.)),
        Some(CompassOctant::SouthEast)
    );
}

#[test]
fn native_drag_uses_window_local_injected_cursor_not_hardware_cache() {
    use bevy::{
        camera::RenderTarget,
        input::{ButtonState, mouse::MouseButtonInput},
        math::Affine2,
        window::{CursorMoved, WindowRef},
    };
    use kedra_ui::window_chrome::{Header, HeaderRegion};
    let mut app = App::new();
    app.add_message::<MouseButtonInput>()
        .add_message::<CursorMoved>()
        .add_systems(Update, input);
    let world = app.world_mut();
    let window = world
        .spawn((
            Window {
                focused: true,
                ..default()
            },
            NativeChromeState::default(),
        ))
        .id();
    let camera = world
        .spawn((
            Camera::default(),
            RenderTarget::Window(WindowRef::Entity(window)),
        ))
        .id();
    let root = world
        .spawn((
            Node::default(),
            UiTargetCamera(camera),
            InheritedVisibility::VISIBLE,
            ComputedNode {
                size: Vec2::new(800., 32.),
                inverse_scale_factor: 1.,
                ..default()
            },
            UiGlobalTransform::from(Affine2::from_translation(Vec2::new(400., 16.))),
        ))
        .id();
    let _header = Header::bind(world, window, root, std::collections::BTreeMap::new()).unwrap();
    world.entity_mut(root).insert(HeaderRegion {
        header: root,
        role: RegionRole::Drag,
    });
    world.write_message(CursorMoved {
        window,
        position: Vec2::new(400., 16.),
        delta: None,
    });
    world.write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Pressed,
        window,
    });
    app.update();
    assert!(
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .internal
            .take_move_request()
    );
}

#[test]
fn client_button_below_header_prevents_native_frame_resize() {
    use bevy::{camera::RenderTarget, math::Affine2, window::WindowRef};
    use kedra_ui::window_chrome::{Header, hit};
    let mut world = World::new();
    let window = world.spawn(Window::default()).id();
    let width = world.get::<Window>(window).unwrap().width();
    let camera = world
        .spawn((
            Camera::default(),
            RenderTarget::Window(WindowRef::Entity(window)),
        ))
        .id();
    let tree = world
        .spawn((
            Node::default(),
            UiTargetCamera(camera),
            InheritedVisibility::VISIBLE,
        ))
        .id();
    let header = world
        .spawn((
            Node::default(),
            ChildOf(tree),
            InheritedVisibility::VISIBLE,
            ComputedNode {
                size: Vec2::new(width, 32.),
                inverse_scale_factor: 1.,
                ..default()
            },
            UiGlobalTransform::from(Affine2::from_translation(Vec2::new(width / 2., 16.))),
        ))
        .id();
    let _binding = Header::bind(
        &mut world,
        window,
        header,
        std::collections::BTreeMap::new(),
    )
    .unwrap();
    world.spawn((
        Node::default(),
        bevy::ui_widgets::Button,
        ChildOf(tree),
        InheritedVisibility::VISIBLE,
        ComputedNode {
            size: Vec2::new(40., 26.),
            inverse_scale_factor: 1.,
            ..default()
        },
        UiGlobalTransform::from(Affine2::from_translation(Vec2::new(width - 20., 100.))),
    ));
    let point = Vec2::new(width - 5., 100.);
    let action = pointer_action(
        world.get::<Window>(window).unwrap(),
        &NativeChromeState::default(),
        point,
        hit(&world, window, point),
        false,
    );
    assert_eq!(
        action, None,
        "The real client hit result must win over the east resize strip"
    );
}
