use super::*;
use crate::{model::*, views};

#[test]
fn source_dependencies_load_only_after_css_plugin_registration_is_complete() {
    let mut app = app();
    assert_eq!(app.plugins_state(), bevy::app::PluginsState::Cleaned);
    assert!(
        app.world()
            .resource::<bevy_flair::core::CssPropertyRegistry>()
            .get_shorthand("border")
            .is_some_and(|properties| !properties.is_empty())
    );
    let sources = app.world().resource::<PrefabSources>().snapshot();
    assert!(!sources.compiled.is_empty());
    assert!(sources.compiled.values().all(|compiled| {
        compiled
            .asset_status(app.world().resource::<AssetServer>())
            .unwrap()
    }));
}

#[test]
fn full_plugin_hydrates_boot_then_retires_owned_pending_views_on_project_transition() {
    let mut app = app();
    app.update();
    app.update();
    app.update();
    let version = element(app.world_mut(), "boot.version");
    assert_eq!(
        app.world().get::<Text>(version).unwrap().0,
        env!("CARGO_PKG_VERSION")
    );
    let old = app
        .world()
        .resource::<kedra_desktop::DesktopSurface>()
        .clone();
    let pending =
        views::request(app.world_mut(), old.root.unwrap(), views::Kind::Projects).unwrap();
    projects(&mut app);
    for entity in [
        old.root.unwrap(),
        old.window.unwrap(),
        old.camera.unwrap(),
        pending,
    ] {
        assert!(!app.world().entities().contains(entity));
    }
    assert!(app.world().resource::<ProjectManager>().project().is_none());
    assert_eq!(app.world().resource::<Messages<AppExit>>().len(), 0);
}

#[test]
fn full_plugin_reports_source_failure_without_mounting_a_fallback() {
    let mut app = app();
    app.world_mut().resource_mut::<PrefabSources>().error = Some("fixture source rejected".into());
    app.update();
    assert_failure(app.world(), "fixture source rejected");
    assert_eq!(
        app.world_mut()
            .query::<&kedra_ui::ScreenId>()
            .iter(app.world())
            .count(),
        0
    );
}

#[test]
fn pending_sources_timeout_is_exact_and_missing_document_is_terminal() {
    let mut app = app();
    app.world_mut().resource_mut::<PrefabSources>().pending = true;
    let root = app.world_mut().spawn_empty().id();
    views::request(app.world_mut(), root, views::Kind::Projects).unwrap();
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_secs(30));
    views::hydrate(app.world_mut());
    assert!(
        app.world()
            .resource::<kedra_ui::authored::components::UiFailure>()
            .0
            .is_none()
    );
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_nanos(1));
    views::hydrate(app.world_mut());
    assert_failure(app.world(), "within 30 seconds");

    let mut app = super::app();
    app.insert_resource(PrefabSources::default());
    app.world_mut()
        .resource_mut::<PrefabSources>()
        .dependencies_ready = true;
    let root = app.world_mut().spawn_empty().id();
    views::request(app.world_mut(), root, views::Kind::Boot).unwrap();
    views::hydrate(app.world_mut());
    assert_failure(app.world(), "document is absent");
}

#[test]
fn library_run_rejects_missing_delivery_before_native_plugin_initialization() {
    let directory = tempfile::tempdir().unwrap();
    let error = crate::run(None, directory.path().join("missing"), None, None, None).unwrap_err();
    assert!(error.contains("Cannot access asset delivery"));
}
