//! Controls must never advertise commands without a usable spatial camera.
use super::*;
use bevy::asset::AssetApp;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture() -> (App, View) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_resource::<ComponentPolicies>()
        .init_asset::<kedra_scene::document::PrefabDocument>();
    let world = app.world_mut();
    let element = world.spawn(Text::default()).id();
    let controls = (0..4).map(|_| world.spawn_empty().id()).collect();
    let view = View {
        root: element,
        viewport: element,
        render: element,
        information: element,
        heading: element,
        controls,
        projection_label: element,
        lighting_label: element,
        lighting_button: element,
        grid_button: element,
        marker: default(),
        compiled: None,
        projection: None,
        lifecycle: default(),
        elapsed: 0.,
        raw_model: false,
    };
    (app, view)
}

fn disabled(world: &World, view: &View) {
    for button in &view.controls {
        assert!(
            world
                .get::<bevy::ui::InteractionDisabled>(*button)
                .is_some()
        );
    }
}

fn spatial(world: &mut World, view: &mut View) -> Result<Entity, String> {
    let compiled = tests::compiled(world).map_err(|e| e.to_string())?;
    let mut projection = projection::Projection::pending(1);
    projection.mount(world, compiled.clone())?;
    let camera = projection.camera().ok_or("Camera missing")?;
    view.compiled = Some(compiled);
    view.projection = Some(projection);
    view.lifecycle.begin()?;
    view.lifecycle.instantiated();
    view.lifecycle.frame();
    Ok(camera)
}

#[test]
fn controls_disable_on_unbound_failed_and_retired_tick_exits() -> TestResult {
    let (mut app, mut view) = fixture();
    for phase in [
        lifecycle::Phase::Unbound,
        lifecycle::Phase::Failed,
        lifecycle::Phase::Retired,
    ] {
        view.lifecycle.phase = phase;
        for button in &view.controls {
            app.world_mut()
                .entity_mut(*button)
                .remove::<bevy::ui::InteractionDisabled>();
        }
        view.tick(app.world_mut(), true, 1.)?;
        disabled(app.world(), &view);
    }
    Ok(())
}

#[test]
fn controls_disable_while_stage_pending_and_on_tick_error_after_readiness() -> TestResult {
    let (mut app, mut view) = fixture();
    spatial(app.world_mut(), &mut view)?;
    // First stage load is pending; the previous camera must not keep tools enabled.
    view.tick(app.world_mut(), true, 1.)?;
    disabled(app.world(), &view);
    view.elapsed = 31.;
    assert!(view.tick(app.world_mut(), true, 1.).is_err());
    disabled(app.world(), &view);
    assert_eq!(view.lifecycle.phase, lifecycle::Phase::Failed);
    Ok(())
}

#[test]
fn failed_and_retired_controls_are_disabled_immediately_and_commands_do_nothing() -> TestResult {
    let (mut app, mut view) = fixture();
    let camera = spatial(app.world_mut(), &mut view)?;
    let grid = view
        .projection
        .as_ref()
        .and_then(|p| p.grid)
        .ok_or("Grid missing")?;
    let before = *app
        .world()
        .get::<Visibility>(grid)
        .ok_or("Visibility missing")?;
    view.message(app.world_mut(), "Renderer failed after readiness", true);
    disabled(app.world(), &view);
    assert!(view.navigation(app.world()).is_none());
    view.grid(app.world_mut());
    view.light(app.world_mut(), true);
    assert_eq!(app.world().get::<Visibility>(grid), Some(&before));
    assert_eq!(
        view.lifecycle.diagnostic.as_deref(),
        Some("Renderer failed after readiness")
    );
    for button in &view.controls {
        app.world_mut()
            .entity_mut(*button)
            .remove::<bevy::ui::InteractionDisabled>();
    }
    view.retire(app.world_mut());
    disabled(app.world(), &view);
    assert!(app.world().get::<Camera>(camera).is_none());
    view.grid(app.world_mut());
    view.light(app.world_mut(), false);
    Ok(())
}

#[test]
fn asset_rig_failure_retires_a_previously_ready_projection() -> TestResult {
    let (mut app, mut view) = fixture();
    let camera = spatial(app.world_mut(), &mut view)?;
    let source = kedra_authoring::persistence::SourceSnapshot {
        namespace: "game".into(),
        generation: 1,
        index: None,
        documents: vec![],
        assets: vec![],
    };
    assert!(
        view.prepare_asset(app.world_mut(), "unsupported.txt", &source, 1)
            .is_err()
    );
    disabled(app.world(), &view);
    assert_eq!(view.lifecycle.phase, lifecycle::Phase::Failed);
    assert!(app.world().get::<Camera>(camera).is_none());
    Ok(())
}
