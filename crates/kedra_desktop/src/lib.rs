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
pub mod chrome;

/// Host-owned temporary veto of ordinary main-window close requests. Desktop
/// neither owns dirty documents nor decides how the host resolves this veto.
#[derive(Component)]
pub struct CloseBlocked;
/// Native close admission milestone. Hosts update `CloseBlocked` before this set
/// from their current source/input state, not a previous-frame approximation.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CloseAdmission;

/// Desktop integration. Install after Bevy's window and input plugins.
pub struct DesktopPlugin;
impl Plugin for DesktopPlugin {
    fn build(&self, app: &mut App) {
        chrome::install(app);
        app.init_resource::<DesktopSurface>()
            .init_resource::<FrameCount>()
            .add_systems(
                Update,
                (
                    created,
                    initial_client_size,
                    close_requested.in_set(CloseAdmission),
                )
                    .chain(),
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
    let surface = auxiliary::allocate(
        world,
        kind,
        title,
        UVec2::new(width, height),
        Vec2::new(min_width, min_height),
        true,
    );
    publish(world, surface);
    surface.root
}

/// Mount content into a new primary surface before retiring the active surface.
///
/// Requires DesktopPlugin and runs on the main Bevy thread. Size must be
/// 64..=8192 logical pixels per axis. Callbacks must preserve the previous surface
/// and focus; they may mutate only candidate-owned presentation and bindings.
/// The mount callback owns descendants of the candidate root. If mount returns an
/// error or validation/visibility fails, `rollback` runs exactly once before candidate
/// retirement, including when mount returned success. It must remove its candidate's
/// non-entity bindings without touching the previous surface or unrelated state.
/// Invalid size rejects before either callback runs. Failure retires the candidate
/// tree/camera/window, leaving the previous primary surface and focus unchanged.
/// Success publishes the candidate and clears focus. Native acknowledgment/frame
/// readiness is separate. This error rollback contract does not cover panics.
pub fn replace_mounted(
    world: &mut World,
    kind: SurfaceKind,
    title: String,
    size: UVec2,
    mount: impl FnOnce(&mut World, auxiliary::Surface) -> Result<(), String>,
    rollback: impl FnOnce(&mut World, auxiliary::Surface),
) -> Result<Entity, String> {
    if !(64..=8192).contains(&size.x) || !(64..=8192).contains(&size.y) {
        return Err("Primary surface size must be 64..=8192".into());
    }
    let surface = auxiliary::allocate(world, kind, title, size, Vec2::splat(64.), false);
    let result = mount(world, surface).and_then(|()| {
        if world.get::<Node>(surface.root).is_none()
            || world.get::<Camera>(surface.camera).is_none()
        {
            return Err("Mounted primary surface was retired before publication".into());
        }
        auxiliary::show(world, surface)
    });
    if let Err(error) = result {
        rollback(world, surface);
        auxiliary::retire(world, surface);
        return Err(error);
    }
    publish(world, surface);
    Ok(surface.root)
}

fn publish(world: &mut World, surface: auxiliary::Surface) {
    let previous = world.resource::<DesktopSurface>().clone();
    if let Some(window) = previous.window {
        world.entity_mut(window).remove::<PrimaryWindow>();
    }
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
}

fn close_requested(
    mut requests: MessageReader<WindowCloseRequested>,
    surface: Res<DesktopSurface>,
    mut exit: MessageWriter<AppExit>,
    blocked: Query<(), With<CloseBlocked>>,
) {
    if requests
        .read()
        .any(|request| Some(request.window) == surface.window && !blocked.contains(request.window))
    {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "Isolated Bevy world assertions")]
    use super::*;
    #[test]
    fn host_close_veto_preserves_main_window_until_explicitly_released() {
        let mut app = App::new();
        app.add_message::<WindowCloseRequested>()
            .add_message::<AppExit>()
            .add_systems(Update, close_requested);
        let window = app
            .world_mut()
            .spawn((Window::default(), CloseBlocked))
            .id();
        app.insert_resource(DesktopSurface {
            window: Some(window),
            ..default()
        });
        app.world_mut()
            .write_message(WindowCloseRequested { window });
        app.update();
        assert!(app.world().resource::<Messages<AppExit>>().is_empty());
        assert!(app.world().entities().contains(window));
        app.world_mut().entity_mut(window).remove::<CloseBlocked>();
        app.world_mut()
            .write_message(WindowCloseRequested { window });
        app.update();
        assert!(!app.world().resource::<Messages<AppExit>>().is_empty());
    }
    use bevy::window::WindowResolution;

    fn live_entities(world: &mut World) -> std::collections::BTreeSet<Entity> {
        world.query::<Entity>().iter(world).collect()
    }

    fn assert_rollback_preserved(
        world: &mut World,
        before: &std::collections::BTreeSet<Entity>,
        previous: &DesktopSurface,
        focus: &InputFocus,
    ) {
        // Bevy 0.20 Entities::len counts retained allocator metadata, not live entities.
        // Check immediately as well as after flushing: cleanup must not need a later frame.
        for flush in [false, true] {
            if flush {
                world.flush();
            }
            assert_eq!(world.entities().count_spawned() as usize, before.len());
            assert_eq!(&live_entities(world), before);
            let current = world.resource::<DesktopSurface>();
            assert_eq!(current.window, previous.window);
            assert_eq!(current.camera, previous.camera);
            assert_eq!(current.root, previous.root);
            assert_eq!(current.frame, previous.frame);
            assert_eq!(world.resource::<InputFocus>(), focus);
            assert!(
                world
                    .get::<PrimaryWindow>(previous.window.unwrap())
                    .is_some()
            );
        }
    }

    #[test]
    fn mounted_replacement_failure_preserves_previous_primary_and_focus() {
        let mut world = World::new();
        world.init_resource::<DesktopSurface>();
        world.init_resource::<InputFocus>();
        let previous = replace(&mut world, SurfaceKind::Projects, "Previous".into());
        let focus = world.spawn(ChildOf(previous)).id();
        world
            .resource_mut::<InputFocus>()
            .set(focus, bevy::input_focus::FocusCause::Navigated);
        let previous_surface = world.resource::<DesktopSurface>().clone();
        let previous_focus = world.resource::<InputFocus>().clone();
        let before = live_entities(&mut world);
        let mut candidate = None;
        let mut child_and_observer = None;
        let mut rollbacks = 0;
        let result = replace_mounted(
            &mut world,
            SurfaceKind::Editor,
            "Candidate".into(),
            UVec2::new(420, 740),
            |world, surface| {
                candidate = Some(surface);
                let child = world.spawn(ChildOf(surface.root)).id();
                let observer = world
                    .spawn(Observer::new(|_: On<Despawn<ChildOf>>| {}).with_entity(child))
                    .id();
                child_and_observer = Some((child, observer));
                assert_eq!(world.resource::<DesktopSurface>().root, Some(previous));
                Err("Rejected content".into())
            },
            |_, _| rollbacks += 1,
        );
        assert_eq!(result, Err("Rejected content".into()));
        assert_eq!(rollbacks, 1);
        assert_rollback_preserved(&mut world, &before, &previous_surface, &previous_focus);
        let candidate = candidate.unwrap();
        let (child, observer) = child_and_observer.unwrap();
        for entity in [
            candidate.root,
            candidate.window,
            candidate.camera,
            child,
            observer,
        ] {
            assert!(!world.entities().contains(entity));
        }
        assert!(
            replace_mounted(
                &mut world,
                SurfaceKind::Editor,
                "Invalid".into(),
                UVec2::ZERO,
                |_, _| panic!("Invalid size ran mount"),
                |_, _| panic!("Invalid size ran rollback")
            )
            .is_err()
        );
        assert_rollback_preserved(&mut world, &before, &previous_surface, &previous_focus);
        let result = replace_mounted(
            &mut world,
            SurfaceKind::Editor,
            "Retired candidate".into(),
            UVec2::new(420, 740),
            |world, surface| {
                world.despawn(surface.root);
                Ok(())
            },
            |_, _| rollbacks += 1,
        );
        assert!(result.is_err());
        assert_eq!(rollbacks, 2);
        assert_rollback_preserved(&mut world, &before, &previous_surface, &previous_focus);
    }

    #[test]
    fn mounted_replacement_publishes_complete_tree_and_retires_only_previous() {
        let mut world = World::new();
        world.init_resource::<DesktopSurface>();
        world.init_resource::<InputFocus>();
        let previous = replace(&mut world, SurfaceKind::Projects, "Previous".into());
        let unrelated = world.spawn_empty().id();
        let root = replace_mounted(
            &mut world,
            SurfaceKind::Editor,
            "Content".into(),
            UVec2::new(420, 740),
            |world, surface| {
                assert!(!world.get::<Window>(surface.window).unwrap().visible);
                world.spawn((Name::new("Mounted child"), ChildOf(surface.root)));
                Ok(())
            },
            |_, _| panic!("Successful mount ran rollback"),
        )
        .unwrap();
        let surface = world.resource::<DesktopSurface>();
        assert_eq!(surface.root, Some(root));
        assert_eq!(
            world.get::<Window>(surface.window.unwrap()).unwrap().size(),
            Vec2::new(420., 740.)
        );
        assert!(
            world
                .get::<Window>(surface.window.unwrap())
                .unwrap()
                .visible
        );
        assert!(
            world
                .get::<PrimaryWindow>(surface.window.unwrap())
                .is_some()
        );
        assert!(!world.entities().contains(previous));
        assert!(world.entities().contains(unrelated));
        assert_eq!(world.get::<Children>(root).unwrap().len(), 1);
    }
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
            assert!(!native.titlebar_show_title && native.titlebar_show_buttons);
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
