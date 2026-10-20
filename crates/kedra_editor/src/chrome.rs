//! Project-owned visual chrome with native window move/resize actions.
use bevy::{math::CompassOctant, prelude::*, ui_widgets::Activate};
use bevy_flair::style::components::ClassList;
use kedra_ui::authored::view::UiView;

#[derive(Component)]
struct MaximizeGlyph;
#[derive(Component)]
struct RestoreGlyph;
#[derive(Component)]
struct WindowBinding(Entity);
#[derive(Component)]
struct CaptionFrame;

#[derive(Component, Clone, Copy)]
enum Action {
    Minimize,
    Maximize,
    Close,
}
#[derive(Component, Default)]
struct ChromeState {
    maximized: bool,
}
pub(crate) fn install(app: &mut App) {
    app.add_observer(activate)
        .add_systems(Update, (native_state, drag_resize));
}
pub(crate) fn bind(world: &mut World, view: &UiView) -> Result<(), String> {
    let window = world
        .resource::<kedra_desktop::DesktopSurface>()
        .window
        .ok_or("caption has no owned window")?;
    let frame = world.resource::<kedra_desktop::DesktopSurface>().frame;
    bind_window(world, view, window, frame)
}
pub(crate) fn bind_window(
    world: &mut World,
    view: &UiView,
    window: Entity,
    frame: Option<Entity>,
) -> Result<(), String> {
    let title = world
        .get::<Window>(window)
        .ok_or("caption window was retired")?
        .title
        .clone();
    let title_entity = view.element("window.title").map_err(|e| e.to_string())?;
    let controls = [
        ("window.minimize", Action::Minimize),
        ("window.maximize", Action::Maximize),
        ("window.close", Action::Close),
    ]
    .into_iter()
    .map(|(key, action)| view.element(key).map(|entity| (entity, action)))
    .collect::<kedra_scene::Result<Vec<_>>>()
    .map_err(|e| e.to_string())?;
    let maximize = view
        .element("window.maximize_glyph")
        .map_err(|e| e.to_string())?;
    let restore = view
        .element("window.restore_glyph")
        .map_err(|e| e.to_string())?;
    world
        .get_mut::<Text>(title_entity)
        .ok_or("caption title is not native Text")?
        .0 = title;
    for (entity, action) in controls {
        world
            .entity_mut(entity)
            .insert((action, WindowBinding(window)));
    }
    world
        .entity_mut(maximize)
        .insert((MaximizeGlyph, WindowBinding(window)));
    world
        .entity_mut(restore)
        .insert((RestoreGlyph, WindowBinding(window)));
    if let Some(frame) = frame {
        world
            .entity_mut(frame)
            .insert((CaptionFrame, WindowBinding(window)));
    }
    world.entity_mut(window).insert(ChromeState::default());
    Ok(())
}
fn activate(
    event: On<Activate>,
    actions: Query<(&Action, &WindowBinding)>,
    mut windows: Query<(&mut Window, &mut ChromeState)>,
    mut close: MessageWriter<bevy::window::WindowCloseRequested>,
) {
    let Ok((action, binding)) = actions.get(event.entity) else {
        return;
    };
    let entity = binding.0;
    let Ok((mut window, state)) = windows.get_mut(entity) else {
        return;
    };
    match action {
        Action::Minimize => window.set_minimized(true),
        Action::Maximize => {
            window.set_maximized(!state.maximized);
        }
        Action::Close => {
            close.write(bevy::window::WindowCloseRequested { window: entity });
        }
    }
}
type CaptionClasses<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut ClassList,
        Has<MaximizeGlyph>,
        Has<RestoreGlyph>,
        &'static WindowBinding,
        Has<CaptionFrame>,
    ),
>;
fn native_state(
    _main_thread: bevy::ecs::system::NonSendMarker,
    mut windows: Query<(Entity, &mut ChromeState)>,
    mut classes: CaptionClasses<'_, '_>,
) {
    bevy::winit::WINIT_WINDOWS.with_borrow(|native| {
        for (entity, mut state) in &mut windows {
            if let Some(window) = native.get_window(entity) {
                state.maximized = window.is_maximized();
                for (mut list, maximize, restore, target, frame) in &mut classes {
                    if target.0 != entity {
                        continue;
                    }
                    if (maximize || restore)
                        && list.contains("hidden") != (maximize == state.maximized)
                    {
                        list.toggle("hidden");
                    }
                    if frame && list.contains("maximized") != state.maximized {
                        list.toggle("maximized");
                    }
                }
            }
        }
    });
}
fn drag_resize(
    buttons: Res<ButtonInput<MouseButton>>,
    mut windows: Query<(&mut Window, &ChromeState)>,
    modals: Query<(), With<bevy::ui_widgets::ModalDialog>>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    for (mut window, state) in &mut windows {
        if !window.focused {
            continue;
        }
        let Some(p) = window.cursor_position() else {
            return;
        };
        let (w, h) = (window.width(), window.height());
        if !modals.is_empty() || (p.y < 33. && p.x >= w - 133.) {
            return;
        }
        let edge = (!state.maximized && window.resizable)
            .then(|| resize_edge(p, Vec2::new(w, h)))
            .flatten();
        if let Some(edge) = edge {
            // Winit explicitly does not implement drag_resize_window on macOS.
            // The titled/resizable AppKit window owns its native edge tracking.
            if !cfg!(target_os = "macos") {
                window.start_drag_resize(edge);
            }
        } else if p.y < 33. {
            window.start_drag_move();
        }
    }
}
fn resize_edge(point: Vec2, size: Vec2) -> Option<CompassOctant> {
    match (
        point.x < 4.,
        point.x >= size.x - 4.,
        point.y < 4.,
        point.y >= size.y - 4.,
    ) {
        (true, _, true, _) => Some(CompassOctant::NorthWest),
        (_, true, true, _) => Some(CompassOctant::NorthEast),
        (true, _, _, true) => Some(CompassOctant::SouthWest),
        (_, true, _, true) => Some(CompassOctant::SouthEast),
        (true, _, _, _) => Some(CompassOctant::West),
        (_, true, _, _) => Some(CompassOctant::East),
        (_, _, true, _) => Some(CompassOctant::North),
        (_, _, _, true) => Some(CompassOctant::South),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caption_close_targets_its_bound_window_not_the_primary_surface() {
        let mut app = App::new();
        app.add_message::<bevy::window::WindowCloseRequested>()
            .add_observer(activate);
        let main = app
            .world_mut()
            .spawn((Window::default(), ChromeState::default()))
            .id();
        let floating = app
            .world_mut()
            .spawn((Window::default(), ChromeState::default()))
            .id();
        let close = app
            .world_mut()
            .spawn((Action::Close, WindowBinding(floating)))
            .id();
        app.world_mut().trigger(Activate { entity: close });
        let messages = app
            .world()
            .resource::<Messages<bevy::window::WindowCloseRequested>>();
        let closed: Vec<_> = messages
            .get_cursor()
            .read(messages)
            .map(|e| e.window)
            .collect();
        assert_eq!(closed, vec![floating]);
        assert!(app.world().get::<Window>(main).is_some());
    }
    #[test]
    fn native_sync_waits_for_window_backend_without_a_non_send_resource() {
        let mut app = App::new();
        app.init_resource::<kedra_desktop::DesktopSurface>()
            .add_systems(Update, native_state);
        let window = app.world_mut().spawn(ChromeState::default()).id();
        app.update();
        assert!(
            !app.world()
                .get::<ChromeState>(window)
                .is_some_and(|state| state.maximized)
        );
    }
    #[test]
    fn resize_edges_do_not_capture_content() {
        let size = Vec2::new(1120., 720.);
        assert_eq!(
            resize_edge(Vec2::new(0., 0.), size),
            Some(CompassOctant::NorthWest)
        );
        assert_eq!(
            resize_edge(Vec2::new(1119., 719.), size),
            Some(CompassOctant::SouthEast)
        );
        assert_eq!(resize_edge(Vec2::new(40., 40.), size), None);
    }
}
