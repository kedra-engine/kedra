//! Controls must never advertise commands without a usable spatial camera.
use super::test_support::{empty, spatial};
use super::*;
use bevy::asset::AssetApp;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture() -> (App, View) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_resource::<ComponentPolicies>()
        .init_asset::<kedra_scene::document::PrefabDocument>();
    let view = empty(app.world_mut());
    (app, view)
}

#[test]
fn visible_serving_preview_stays_continuous_and_hidden_failed_or_retired_views_release_it()
-> TestResult {
    use bevy::winit::{UpdateMode, WinitSettings};
    use std::time::Duration;
    let (mut app, mut view) = fixture();
    app.add_plugins(kedra_wakeup::ReactivePlugin {
        idle: Duration::from_millis(100),
        focused_active: Duration::from_millis(16),
        unfocused_active: Duration::from_millis(100),
    });
    spatial(app.world_mut(), &mut view)?;
    // The fixture is already frame-ready. Readiness must not authorize idle.
    assert_eq!(view.lifecycle.phase, lifecycle::Phase::Ready);
    view.tick(app.world_mut(), true, 1.)?;
    app.world_mut().run_schedule(Last);
    assert_eq!(
        app.world().resource::<WinitSettings>().focused_mode,
        UpdateMode::Continuous
    );
    view.tick(app.world_mut(), false, 1.)?;
    app.world_mut().run_schedule(Last);
    assert_eq!(
        app.world().resource::<WinitSettings>().focused_mode,
        UpdateMode::reactive_low_power(Duration::from_millis(100))
    );
    view.tick(app.world_mut(), true, 1.)?;
    app.world_mut().run_schedule(Last);
    assert_eq!(
        app.world().resource::<WinitSettings>().focused_mode,
        UpdateMode::Continuous
    );
    view.message(app.world_mut(), "Preview failed", true);
    view.tick(app.world_mut(), true, 1.)?;
    app.world_mut().run_schedule(Last);
    assert_eq!(
        app.world().resource::<WinitSettings>().focused_mode,
        UpdateMode::reactive_low_power(Duration::from_millis(100))
    );
    view.retire(app.world_mut());
    view.tick(app.world_mut(), true, 1.)?;
    app.world_mut().run_schedule(Last);
    assert_eq!(
        app.world().resource::<WinitSettings>().unfocused_mode,
        UpdateMode::reactive_low_power(Duration::from_millis(100))
    );
    Ok(())
}

fn disabled(world: &World, view: &View) {
    for button in &view.controls.buttons {
        assert!(
            world
                .get::<bevy::ui::InteractionDisabled>(*button)
                .is_some()
        );
    }
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
        for button in &view.controls.buttons {
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
    for button in &view.controls.buttons {
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

#[test]
fn hiding_overlays_keeps_selection_bounds_and_other_views_unchanged() -> TestResult {
    let (mut app, mut view) = fixture();
    let camera = spatial(app.world_mut(), &mut view)?;
    let bounds = Some((Vec3::ZERO, Vec3::ONE));
    app.world_mut()
        .get_mut::<camera::Navigation>(camera)
        .ok_or("Navigation missing")?
        .selection = bounds;
    assert_eq!(view.snapshot(app.world())["overlays"], true);
    view.toggle_overlays(app.world_mut());
    assert_eq!(view.snapshot(app.world())["overlays"], false);
    assert_eq!(
        app.world()
            .get::<camera::Navigation>(camera)
            .ok_or("Navigation missing")?
            .selection,
        bounds
    );
    camera::command(app.world_mut(), camera, camera::Command::FrameSelection)?;
    view.toggle_overlays(app.world_mut());
    assert_eq!(view.snapshot(app.world())["overlays"], true);
    let (_, other) = fixture();
    assert!(other.marker.is_visible());
    Ok(())
}

#[test]
fn ready_ui_preview_can_toggle_bounds_without_advertising_a_spatial_camera() -> TestResult {
    let (mut app, mut view) = fixture();
    let camera = spatial(app.world_mut(), &mut view)?;
    app.world_mut()
        .entity_mut(camera)
        .remove::<camera::Navigation>();
    assert!(view.navigation(app.world()).is_none());
    view.toggle_overlays(app.world_mut());
    assert_eq!(view.snapshot(app.world())["overlays"], false);
    Ok(())
}
