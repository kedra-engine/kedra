//! Actual semantic adapter, registered shared services, jobs and retained views.
#![allow(clippy::unwrap_used, reason = "Remote receipt and state assertions")]
use super::*;
use crate::startup_tests::{app, element, projects};
use std::time::{Duration, Instant};

fn request(world: &mut World, command_value: Value) -> Value {
    let session = world.resource::<kedra_app::AppSession>().id.clone();
    command(
        In(Some(
            json!({"session":session,"id":Uuid::new_v4(),"command":command_value}),
        )),
        world,
    )
    .unwrap()
}

fn until(app: &mut App, condition: impl Fn(&World) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition(app.world()) {
        assert!(
            Instant::now() < deadline,
            "remote flow did not reach its explicit completion"
        );
        app.update();
        std::thread::yield_now();
    }
}

#[test]
fn actual_registration_discovery_and_invalid_session_do_not_admit_effects() {
    let mut app = app();
    for name in [
        "editor.state",
        "editor.workspace",
        "editor.project",
        "app.operation",
        "app.input",
    ] {
        assert!(
            app.world()
                .resource::<kedra_app::CommandCatalog>()
                .0
                .contains_key(name)
        );
    }
    assert_eq!(state(In(None), app.world_mut()).unwrap()["phase"], "boot");
    assert_eq!(
        workspace_state(In(None), app.world_mut()).unwrap()["bound"],
        false
    );
    let id = Uuid::new_v4();
    assert!(
        command(
            In(Some(
                json!({"session":"stale","id":id,"command":{"kind":"new_form"}})
            )),
            app.world_mut()
        )
        .is_err()
    );
    assert!(app.world().resource::<Operations>().get(id).is_none());
    let receipt = request(
        app.world_mut(),
        json!({"kind":"open","root":"not consulted during boot"}),
    );
    assert_eq!(receipt["state"], "failed");
    assert!(receipt["detail"].as_str().unwrap().contains("startup"));
    assert!(!app.world().contains_resource::<jobs::PendingJob>());
}

#[test]
fn forms_require_mounted_fields_and_retries_cannot_replay_or_mutate_a_receipt() {
    let mut app = app();
    projects(&mut app);
    assert_eq!(
        request(app.world_mut(), json!({"kind":"cancel"}))["state"],
        "failed"
    );
    assert_eq!(
        request(app.world_mut(), json!({"kind":"new_form"}))["state"],
        "succeeded"
    );
    let early = request(app.world_mut(), json!({"kind":"draft","name":"Too early"}));
    assert_eq!(early["state"], "failed");
    assert!(early["detail"].as_str().unwrap().contains("not mounted"));
    app.update();
    let session = app.world().resource::<kedra_app::AppSession>().id.clone();
    let id = Uuid::new_v4();
    let original = json!({"session":session,"id":id,"command":{"kind":"draft","name":"日本語","directory":"relative"}});
    let receipt = command(In(Some(original.clone())), app.world_mut()).unwrap();
    assert_eq!(receipt["state"], "succeeded");
    let name = element(app.world_mut(), "entry.name");
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(name)
            .unwrap()
            .value()
            .to_string(),
        "日本語"
    );
    assert_eq!(
        command(In(Some(original.clone())), app.world_mut()).unwrap(),
        receipt
    );
    let mut conflicting = original;
    conflicting["command"]["name"] = json!("Different");
    assert!(command(In(Some(conflicting)), app.world_mut()).is_err());
    app.update();
    let receipt = request(app.world_mut(), json!({"kind":"submit"}));
    assert_eq!(receipt["state"], "failed");
    assert!(receipt["detail"].as_str().unwrap().contains("absolute"));
    assert_eq!(
        request(app.world_mut(), json!({"kind":"open_form"}))["state"],
        "failed"
    );
    assert_eq!(
        request(app.world_mut(), json!({"kind":"cancel"}))["state"],
        "succeeded"
    );
    assert_eq!(
        request(app.world_mut(), json!({"kind":"retry"}))["state"],
        "failed"
    );
    assert_eq!(
        request(app.world_mut(), json!({"kind":"projects"}))["state"],
        "failed"
    );
}

#[test]
fn failed_preflight_has_a_terminal_receipt_visible_notice_and_same_id_retry() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = app();
    projects(&mut app);
    let session = app.world().resource::<kedra_app::AppSession>().id.clone();
    let id = Uuid::new_v4();
    let original = json!({"session":session,"id":id,"command":{"kind":"open","root":directory.path().join("missing")}});
    assert_eq!(
        command(In(Some(original.clone())), app.world_mut()).unwrap()["state"],
        "pending"
    );
    let new_id = Uuid::new_v4();
    assert!(
        command(
            In(Some(
                json!({"session":session,"id":new_id,"command":{"kind":"new_form"}})
            )),
            app.world_mut()
        )
        .is_err()
    );
    assert!(app.world().resource::<Operations>().get(new_id).is_none());
    until(&mut app, |world| {
        world.resource::<Operations>().get(id).unwrap().state == OperationState::Failed
    });
    let receipt = command(In(Some(original)), app.world_mut()).unwrap();
    assert_eq!(receipt["state"], "failed");
    let state = state(In(None), app.world_mut()).unwrap();
    assert_eq!(state["phase"], "projects");
    assert_eq!(state["busy"], false);
    assert!(!state["notice"].as_str().unwrap().is_empty());
    assert!(app.world().resource::<Pending>().0.is_none());
}

#[test]
fn real_open_exposes_workspace_without_waiting_for_a_loading_window_and_returns() {
    let directory = tempfile::tempdir().unwrap();
    let project = kedra_project::create(directory.path(), "Integration 日本語").unwrap();
    let mut app = app();
    projects(&mut app);
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_secs(1),
    ));
    let receipt = request(app.world_mut(), json!({"kind":"open","root":project.root}));
    let id = Uuid::parse_str(receipt["id"].as_str().unwrap()).unwrap();
    until(&mut app, |world| {
        world.resource::<Operations>().get(id).unwrap().state == OperationState::Succeeded
    });
    until(&mut app, |world| {
        world.contains_resource::<crate::workspace::EditorWorkspace>()
    });
    let window = app
        .world()
        .resource::<kedra_desktop::DesktopSurface>()
        .window
        .unwrap();
    assert_eq!(
        app.world().get::<kedra_desktop::SurfaceKind>(window),
        Some(&kedra_desktop::SurfaceKind::Editor)
    );
    let workspace = workspace_state(In(None), app.world_mut()).unwrap();
    assert_eq!(workspace["bound"], true);
    assert_eq!(workspace["native_alive"], true);
    assert_eq!(
        state(In(None), app.world_mut()).unwrap()["project"]["name"],
        "Integration 日本語"
    );
    assert!(
        app.world()
            .resource::<kedra_ui::authored::components::UiFailure>()
            .0
            .is_none()
    );
    assert_eq!(
        request(app.world_mut(), json!({"kind":"projects"}))["state"],
        "succeeded"
    );
    until(&mut app, |world| {
        *world.resource::<State<crate::AppPhase>>().get() == crate::AppPhase::Projects
    });
    assert_eq!(
        workspace_state(In(None), app.world_mut()).unwrap()["bound"],
        false
    );
    assert!(!app.world().entities().contains(window));
    assert_eq!(kedra_project::open(&project.root).unwrap(), project);
}
