//! Native surfaces with explicit ownership of a window, camera and UI root.
//!
//! A transition creates the replacement before retiring the previous surface.
//! Closing the active native window exits; closing a retired one is ignored.
use bevy::{
    diagnostic::FrameCount,
    input_focus::InputFocus,
    prelude::*,
    window::{ExitCondition, PrimaryWindow, WindowCloseRequested},
};
pub mod auxiliary;

/// Desktop integration. Install after Bevy's window and input plugins.
pub struct DesktopPlugin;
impl Plugin for DesktopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DesktopSurface>()
            .init_resource::<FrameCount>()
            .add_systems(
                Update,
                (created, initial_client_size, close_requested).chain(),
            );
    }
}

/// Configure Bevy without an implicit window or automatic last-window exit.
/// The lifecycle owner creates the first surface during state entry.
pub fn window_plugin() -> WindowPlugin {
    WindowPlugin {
        primary_window: None,
        exit_condition: ExitCondition::DontExit,
        close_when_requested: false,
        ..default()
    }
}

/// Purpose of a native surface, independent of project or UI data.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceKind {
    /// Early engine bootstrap, before normal editor assets are requested.
    Boot,
    /// Project manager, including modal entry forms.
    Projects,
    /// Project preparation and actionable failures.
    Loading,
    /// Editor host; workspace features are installed separately.
    Editor,
}

/// Handles owned by the desktop lifecycle; unrelated windows are never removed.
#[derive(Resource, Default, Clone, Debug)]
pub struct DesktopSurface {
    /// Active native window.
    pub window: Option<Entity>,
    /// Camera targeting only the active native window.
    pub camera: Option<Entity>,
    /// Root owning the active surface's UI descendants.
    pub root: Option<Entity>,
    /// Authored frame under the camera-targeted technical root, once hydrated.
    pub frame: Option<Entity>,
}

#[derive(Component)]
struct InitialClientSize {
    logical: Vec2,
    created: Option<u32>,
}

fn created(
    mut events: MessageReader<bevy::window::WindowCreated>,
    frame: Res<FrameCount>,
    mut windows: Query<&mut InitialClientSize>,
) {
    for event in events.read() {
        if let Ok(mut size) = windows.get_mut(event.window) {
            size.created = Some(frame.0);
        }
    }
}

fn initial_client_size(
    frame: Res<FrameCount>,
    mut windows: Query<(Entity, &mut Window, &InitialClientSize)>,
    mut commands: Commands,
) {
    for (entity, mut window, size) in &mut windows {
        if size
            .created
            .is_some_and(|created| frame.0.wrapping_sub(created) >= 2)
        {
            // AppKit can report a one-pixel initial content adjustment. Reconcile
            // the requested client size once, never constrain subsequent user resizing.
            if window.size() != size.logical {
                window.resolution.set(size.logical.x, size.logical.y);
            }
            commands.entity(entity).remove::<InitialClientSize>();
        }
    }
}

/// Replace the active surface and return its empty UI root.
///
/// This must run on the main Bevy thread. All descendants of the former root
/// are retired, keyboard focus is cleared and no project resources are removed.
pub fn replace(world: &mut World, kind: SurfaceKind, title: String) -> Entity {
    let (width, height, min_width, min_height) = match kind {
        SurfaceKind::Projects => (1120, 720, 760., 560.),
        SurfaceKind::Boot | SurfaceKind::Loading => (640, 360, 560., 320.),
        SurfaceKind::Editor => (1440, 900, 760., 560.),
    };
    let previous = world.resource::<DesktopSurface>().clone();
    if let Some(window) = previous.window {
        world.entity_mut(window).remove::<PrimaryWindow>();
    }
    let surface = auxiliary::allocate(
        world,
        kind,
        title,
        UVec2::new(width, height),
        Vec2::new(min_width, min_height),
        true,
    );
    let (window, camera, root) = (surface.window, surface.camera, surface.root);
    world.entity_mut(window).insert(PrimaryWindow);
    if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
        focus.clear();
    }
    for entity in [previous.root, previous.camera, previous.window]
        .into_iter()
        .flatten()
    {
        world.despawn(entity);
    }
    *world.resource_mut::<DesktopSurface>() = DesktopSurface {
        window: Some(window),
        camera: Some(camera),
        root: Some(root),
        frame: None,
    };
    root
}

fn close_requested(
    mut requests: MessageReader<WindowCloseRequested>,
    surface: Res<DesktopSurface>,
    mut exit: MessageWriter<AppExit>,
) {
    if requests
        .read()
        .any(|request| Some(request.window) == surface.window)
    {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "Isolated Bevy world assertions")]
    use super::*;
    use bevy::window::WindowResolution;
    #[test]
    fn replacement_owns_only_its_window_camera_and_tree() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<InputFocus>()
            .add_message::<WindowCloseRequested>()
            .add_message::<bevy::window::WindowCreated>()
            .add_plugins(DesktopPlugin);
        let other = app.world_mut().spawn_empty().id();
        let first = replace(app.world_mut(), SurfaceKind::Projects, "Projects".into());
        assert!(app.world().get::<Node>(first).is_some());
        assert!(
            app.world()
                .get::<bevy::input_focus::tab_navigation::TabGroup>(first)
                .is_some()
        );
        let child = app.world_mut().spawn(ChildOf(first)).id();
        let old = app.world().resource::<DesktopSurface>().clone();
        let native = app.world().get::<Window>(old.window.unwrap()).unwrap();
        assert_eq!(native.decorations, cfg!(target_os = "macos"));
        if cfg!(target_os = "macos") {
            assert!(
                native.fullsize_content_view
                    && native.titlebar_transparent
                    && native.titlebar_shown
            );
            assert!(!native.titlebar_show_title && !native.titlebar_show_buttons);
        }
        let second = replace(app.world_mut(), SurfaceKind::Loading, "Loading".into());
        assert!(app.world().get_entity(first).is_err());
        assert!(app.world().get_entity(child).is_err());
        assert!(app.world().get_entity(old.camera.unwrap()).is_err());
        assert!(app.world().get_entity(old.window.unwrap()).is_err());
        assert!(app.world().get_entity(other).is_ok());
        assert_eq!(app.world().resource::<DesktopSurface>().root, Some(second));
        app.world_mut().write_message(WindowCloseRequested {
            window: old.window.unwrap(),
        });
        app.update();
        assert_eq!(app.world().resource::<Messages<AppExit>>().len(), 0);
        let window = app.world().resource::<DesktopSurface>().window.unwrap();
        app.world_mut()
            .write_message(WindowCloseRequested { window });
        app.update();
        assert!(!app.world().resource::<Messages<AppExit>>().is_empty());
    }

    #[test]
    fn initial_client_adjustment_is_reconciled_once_without_locking_user_resize() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<FrameCount>();
        world.resource_mut::<FrameCount>().0 = 2;
        let entity = world
            .spawn((
                Window {
                    resolution: WindowResolution::new(640, 361),
                    ..default()
                },
                InitialClientSize {
                    logical: Vec2::new(640., 360.),
                    created: Some(0),
                },
            ))
            .id();
        world.run_system_once(initial_client_size).unwrap();
        assert_eq!(
            world.get::<Window>(entity).unwrap().size(),
            Vec2::new(640., 360.)
        );
        assert!(world.get::<InitialClientSize>(entity).is_none());
        world
            .get_mut::<Window>(entity)
            .unwrap()
            .resolution
            .set(900., 600.);
        world.run_system_once(initial_client_size).unwrap();
        assert_eq!(
            world.get::<Window>(entity).unwrap().size(),
            Vec2::new(900., 600.)
        );
    }
}
