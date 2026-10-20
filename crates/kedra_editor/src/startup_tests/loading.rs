use super::*;
use crate::{actions, model::*};
use bevy_flair::style::components::ClassList;

fn until(app: &mut App, condition: impl Fn(&World) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition(app.world()) {
        assert!(
            Instant::now() < deadline,
            "loading fixture did not reach completion"
        );
        app.update();
        std::thread::yield_now();
    }
}

#[test]
fn long_unicode_project_identity_survives_the_fast_open_path() {
    let directory = tempfile::tempdir().unwrap();
    let name = "Кедра_日本語_".repeat(7);
    let project = kedra_project::create(directory.path(), &name).unwrap();
    let mut app = app();
    projects(&mut app);
    actions::begin(app.world_mut(), Intent::Open(project.root.clone()));
    until(&mut app, |world| {
        world.resource::<ProjectManager>().is_ready()
    });
    app.update();
    let manager = app.world().resource::<ProjectManager>();
    assert_eq!(manager.project().unwrap().manifest.name, name);
    assert_eq!(manager.project().unwrap().root, project.root);
    assert_eq!(manager.phase(), Phase::Editor);
    assert_eq!(kedra_project::open(&project.root).unwrap(), project);
}

#[test]
fn post_preflight_failure_is_immediate_logged_and_retry_clears_failure_presentation() {
    let directory = tempfile::tempdir().unwrap();
    let project = kedra_project::create(directory.path(), "Retry me").unwrap();
    let manifest = project.root.join(kedra_project::MANIFEST_FILE);
    let original = std::fs::read(&manifest).unwrap();
    let mut app = app();
    projects(&mut app);
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_millis(10),
    ));
    // Simulate a real race: preflight's project disappeared before preparation's
    // independent read. Begin is the production preflight-success continuation.
    actions::begin(app.world_mut(), Intent::Open(project.root.clone()));
    std::fs::remove_file(&manifest).unwrap();
    until(&mut app, |world| {
        world.resource::<ProjectManager>().loading_error().is_some()
    });
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<ProjectManager>().phase(),
        Phase::Loading
    );
    assert!(
        app.world()
            .resource::<ProjectManager>()
            .native_created_at
            .is_none()
    );
    let error = app
        .world()
        .resource::<ProjectManager>()
        .loading_error()
        .unwrap()
        .to_owned();
    let phase = element(app.world_mut(), "loading.phase");
    assert_eq!(app.world().get::<Text>(phase).unwrap().0, error);
    assert!(
        app.world()
            .get::<ClassList>(phase)
            .unwrap()
            .contains("error")
    );
    for (key, hidden) in [
        ("loading.actions", true),
        ("loading.failure_actions", false),
        ("loading.retry", false),
        ("loading.spinner", true),
        ("loading.error_icon", false),
    ] {
        let entity = element(app.world_mut(), key);
        assert_eq!(
            app.world()
                .get::<ClassList>(entity)
                .unwrap()
                .contains("hidden"),
            hidden,
            "{key}"
        );
    }
    let frame = element(app.world_mut(), "window.frame");
    let panel = element(app.world_mut(), "loading.panel");
    for entity in [frame, panel] {
        assert!(
            app.world()
                .get::<ClassList>(entity)
                .unwrap()
                .contains("failed")
        );
    }
    assert!(
        app.world_mut()
            .query::<&kedra_ui::authored::components::UiSpin>()
            .iter(app.world())
            .all(|spin| spin.paused)
    );
    let log = element(app.world_mut(), "loading.log_text");
    let logged = app.world().get::<Text>(log).unwrap().0.clone();
    assert!(logged.contains("Project preparation requested"));
    assert!(logged.contains(&error));
    app.update();
    assert_eq!(
        app.world().get::<Text>(log).unwrap().0,
        logged,
        "unchanged phases must not append duplicate log entries"
    );
    actions::dispatch(app.world_mut(), Action::LoadingLog, None);
    let host = element(app.world_mut(), "loading.log");
    assert!(
        !app.world()
            .get::<ClassList>(host)
            .unwrap()
            .contains("hidden")
    );
    assert_eq!(
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .filter(|text| text.0 == "Hide startup log")
            .count(),
        2
    );
    actions::dispatch(app.world_mut(), Action::LoadingLog, None);
    assert!(
        app.world()
            .get::<ClassList>(host)
            .unwrap()
            .contains("hidden")
    );

    std::fs::write(&manifest, &original).unwrap();
    let generation = app.world().resource::<ProjectManager>().generation;
    actions::dispatch(app.world_mut(), Action::Retry, None);
    assert_ne!(
        app.world().resource::<ProjectManager>().generation,
        generation
    );
    until(&mut app, |world| {
        world.resource::<ProjectManager>().is_ready()
    });
    app.update();
    assert_eq!(
        app.world().resource::<ProjectManager>().phase(),
        Phase::Editor
    );
    for entity in [phase, frame, panel, log] {
        assert!(
            !app.world().entities().contains(entity),
            "Successful retry must retire failure presentation without a display delay"
        );
    }
    assert_eq!(kedra_project::open(&project.root).unwrap(), project);
}

#[test]
fn failed_create_has_no_retry_and_back_retires_loading_surface_without_touching_sources() {
    let directory = tempfile::tempdir().unwrap();
    let project = kedra_project::create(directory.path(), "Existing").unwrap();
    let manifest = project.root.join(kedra_project::MANIFEST_FILE);
    let original = std::fs::read(&manifest).unwrap();
    let mut app = app();
    projects(&mut app);
    actions::begin(
        app.world_mut(),
        Intent::Create {
            parent: directory.path().into(),
            name: "Existing".into(),
        },
    );
    until(&mut app, |world| {
        world.resource::<ProjectManager>().loading_error().is_some()
    });
    for _ in 0..3 {
        app.update();
    }
    let retry = element(app.world_mut(), "loading.retry");
    assert!(
        app.world()
            .get::<ClassList>(retry)
            .unwrap()
            .contains("hidden")
    );
    let generation = app.world().resource::<ProjectManager>().generation;
    actions::dispatch(app.world_mut(), Action::Retry, None);
    assert_eq!(
        app.world().resource::<ProjectManager>().generation,
        generation
    );
    assert!(!app.world().contains_resource::<crate::jobs::PendingJob>());
    let old = app
        .world()
        .resource::<kedra_desktop::DesktopSurface>()
        .clone();
    actions::dispatch(app.world_mut(), Action::Back, None);
    until(&mut app, |world| {
        *world.resource::<State<AppPhase>>().get() == AppPhase::Projects
    });
    assert!(!app.world().entities().contains(old.window.unwrap()));
    assert!(!app.world().entities().contains(old.root.unwrap()));
    assert!(
        app.world()
            .resource::<ProjectManager>()
            .loading_error()
            .is_none()
    );
    assert_eq!(std::fs::read(manifest).unwrap(), original);
}
