//! Isolated preference lifecycle and async flush tests; never uses personal state.
use super::*;
use crate::workspace::input::regression_tests::setup;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn update_flush_propagates_save_failure_without_authorizing_exit() -> TestResult {
    let (mut app, _, _) = setup()?;
    app.add_message::<AppExit>();
    app.world_mut()
        .resource_mut::<EditorWorkspace>()
        .preferences
        .task = Some(IoTaskPool::get().spawn(async { Err("cancelled or failed save".into()) }));
    assert!(crate::workspace::save_for_update(app.world_mut()).is_err());
    assert!(app.world().resource::<Messages<AppExit>>().is_empty());
    assert_eq!(
        app.world()
            .resource::<EditorWorkspace>()
            .preferences
            .error
            .as_deref(),
        Some("cancelled or failed save")
    );
    Ok(())
}

#[test]
fn dirty_async_write_then_exit_flush_keeps_latest_published_state() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("workspace.json");
    let (mut app, a, _) = setup()?;
    let world = app.world_mut();
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            state.preferences.path = Some(path.clone());
            state.preferences.dirty = true;
            poll(world, &mut state);
            assert!(
                state.preferences.task.is_none(),
                "Throttle must not launch early"
            );
            world
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_secs(2));
            poll(world, &mut state);
            assert!(state.preferences.task.is_some());
            state
                .layout
                .execute(Command::Hide(a))
                .map_err(|e| e.to_string())?;
            state.preferences.dirty = true;
            // Flush waits for the serialized in-flight write, then writes the new state.
            save(world, &mut state);
            assert!(state.preferences.task.is_none());
            assert!(state.preferences.error.is_none());
            let bytes = read(&path)?.ok_or("saved bytes")?;
            assert_eq!(decode(&bytes)?.layout, *state.layout.snapshot());
            assert_eq!(
                state.preferences.baseline.as_deref(),
                Some(bytes.as_slice())
            );
            poll(world, &mut state);
            world
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_secs(2));
            poll(world, &mut state);
            assert!(
                state.preferences.task.is_none(),
                "Unchanged bytes must not start another write"
            );
            assert!(!state.preferences.dirty);
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn exit_without_request_does_not_write_and_requested_exit_flushes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("workspace.json");
    let (mut app, _, _) = setup()?;
    let world = app.world_mut();
    world.init_resource::<Messages<AppExit>>();
    world.resource_mut::<EditorWorkspace>().preferences.path = Some(path.clone());
    save_on_exit(world);
    assert!(!path.exists());
    world.write_message(AppExit::Success);
    save_on_exit(world);
    let bytes = read(&path)?.ok_or("saved")?;
    assert_eq!(
        decode(&bytes)?.layout,
        *world.resource::<EditorWorkspace>().layout.snapshot()
    );
    Ok(())
}

#[test]
fn write_conflict_blocks_flush_until_explicit_reset_preserves_external_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("workspace.json");
    let (mut app, _, _) = setup()?;
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            state.preferences.path = Some(path.clone());
            // Exercise the task-failure branch of flush, not just the low-level writer.
            state.preferences.task =
                Some(IoTaskPool::get().spawn(async { Err("conflicting writer".into()) }));
            fs::write(&path, b"external original").map_err(|e| e.to_string())?;
            save(world, &mut state);
            assert!(state.preferences.error.is_some());
            assert_eq!(
                read(&path)?.as_deref(),
                Some(b"external original".as_slice())
            );
            let status = world.spawn(Text::new("")).id();
            state.status = Some(status);
            poll(world, &mut state);
            assert_eq!(
                world.get::<Text>(status).map(|t| t.0.as_str()),
                Some("conflicting writer")
            );
            allow_reset(&mut state)?;
            assert!(!path.exists());
            assert!(state.preferences.error.is_none());
            let backup = fs::read_dir(dir.path())
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .find(|e| e.file_name().to_string_lossy().contains("rejected-"))
                .ok_or("backup")?;
            assert_eq!(
                fs::read(backup.path()).map_err(|e| e.to_string())?,
                b"external original"
            );
            save(world, &mut state);
            assert_eq!(
                decode(&read(&path)?.ok_or("saved")?)?.layout,
                *state.layout.snapshot()
            );
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn project_preference_load_accepts_roundtrip_and_preserves_rejected_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let project = kedra_project::create(dir.path(), "Workspace test")?;
    let (mut app, _, _) = setup()?;
    let world = app.world_mut();
    world.insert_resource(crate::workspace::test_support::project_manager(
        project,
        Some(dir.path().join("state/recent.toml")),
    )?);
    let preferences = Preferences::load(world);
    let path = preferences.path.clone().ok_or("project path")?;
    assert!(preferences.error.is_none() && preferences.restored.is_none());
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        state.preferences = preferences;
        save(world, &mut state);
    });
    let restored = Preferences::load(world);
    assert!(restored.restored.is_some() && restored.error.is_none());
    fs::write(&path, b"not a layout")?;
    let rejected = Preferences::load(world);
    assert!(rejected.restored.is_none() && rejected.error.is_some());
    assert_eq!(
        rejected.baseline.as_deref(),
        Some(b"not a layout".as_slice())
    );
    assert_eq!(fs::read(&path)?, b"not a layout");
    fs::write(&path, vec![b' '; MAX_BYTES + 1])?;
    assert!(Preferences::load(world).error.is_some());
    Ok(())
}

#[test]
fn lock_contention_and_bad_metadata_are_explicit_errors() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("workspace.json");
    write(&path, None, b"original")?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path.with_extension("lock"))?;
    lock.try_lock()?;
    assert!(write(&path, Some(b"original"), b"replacement").is_err());
    assert_eq!(read(&path)?.as_deref(), Some(b"original".as_slice()));
    drop(lock);
    let (app, _, _) = setup()?;
    let state = app.world().resource::<EditorWorkspace>();
    let mut value: serde_json::Value = serde_json::from_slice(&encoded(app.world(), state)?)?;
    value["main"] = serde_json::to_value(HostId::new())?;
    assert!(decode(&serde_json::to_vec(&value)?).is_err());
    value["main"] = serde_json::to_value(state.host)?;
    let key = serde_json::to_value(state.host)?
        .as_str()
        .ok_or("host string")?
        .to_owned();
    value["placements"][&key]["width"] = 0.into();
    assert!(decode(&serde_json::to_vec(&value)?).is_err());
    Ok(())
}

#[test]
fn poll_collects_completed_writes_and_reports_worker_failure() -> TestResult {
    use crate::workspace::test_support::wait;
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("workspace.json");
    let (mut app, _, _) = setup()?;
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            state.preferences.path = Some(path.clone());
            world
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_secs(2));
            poll(world, &mut state);
            wait(state.preferences.task.as_ref().ok_or("task")?);
            poll(world, &mut state);
            assert!(state.preferences.task.is_none());
            assert_eq!(state.preferences.baseline, read(&path)?);
            state.preferences.task =
                Some(IoTaskPool::get().spawn(async { Err("worker failure".into()) }));
            wait(state.preferences.task.as_ref().ok_or("error task")?);
            poll(world, &mut state);
            assert_eq!(state.preferences.error.as_deref(), Some("worker failure"));
            assert!(state.preferences.task.is_none());
            Ok(())
        },
    )?;
    Ok(())
}
