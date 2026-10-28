//! Scene-driven debug entry, sharing the ordinary editor's native capabilities.
#[cfg(test)]
#[path = "diagnostics/lifecycle_tests.rs"]
mod lifecycle_tests;
mod values;
use bevy::prelude::*;
use kedra_scene::{instance::CompiledPrefab, reflection::ComponentPolicies};
use kedra_ui::authored::view::UiView;
use std::{cell::Cell, collections::BTreeMap, sync::Arc};

#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
struct Surface {
    width: u32,
    height: u32,
}
pub(crate) fn install(app: &mut App) -> Result<(), String> {
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    {
        let mut policies = app.world_mut().resource_mut::<ComponentPolicies>();
        let mut types = registry.write();
        policies
            .register::<Surface>(&mut types, "editor.surface", 1, |value| {
                let surface = Surface::from_reflect(value)
                    .ok_or_else(|| kedra_scene::Error("Invalid scene surface".into()))?;
                if !(64..=8192).contains(&surface.width) || !(64..=8192).contains(&surface.height) {
                    return Err(kedra_scene::Error(
                        "Scene surface size must be 64..=8192".into(),
                    ));
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        policies
            .register::<crate::catalog::Canvas>(
                &mut types,
                "editor.catalog",
                1,
                crate::catalog::validate_canvas,
            )
            .map_err(|e| e.to_string())?;
        policies
            .register::<values::Settings>(&mut types, "editor.values", 1, |_| Ok(()))
            .map_err(|e| e.to_string())?;
        policies
            .register::<crate::workspace::fixture::OverflowSource>(
                &mut types,
                "editor.dock_overflow",
                1,
                crate::workspace::fixture::validate_overflow,
            )
            .map_err(|e| e.to_string())?;
    }
    crate::catalog::install(app);
    crate::workspace::fixture::install(app);
    values::install(app);
    app.add_systems(
        Update,
        (|startup: Res<kedra_scene::startup::StartupScene>,
          keys: Res<ButtonInput<KeyCode>>,
          mut exit: MessageWriter<AppExit>| {
            if let kedra_scene::startup::StartupState::Failed(error) = startup.state() {
                error!("Startup scene failed: {error}");
                exit.write(AppExit::error());
            } else if keys.just_pressed(KeyCode::F12) {
                exit.write(AppExit::Success);
            }
        })
        .run_if(resource_exists::<kedra_scene::startup::StartupScene>),
    );
    Ok(())
}

pub(crate) fn mount(world: &mut World, scene: &Arc<CompiledPrefab>) -> kedra_scene::Result<Entity> {
    let source = scene
        .source_entities()
        .get(&scene.graph().root)
        .ok_or_else(|| kedra_scene::Error("Scene root missing".into()))?;
    let size = scene
        .authoring_world()
        .get::<Surface>(*source)
        .map_or(UVec2::new(640, 360), |surface| {
            UVec2::new(surface.width, surface.height)
        });
    let canvas = Cell::new(None);
    let reduced_motion = world
        .resource::<kedra_ui::authored::components::UiMotion>()
        .reduced;
    kedra_desktop::replace_mounted(
        world,
        kedra_desktop::SurfaceKind::Editor,
        "Kedra".into(),
        size,
        |world, native| bind_scene(world, scene, native, &canvas),
        |world, native| retire_bindings(world, native, canvas.get(), reduced_motion),
    )
    .map_err(kedra_scene::Error)
}

fn bind_scene(
    world: &mut World,
    scene: &Arc<CompiledPrefab>,
    native: kedra_desktop::auxiliary::Surface,
    canvas: &Cell<Option<Entity>>,
) -> Result<(), String> {
    let source = scene
        .source_entities()
        .get(&scene.graph().root)
        .ok_or("Scene root missing")?;
    let scoped_values = scene
        .authoring_world()
        .get::<values::Settings>(*source)
        .is_some();
    let view = UiView::mount(
        world,
        scene,
        Some(native.root),
        &BTreeMap::new(),
        if scoped_values { "fixture" } else { "" },
        1.,
    )
    .map_err(|e| e.to_string())?;
    let root = view.root;
    canvas.set(Some(root));
    if world.get::<crate::catalog::Canvas>(root).is_some() {
        crate::catalog::bind(world, &view).map_err(|e| e.to_string())?;
    }
    if world
        .get::<crate::workspace::fixture::OverflowSource>(root)
        .is_some()
    {
        crate::workspace::fixture::bind(world, &view, native.window, native.root)?;
    }
    if scoped_values {
        values::bind(world, view)?;
    }
    Ok(())
}

fn retire_bindings(
    world: &mut World,
    native: kedra_desktop::auxiliary::Surface,
    canvas: Option<Entity>,
    reduced_motion: bool,
) {
    if let Some(root) = canvas {
        crate::catalog::retire_binding(world, root);
        values::retire_binding(world, root);
    }
    crate::workspace::fixture::retire_binding(world, native.root);
    world
        .resource_mut::<kedra_ui::authored::components::UiMotion>()
        .reduced = reduced_motion;
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    #[test]
    fn explicit_scene_keeps_editor_capabilities_but_does_not_enter_project_screens() {
        let mut app = crate::test_support::app();
        app.world_mut().resource_mut::<crate::ExplicitScene>().0 = true;
        app.world_mut()
            .resource_mut::<NextState<crate::AppPhase>>()
            .set(crate::AppPhase::Projects);
        app.update();
        assert!(
            app.world()
                .resource::<kedra_desktop::DesktopSurface>()
                .window
                .is_none()
        );
        assert!(!app.world().contains_resource::<crate::jobs::PendingJob>());
        assert!(
            app.world()
                .contains_resource::<crate::model::ProjectManager>()
        );
        assert!(
            app.world()
                .contains_resource::<crate::actions::native::Pending>()
        );
    }

    #[test]
    fn explicit_scene_rejects_concurrent_project_startup_before_asset_io() {
        for (project, delivery) in [
            (Some("project".into()), None),
            (None, Some("delivery".into())),
        ] {
            let result = crate::run(
                project,
                "missing-assets".into(),
                kedra_app::launch::LaunchOptions {
                    scene: Some("editor://fixtures/catalog.scene.ron".into()),
                    ..Default::default()
                },
                None,
                delivery,
                None,
            );
            assert!(result.is_err_and(|error| error.contains("concurrently")));
        }
    }
}
