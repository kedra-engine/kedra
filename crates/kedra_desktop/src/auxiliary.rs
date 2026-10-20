//! Additional native surfaces share the startup allocator, never replace the main.
use crate::{InitialClientSize, SurfaceKind};
use bevy::{camera::RenderTarget, prelude::*, window::WindowResolution};

/// Explicitly owned native resources. Copying handles never transfers ownership.
/// The caller must move retained children before calling [`retire`].
#[derive(Clone, Copy, Debug)]
pub struct Surface {
    /// Native Bevy window entity.
    pub window: Entity,
    /// Camera rendering only this window.
    pub camera: Entity,
    /// Targeted technical root owning this surface's UI.
    pub root: Entity,
}

/// Stage an invisible additional editor surface on Bevy's main thread.
/// Native creation is asynchronous. Wait for [`ready`] before transferring content.
/// No main window, primary marker, focus, project or document is replaced.
pub fn stage(world: &mut World, title: String, size: UVec2) -> Result<Surface, String> {
    if size.x < 240 || size.y < 192 || size.x > 16384 || size.y > 16384 {
        return Err("Floating window size is outside supported bounds".into());
    }
    Ok(allocate(
        world,
        SurfaceKind::Editor,
        title,
        size,
        Vec2::new(240., 192.),
        false,
    ))
}

pub(crate) fn allocate(
    world: &mut World,
    kind: SurfaceKind,
    title: String,
    size: UVec2,
    minimum: Vec2,
    visible: bool,
) -> Surface {
    let window = world
        .spawn((
            kind,
            InitialClientSize {
                logical: size.as_vec2(),
                created: None,
            },
            Window {
                title,
                resizable: !matches!(kind, SurfaceKind::Boot | SurfaceKind::Loading),
                enabled_buttons: bevy::window::EnabledButtons {
                    minimize: !matches!(kind, SurfaceKind::Boot | SurfaceKind::Loading),
                    maximize: !matches!(kind, SurfaceKind::Boot | SurfaceKind::Loading),
                    close: true,
                },
                visible,
                resolution: WindowResolution::new(size.x, size.y),
                resize_constraints: bevy::window::WindowResizeConstraints {
                    min_width: minimum.x,
                    min_height: minimum.y,
                    ..default()
                },
                position: WindowPosition::Centered(MonitorSelection::Primary),
                // AppKit requires structural native titlebar masks for stable maximize.
                decorations: cfg!(target_os = "macos"),
                fullsize_content_view: cfg!(target_os = "macos"),
                titlebar_transparent: cfg!(target_os = "macos"),
                titlebar_show_title: !cfg!(target_os = "macos"),
                titlebar_show_buttons: true,
                ..default()
            },
        ))
        .id();
    let camera = world
        .spawn((
            Camera2d,
            RenderTarget::Window(bevy::window::WindowRef::Entity(window)),
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb_u8(36, 36, 36)),
                ..default()
            },
        ))
        .id();
    let root = world
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                min_width: px(0),
                min_height: px(0),
                overflow: Overflow::clip(),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            UiTargetCamera(camera),
            bevy::input_focus::tab_navigation::TabGroup::default(),
        ))
        .id();
    Surface {
        window,
        camera,
        root,
    }
}
/// Native backend acknowledgment plus live camera/root. Call on the main thread.
/// This establishes resource readiness, not rendered-frame acceptance.
pub fn ready(world: &World, surface: Surface) -> bool {
    world.get::<Camera>(surface.camera).is_some()
        && world.get::<Node>(surface.root).is_some()
        && bevy::winit::WINIT_WINDOWS
            .with_borrow(|windows| windows.get_window(surface.window).is_some())
}
/// Publish the staged window after mounting and transfer. Retired resources fail.
pub fn show(world: &mut World, surface: Surface) -> Result<(), String> {
    let mut window = world
        .get_mut::<Window>(surface.window)
        .ok_or("Floating window was retired")?;
    if !window.visible {
        window.visible = true;
    }
    Ok(())
}
/// Apply validated restored logical size and physical placement. This cancels the
/// allocator's one-time client-size correction, which must not overwrite restore.
/// Unsupported/missing monitor positions should be centered by the caller.
pub fn restore_placement(
    world: &mut World,
    window: Entity,
    size: UVec2,
    position: WindowPosition,
) -> Result<(), String> {
    if size.x < 240 || size.y < 192 || size.x > 16384 || size.y > 16384 {
        return Err("Restored window size is outside supported bounds".into());
    }
    let mut target = world
        .get_mut::<Window>(window)
        .ok_or("Restored window was retired")?;
    target.resolution.set(size.x as f32, size.y as f32);
    target.position = position;
    world.entity_mut(window).remove::<InitialClientSize>();
    Ok(())
}
/// Retire precisely the owned root, camera and window. Caller detaches content first.
pub fn retire(world: &mut World, surface: Surface) {
    for entity in [surface.root, surface.camera, surface.window] {
        world.despawn(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staging_does_not_replace_main_and_retirement_only_removes_owned_resources() {
        let mut world = World::new();
        world.init_resource::<crate::DesktopSurface>();
        let unrelated = world.spawn_empty().id();
        let surface = stage(&mut world, "Test".into(), UVec2::new(640, 400))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(world.resource::<crate::DesktopSurface>().window.is_none());
        assert!(
            !world
                .get::<Window>(surface.window)
                .is_some_and(|w| w.visible)
        );
        assert!(!ready(&world, surface));
        retire(&mut world, surface);
        assert!(world.entities().contains(unrelated));
        assert!(!world.entities().contains(surface.camera));
        assert!(stage(&mut world, "Invalid".into(), UVec2::ZERO).is_err());
    }
}
