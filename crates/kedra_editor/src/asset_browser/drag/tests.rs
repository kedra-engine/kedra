//! Browser drag proof belongs to the picked entity even when the binding is on
//! its ancestor. A completed/cancelled drag must not leave reusable proof behind.
use super::*;
use bevy::{
    camera::NormalizedRenderTarget,
    ecs::system::RunSystemOnce,
    input::ButtonState,
    picking::{
        backend::HitData,
        events::Pointer,
        pointer::{Location, PointerButton, PointerId},
    },
    window::WindowRef,
};

#[test]
fn descendant_drag_captures_original_pick_and_end_revokes_only_that_proof() -> Result<(), String> {
    let mut app = App::new();
    install(&mut app);
    let window = app.world_mut().spawn(Window::default()).id();
    let row = app
        .world_mut()
        .spawn(Binding {
            view: ViewId::new(),
            path: PathBuf::new(),
            generation: 7,
            action: Action::Select("marker.glb".into()),
        })
        .id();
    let label = app.world_mut().spawn(ChildOf(row)).id();
    app.world_mut().flush();
    let pointer = Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Entity(window).normalize(None).ok_or("window")?,
            ),
            position: Vec2::ZERO,
        },
    );
    app.world_mut().trigger(PointerDragStart {
        entity: label,
        pointer: pointer.clone(),
        button: PointerButton::Secondary,
        hit: HitData::new(window, 0., None, None),
    });
    app.world_mut().flush();
    assert!(app.world().get::<Capture>(label).is_none());
    app.world_mut().trigger(PointerDragStart {
        entity: label,
        pointer: pointer.clone(),
        button: PointerButton::Primary,
        hit: HitData::new(window, 0., None, None),
    });
    app.world_mut().flush();
    let capture = app
        .world()
        .get::<Capture>(label)
        .ok_or("original child lost its drag proof")?;
    assert_eq!(capture.root, row);
    assert_eq!(capture.binding.generation, 7);
    assert!(app.world().get::<Capture>(row).is_none());
    app.world_mut().trigger(PointerDragEnd {
        entity: label,
        pointer,
        button: PointerButton::Primary,
        distance: Vec2::ZERO,
    });
    app.world_mut().flush();
    assert!(app.world().get::<Capture>(label).is_none());
    assert!(
        app.world().get::<Binding>(row).is_some(),
        "ending a drag retired the source entry"
    );
    Ok(())
}

fn capture(app: &mut App, window: Entity, pointer_id: PointerId) -> Result<Entity, String> {
    let row = app
        .world_mut()
        .spawn((
            ChildOf(window),
            Binding {
                view: ViewId::new(),
                path: PathBuf::new(),
                generation: 7,
                action: Action::Select("marker.glb".into()),
            },
        ))
        .id();
    let label = app.world_mut().spawn(ChildOf(row)).id();
    let pointer = Pointer::new(
        pointer_id,
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Entity(window).normalize(None).ok_or("window")?,
            ),
            position: Vec2::ZERO,
        },
    );
    app.world_mut().trigger(PointerDragStart {
        entity: label,
        pointer,
        button: PointerButton::Primary,
        hit: HitData::new(window, 0., None, None),
    });
    app.world_mut().flush();
    if app.world().get::<Capture>(label).is_none() {
        return Err("Drag did not capture the picked child".into());
    }
    Ok(label)
}

#[test]
fn escape_retires_only_browser_proof_owned_by_the_event_window() -> Result<(), String> {
    let mut app = App::new();
    install(&mut app);
    let window = app.world_mut().spawn(Window::default()).id();
    let foreign_window = app.world_mut().spawn(Window::default()).id();
    let owner = capture(&mut app, window, PointerId::Mouse)?;
    let foreign = capture(&mut app, foreign_window, PointerId::Mouse)?;
    app.world_mut().trigger(FocusedInput::new(
        window,
        KeyboardInput {
            key_code: KeyCode::Escape,
            logical_key: Key::Escape,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        },
        window,
    ));
    app.world_mut().flush();
    assert!(app.world().get::<Capture>(owner).is_none());
    assert!(app.world().get::<Capture>(foreign).is_some());
    assert!(app.world().entities().contains(owner));
    Ok(())
}

#[test]
fn pointer_cancellation_without_hover_keeps_other_window_and_pointer_proofs() -> Result<(), String>
{
    let mut app = App::new();
    install(&mut app);
    let window = app.world_mut().spawn(Window::default()).id();
    let foreign_window = app.world_mut().spawn(Window::default()).id();
    let owner = capture(&mut app, window, PointerId::Mouse)?;
    let other_pointer = capture(&mut app, window, PointerId::Touch(7))?;
    let foreign = capture(&mut app, foreign_window, PointerId::Mouse)?;
    app.world_mut().write_message(PointerInput::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Entity(window).normalize(None).ok_or("window")?,
            ),
            position: Vec2::ZERO,
        },
        PointerAction::Cancel,
    ));
    app.world_mut()
        .run_system_once(pointer_cancel)
        .map_err(|error| error.to_string())?;
    assert!(app.world().get::<Capture>(owner).is_none());
    assert!(app.world().get::<Capture>(other_pointer).is_some());
    assert!(app.world().get::<Capture>(foreign).is_some());
    assert!(app.world().entities().contains(owner));
    Ok(())
}
