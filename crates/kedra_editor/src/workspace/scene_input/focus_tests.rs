//! Active-document shortcuts and hover navigation preserve tool/text ownership.
use super::*;
use bevy::camera_controller::pan_orbit_camera::prelude::PanOrbitCamera;
use bevy::ecs::system::RunSystemOnce;

#[test]
fn tool_focus_routes_to_active_scene_but_text_and_inactive_windows_do_not()
-> Result<(), Box<dyn std::error::Error>> {
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
    {
        let mut state = world.resource_mut::<EditorWorkspace>();
        state.rendered = Some(state.layout.revision());
        state.content_view = Some(view_id);
        state.panels.scenes.insert(view_id, view);
    }
    let row = world.spawn_empty().id();
    world
        .resource_mut::<InputFocus>()
        .set(row, FocusCause::Pressed);
    world
        .get_mut::<camera::Navigation>(camera)
        .ok_or("Navigation missing")?
        .selection = Some((Vec3::splat(10.), Vec3::splat(12.)));
    let key = KeyboardInput {
        key_code: KeyCode::KeyF,
        logical_key: bevy::input::keyboard::Key::Character("f".into()),
        state: ButtonState::Pressed,
        text: Some("f".into()),
        repeat: false,
        window,
    };
    world
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key.clone());
    world
        .run_system_once(events)
        .map_err(|error| error.to_string())?;
    assert_eq!(
        world
            .get::<camera::Navigation>(camera)
            .ok_or("Navigation missing")?
            .pivot,
        Vec3::splat(11.)
    );
    assert_eq!(world.resource::<InputFocus>().get(), Some(row));
    let before = *world.get::<Transform>(camera).ok_or("Camera missing")?;
    let field = world.spawn(bevy::text::EditableText::new("draft")).id();
    world
        .resource_mut::<InputFocus>()
        .set(field, FocusCause::Pressed);
    world
        .get_mut::<camera::Navigation>(camera)
        .ok_or("Navigation missing")?
        .selection = Some((Vec3::ZERO, Vec3::ONE));
    world.resource_mut::<Messages<KeyboardInput>>().clear();
    world
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key.clone());
    world
        .run_system_once(events)
        .map_err(|error| error.to_string())?;
    assert_eq!(world.get::<Transform>(camera), Some(&before));
    assert_eq!(world.resource::<InputFocus>().get(), Some(field));
    let modal = world
        .spawn(bevy::input_focus::tab_navigation::TabGroup {
            modal: true,
            ..default()
        })
        .id();
    let control = world.spawn(ChildOf(modal)).id();
    world
        .resource_mut::<InputFocus>()
        .set(control, FocusCause::Pressed);
    world.resource_mut::<Messages<KeyboardInput>>().clear();
    world.resource_mut::<Messages<KeyboardInput>>().write(key);
    world
        .run_system_once(events)
        .map_err(|error| error.to_string())?;
    assert_eq!(world.get::<Transform>(camera), Some(&before));
    assert_eq!(world.resource::<InputFocus>().get(), Some(control));
    world.resource_mut::<Messages<KeyboardInput>>().clear();
    world
        .resource_mut::<InputFocus>()
        .set(row, FocusCause::Pressed);
    *world.resource_mut::<input::CursorSample>() = input::CursorSample {
        window: Some(window),
        position: Vec2::splat(100.),
    };
    world
        .resource_mut::<Messages<MouseWheel>>()
        .write(MouseWheel {
            phase: bevy::input::touch::TouchPhase::Moved,
            unit: MouseScrollUnit::Line,
            x: 0.,
            y: 1.,
            window,
        });
    assert!(under_pointer(world, world.resource::<EditorWorkspace>(), window).is_some());
    world
        .run_system_once(events)
        .map_err(|error| error.to_string())?;
    assert!(
        world
            .get::<PanOrbitCamera>(camera)
            .ok_or("Controller missing")?
            .current_motion
            .is_user_controlled()
    );
    assert_eq!(world.resource::<InputFocus>().get(), Some(row));
    camera::cancel(world, camera);
    world
        .get_mut::<Window>(window)
        .ok_or("Window missing")?
        .focused = false;
    world
        .run_system_once(events)
        .map_err(|error| error.to_string())?;
    assert!(
        !world
            .get::<PanOrbitCamera>(camera)
            .ok_or("Controller missing")?
            .current_motion
            .is_user_controlled()
    );
    Ok(())
}
