//! Completed task outcomes and controlled clocks exercise the production poller.
#![allow(clippy::unwrap_used, reason = "Isolated lifecycle assertions")]
use super::*;
use bevy::ecs::system::RunSystemOnce;
use std::time::{Duration, Instant};

fn world() -> World {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ProjectManager>()
        .init_resource::<EntryForm>()
        .init_resource::<bevy::input_focus::InputFocus>();
    let mut manager = app.world_mut().resource_mut::<ProjectManager>();
    manager.history_path = None;
    manager.history_writable = false;
    std::mem::take(app.world_mut())
}

fn await_job(world: &World) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !world.resource::<PendingJob>().task.is_finished() {
        assert!(
            Instant::now() < deadline,
            "isolated I/O task did not complete"
        );
        std::thread::yield_now();
    }
}

fn completed(world: &mut World, generation: u64, outcome: Outcome) {
    world.insert_resource(PendingJob {
        generation,
        task: IoTaskPool::get().spawn(async move { outcome }),
    });
    await_job(world);
    poll(world);
    assert!(!world.contains_resource::<PendingJob>());
}

#[test]
fn pending_and_stale_tasks_never_publish_or_erase_a_newer_request() {
    let mut world = world();
    world.insert_resource(PendingJob {
        generation: 0,
        task: IoTaskPool::get().spawn(bevy::tasks::futures_lite::future::pending()),
    });
    poll(&mut world);
    assert!(world.contains_resource::<PendingJob>());
    world.remove_resource::<PendingJob>();
    let generation = world
        .resource_mut::<ProjectManager>()
        .begin(Intent::Open("new".into()))
        .unwrap();
    completed(
        &mut world,
        generation.wrapping_sub(1),
        Outcome::Project(Err("stale".into())),
    );
    assert!(world.resource::<ProjectManager>().loading_error().is_none());
    assert_eq!(
        world.resource::<ProjectManager>().intent,
        Some(Intent::Open("new".into()))
    );
    completed(
        &mut world,
        generation,
        Outcome::Project(Err("current failure".into())),
    );
    assert_eq!(
        world.resource::<ProjectManager>().loading_error(),
        Some("current failure")
    );
    assert!(world.resource::<ProjectManager>().project().is_none());
}

#[test]
fn actual_history_loading_preserves_corruption_and_disables_writes() {
    let directory = tempfile::tempdir().unwrap();
    let history = directory.path().join("recent.toml");
    std::fs::write(&history, b"invalid original history").unwrap();
    let mut world = world();
    {
        let mut manager = world.resource_mut::<ProjectManager>();
        manager.history_path = Some(history.clone());
        manager.history_writable = true;
    }
    load_history(&mut world);
    await_job(&world);
    poll(&mut world);
    assert!(!world.resource::<ProjectManager>().history_writable);
    assert!(
        world
            .resource::<ProjectManager>()
            .notice
            .contains("will not be overwritten")
    );
    assert_eq!(
        std::fs::read(&history).unwrap(),
        b"invalid original history"
    );
    world.resource_mut::<ProjectManager>().history_path = None;
    load_history(&mut world);
    await_job(&world);
    poll(&mut world);
    assert!(
        world
            .resource::<ProjectManager>()
            .notice
            .contains("unavailable")
    );
}

#[test]
fn completed_history_starts_initial_request_only_after_loading_and_closes_modal() {
    let directory = tempfile::tempdir().unwrap();
    let project = kedra_project::create(directory.path(), "日本語").unwrap();
    let mut history = RecentProjects::default();
    history.remember(&project);
    let mut world = world();
    let modal = world.spawn_empty().id();
    {
        let mut manager = world.resource_mut::<ProjectManager>();
        manager.initial_project = Some(project.root.clone());
        manager.modal = Some(modal);
    }
    completed(&mut world, 0, Outcome::History(Ok(history)));
    let manager = world.resource::<ProjectManager>();
    assert_eq!(manager.phase(), Phase::Loading);
    assert_eq!(manager.history.entries().len(), 1);
    assert!(manager.initial_project.is_none());
    assert!(manager.modal.is_none());
    assert!(!world.entities().contains(modal));
}

#[test]
fn preflight_refusal_retains_form_and_success_begins_without_creating_sources() {
    let directory = tempfile::tempdir().unwrap();
    let mut world = world();
    world.resource_mut::<EntryForm>().mode = Some(EntryMode::Create);
    let intent = Intent::Create {
        parent: directory.path().into(),
        name: "New game".into(),
    };
    completed(
        &mut world,
        0,
        Outcome::Preflight {
            intent: intent.clone(),
            result: Err("no access".into()),
        },
    );
    assert_eq!(world.resource::<EntryForm>().error, "no access");
    assert_eq!(world.resource::<ProjectManager>().phase(), Phase::Projects);
    preflight(&mut world, intent.clone());
    await_job(&world);
    poll(&mut world);
    assert_eq!(world.resource::<ProjectManager>().phase(), Phase::Loading);
    assert_eq!(world.resource::<ProjectManager>().intent, Some(intent));
    assert!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn actual_prepare_creates_once_and_history_write_failure_is_nonfatal() {
    let directory = tempfile::tempdir().unwrap();
    let blocker = directory.path().join("not-a-directory");
    std::fs::write(&blocker, b"keep").unwrap();
    let mut world = world();
    {
        let mut manager = world.resource_mut::<ProjectManager>();
        manager.history_writable = true;
        manager.history_path = Some(blocker.join("recent.toml"));
        manager
            .begin(Intent::Create {
                parent: directory.path().into(),
                name: "New game".into(),
            })
            .unwrap();
    }
    prepare(&mut world);
    await_job(&world);
    poll(&mut world);
    let manager = world.resource::<ProjectManager>();
    assert!(manager.is_ready());
    assert_eq!(manager.phase(), Phase::Loading);
    assert!(manager.notice.contains("could not be saved"));
    assert_eq!(manager.history.entries().len(), 1);
    assert!(kedra_project::open(&directory.path().join("New game")).is_ok());
    assert_eq!(std::fs::read(&blocker).unwrap(), b"keep");
    // A second real creation cannot silently turn an existing destination into success.
    prepare(&mut world);
    await_job(&world);
    poll(&mut world);
    assert!(
        world.resource::<ProjectManager>().is_ready(),
        "duplicate completion replaced settled preparation"
    );
}

#[test]
fn picker_cancel_and_non_unicode_result_do_not_replace_existing_draft() {
    let mut world = world();
    world.resource_mut::<EntryForm>().directory = "/original".into();
    let field = world
        .spawn((Field::Directory, bevy::text::EditableText::new("/original")))
        .id();
    completed(&mut world, 0, Outcome::Directory(None));
    assert_eq!(world.resource::<EntryForm>().directory, "/original");
    completed(&mut world, 0, Outcome::Directory(Some("/日本語".into())));
    assert_eq!(world.resource::<EntryForm>().directory, "/日本語");
    assert_eq!(
        world
            .get::<bevy::text::EditableText>(field)
            .unwrap()
            .value()
            .to_string(),
        "/日本語"
    );
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        completed(
            &mut world,
            0,
            Outcome::Directory(Some(std::ffi::OsString::from_vec(vec![255]).into())),
        );
        assert_eq!(world.resource::<EntryForm>().directory, "/日本語");
        assert!(
            world
                .resource::<EntryForm>()
                .error
                .contains("not valid Unicode")
        );
    }
}

#[test]
fn loading_enters_editor_immediately_when_owned_creation_hydration_and_work_are_ready() {
    let directory = tempfile::tempdir().unwrap();
    let project = kedra_project::create(directory.path(), "Ready").unwrap();
    let mut world = world();
    let window = world.spawn(Window::default()).id();
    world.insert_resource(kedra_desktop::DesktopSurface {
        window: Some(window),
        ..default()
    });
    world.insert_resource(State::new(AppPhase::Loading));
    world.insert_resource(FrameCount(100));
    world.init_resource::<Messages<WindowCreated>>();
    let generation = world
        .resource_mut::<ProjectManager>()
        .begin(Intent::Open(project.root.clone()))
        .unwrap();
    world
        .resource_mut::<ProjectManager>()
        .complete(generation, Ok(project));
    let hydrated = world.spawn(crate::loading::HydratedAt(100)).id();
    let other = world.spawn_empty().id();
    world.write_message(WindowCreated { window: other });
    world.run_system_once(finish_loading).unwrap();
    assert!(
        world
            .resource::<ProjectManager>()
            .native_created_at
            .is_none()
    );
    world.write_message(WindowCreated { window });
    world.run_system_once(finish_loading).unwrap();
    assert_eq!(
        world.resource::<ProjectManager>().native_created_at,
        Some(100)
    );
    assert!(world.resource::<ProjectManager>().loading_started.is_none());
    // New system instances get fresh message cursors. Clear consumed creation before
    // the next direct call so the synthetic event is not delivered a second time.
    world.resource_mut::<Messages<WindowCreated>>().clear();
    world.resource_mut::<FrameCount>().0 = 102;
    // Even a long-running process must still wait for source hydration.
    world
        .entity_mut(hydrated)
        .remove::<crate::loading::HydratedAt>();
    world.run_system_once(finish_loading).unwrap();
    assert_eq!(world.resource::<ProjectManager>().phase(), Phase::Loading);
    world
        .entity_mut(hydrated)
        .insert(crate::loading::HydratedAt(100));
    world.run_system_once(finish_loading).unwrap();
    assert_eq!(
        world.resource::<ProjectManager>().loading_started,
        Some(Duration::ZERO)
    );
    assert_eq!(
        world.resource::<Time<Real>>().elapsed(),
        Duration::ZERO,
        "No wall-clock advance is required"
    );
    assert_eq!(world.resource::<ProjectManager>().phase(), Phase::Editor);
}

#[test]
fn pending_job_disables_real_manager_controls_and_completion_reenables_them() {
    let mut app = crate::startup_tests::app();
    crate::startup_tests::projects(&mut app);
    let new = crate::startup_tests::element(app.world_mut(), "projects.new");
    app.insert_resource(PendingJob {
        generation: 0,
        task: IoTaskPool::get().spawn(bevy::tasks::futures_lite::future::pending()),
    });
    app.update();
    assert!(
        app.world()
            .get::<bevy::ui::InteractionDisabled>(new)
            .is_some()
    );
    actions::dispatch(app.world_mut(), Action::New, Some(new));
    assert!(app.world().resource::<EntryForm>().mode.is_none());
    app.world_mut().remove_resource::<PendingJob>();
    app.update();
    assert!(
        app.world()
            .get::<bevy::ui::InteractionDisabled>(new)
            .is_none()
    );
    actions::dispatch(app.world_mut(), Action::New, Some(new));
    assert!(app.world().resource::<EntryForm>().mode == Some(EntryMode::Create));
}
