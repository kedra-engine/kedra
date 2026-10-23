//! Exercise real authored binders, then fail desktop publication after mount succeeds.
use super::*;

#[test]
fn post_mount_camera_and_window_failures_retire_all_scene_bindings() -> Result<(), String> {
    let mut app = crate::test_support::diagnostic_app();
    let previous_root = kedra_desktop::replace(
        app.world_mut(),
        kedra_desktop::SurfaceKind::Projects,
        "Previous".into(),
    );
    let previous = app
        .world()
        .resource::<kedra_desktop::DesktopSurface>()
        .clone();
    let focused = app.world_mut().spawn(ChildOf(previous_root)).id();
    app.world_mut()
        .resource_mut::<bevy::input_focus::InputFocus>()
        .set(focused, bevy::input_focus::FocusCause::Navigated);
    let previous_focus = app
        .world()
        .resource::<bevy::input_focus::InputFocus>()
        .clone();
    for path in [
        "editor://fixtures/catalog.scene.ron",
        "editor://fixtures/docking.scene.ron",
        "editor://fixtures/inspector.scene.ron",
    ] {
        let scene = crate::views::compiled(app.world(), path)?;
        for remove_window in [false, true] {
            let before: std::collections::BTreeSet<Entity> = app
                .world_mut()
                .query::<Entity>()
                .iter(app.world())
                .collect();
            let canvas = Cell::new(None);
            let mounted = Cell::new(false);
            let candidate = Cell::new(None);
            let binding_entities = Cell::new([None; 3]);
            let reduced = app
                .world()
                .resource::<kedra_ui::authored::components::UiMotion>()
                .reduced;
            let result = kedra_desktop::replace_mounted(
                app.world_mut(),
                kedra_desktop::SurfaceKind::Editor,
                "Candidate".into(),
                UVec2::new(1200, 800),
                |world, native| {
                    candidate.set(Some(native));
                    bind_scene(world, &scene, native, &canvas)?;
                    binding_entities.set([
                        world.resource_entity::<crate::workspace::EditorWorkspace>(),
                        crate::workspace::fixture::binding_entity(world),
                        values::binding_entity(world),
                    ]);
                    mounted.set(true);
                    world.despawn(if remove_window {
                        native.window
                    } else {
                        native.camera
                    });
                    Ok(())
                },
                |world, native| retire_bindings(world, native, canvas.get(), reduced),
            );
            assert!(
                mounted.get(),
                "Scene never reached post-mount validation: {path}: {result:?}"
            );
            assert!(result.is_err());
            // Bevy 0.20 retains a cached IsResource entity after remove_resource.
            // Only these exact binding slots may be added; their values must be gone.
            let mut expected = before;
            for entity in binding_entities.get().into_iter().flatten() {
                let world = app.world();
                assert!(
                    world
                        .get::<bevy::ecs::resource::IsResource>(entity)
                        .is_some()
                );
                assert_eq!(
                    world
                        .inspect_entity(entity)
                        .map_err(|error| error.to_string())?
                        .count(),
                    1,
                    "Binding resource value or candidate components remain on {entity:?}"
                );
                expected.insert(entity);
            }
            // Include authored descendants and observer entities, not just native handles.
            // Check before and after flush so queued work cannot hide a leak or recreate content.
            for flush in [false, true] {
                let world = app.world_mut();
                if flush {
                    world.flush();
                }
                let live: std::collections::BTreeSet<Entity> =
                    world.query::<Entity>().iter(world).collect();
                for entity in live.difference(&expected) {
                    let components = world
                        .inspect_entity(*entity)
                        .map_err(|error| error.to_string())?
                        .map(|(_, info)| info.name().to_string())
                        .collect::<Vec<_>>();
                    eprintln!("Unexpected candidate entity {entity:?}: {components:?}");
                }
                assert_eq!(live, expected, "Candidate entity leak: {path}");
                assert_eq!(world.entities().count_spawned() as usize, expected.len());
                assert_eq!(
                    world.resource::<bevy::input_focus::InputFocus>(),
                    &previous_focus
                );
            }
            let world = app.world();
            assert!(!crate::catalog::has_binding(world));
            assert!(!crate::workspace::fixture::has_binding(world));
            assert!(!world.contains_resource::<crate::workspace::EditorWorkspace>());
            assert!(!values::has_binding(world));
            assert_eq!(
                world.resource::<kedra_desktop::DesktopSurface>().window,
                previous.window
            );
            assert_eq!(
                world.resource::<kedra_desktop::DesktopSurface>().root,
                previous.root
            );
            assert_eq!(
                world.resource::<kedra_desktop::DesktopSurface>().camera,
                previous.camera
            );
            if let Some(window) = previous.window {
                assert!(world.get::<bevy::window::PrimaryWindow>(window).is_some());
            }
            assert_eq!(
                world.resource::<bevy::input_focus::InputFocus>(),
                &previous_focus
            );
            assert_eq!(
                world
                    .resource::<kedra_ui::authored::components::UiMotion>()
                    .reduced,
                reduced
            );
            if let Some(native) = candidate.get() {
                for entity in [native.root, native.camera, native.window] {
                    assert!(!world.entities().contains(entity));
                }
            }
        }
    }
    Ok(())
}
