//! Bounded layout preferences, separate from source Save and session drafts.
use super::*;
use bevy::tasks::{IoTaskPool, Task, block_on, poll_once};
use kedra_docking::snapshot::{MAX_BYTES, Snapshot};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Placement {
    pub width: u32,
    pub height: u32,
    pub position: Option<[i32; 2]>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Stored {
    pub main: HostId,
    pub layout: Snapshot,
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    pub placements: BTreeMap<HostId, Placement>,
}
#[derive(Default)]
pub(super) struct Preferences {
    path: Option<PathBuf>,
    baseline: Option<Vec<u8>>,
    pub restored: Option<Stored>,
    pub error: Option<String>,
    pub dirty: bool,
    last: Duration,
    task: Option<Task<Result<Vec<u8>, String>>>,
}
fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    // Match the project reader's static regular-file preflight: opening a FIFO
    // can block before a bounded read begins. This is not a sandbox against
    // concurrent path replacement. Recheck the opened handle before reading.
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.is_file() {
        return Err("Workspace preferences require a regular file".into());
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Workspace preferences require a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Workspace preferences exceed the size limit".into());
    }
    Ok(Some(bytes))
}
fn decode(bytes: &[u8]) -> Result<Stored, String> {
    if bytes.len() > MAX_BYTES {
        return Err("Workspace preferences exceed the size limit".into());
    }
    let value: Stored = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    value.layout.validate().map_err(|e| e.to_string())?;
    if !value.layout.hosts.contains_key(&value.main) || value.placements.len() > 64 {
        return Err("Invalid workspace host metadata".into());
    }
    for (host, place) in &value.placements {
        if !value.layout.hosts.contains_key(host)
            || !(240..=16384).contains(&place.width)
            || !(192..=16384).contains(&place.height)
        {
            return Err("Invalid floating host placement".into());
        }
    }
    Ok(value)
}
impl Preferences {
    pub fn load(world: &World) -> Self {
        let manager = world.resource::<crate::model::ProjectManager>();
        let path = manager.project.as_ref().and_then(|project| {
            manager
                .history_path
                .as_ref()
                .and_then(|history| history.parent())
                .map(|dir| {
                    dir.join("workspaces")
                        .join(format!("{}.json", project.manifest.id))
                })
        });
        let mut state = Self { path, ..default() };
        if let Some(path) = &state.path {
            match read(path) {
                Ok(Some(bytes)) => {
                    match decode(&bytes) {
                        Ok(value) => state.restored = Some(value),
                        Err(error) => {
                            state.error = Some(format!(
                                "Workspace was not restored: {error}. Reset layout to recover; the original is retained."
                            ))
                        }
                    }
                    state.baseline = Some(bytes);
                }
                Ok(None) => {}
                Err(error) => {
                    state.error = Some(format!(
                        "Cannot read workspace preferences: {error}. Reset layout to recover."
                    ))
                }
            }
        }
        state
    }
}
fn encoded(world: &World, state: &EditorWorkspace) -> Result<Vec<u8>, String> {
    let placements = state
        .surfaces
        .iter()
        .filter_map(|(id, surface)| {
            world.get::<Window>(surface.window).map(|window| {
                (
                    *id,
                    Placement {
                        width: window.width().round().clamp(240., 16384.) as u32,
                        height: window.height().round().clamp(192., 16384.) as u32,
                        position: match window.position {
                            WindowPosition::At(pos) => Some([pos.x, pos.y]),
                            _ => None,
                        },
                    },
                )
            })
        })
        .collect();
    let value = Stored {
        main: state.host,
        layout: state.layout.snapshot().clone(),
        placements,
    };
    value.layout.validate().map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Workspace preferences exceed the size limit".into());
    }
    Ok(bytes)
}
// A file close is not an explicit unlock: duplicated handles (including those
// temporarily inherited during another thread's process spawn) can keep the
// underlying lock alive. The preference operation owns release on every exit.
struct PreferenceLock(File);
impl PreferenceLock {
    fn acquire(path: &Path) -> Result<Self, String> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))
            .map_err(|e| e.to_string())?;
        file.try_lock()
            .map_err(|e| format!("Another editor is writing this workspace: {e}"))?;
        Ok(Self(file))
    }
}
impl Drop for PreferenceLock {
    fn drop(&mut self) {
        if let Err(error) = self.0.unlock() {
            warn!("Cannot release workspace preference lock: {error}");
        }
    }
}

fn write(path: &Path, baseline: Option<&[u8]>, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("Workspace preference path has no parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let _lock = PreferenceLock::acquire(path)?;
    if read(path)?.as_deref() != baseline {
        return Err("Workspace preferences changed externally; not overwritten".into());
    }
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    File::open(parent)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub(super) fn poll(world: &mut World, state: &mut EditorWorkspace) {
    if let Some(result) = state
        .preferences
        .task
        .as_mut()
        .and_then(|task| block_on(poll_once(task)))
    {
        state.preferences.task = None;
        match result {
            Ok(bytes) => state.preferences.baseline = Some(bytes),
            Err(error) => {
                state.preferences.error = Some(error.clone());
                notice(world, state, &error);
            }
        }
    }
    if let Some(error) = &state.preferences.error {
        if let Some(mut text) = state.status.and_then(|e| world.get_mut::<Text>(e)) {
            text.0 = error.clone();
        }
        return;
    }
    let now = world.resource::<Time<Real>>().elapsed();
    if state.preferences.task.is_some()
        || now.saturating_sub(state.preferences.last) < Duration::from_secs(2)
    {
        return;
    }
    // Include native resize/move even when topology revision is unchanged.
    state.preferences.last = now;
    let Some(path) = state.preferences.path.clone() else {
        return;
    };
    let bytes = match encoded(world, state) {
        Ok(bytes) => bytes,
        Err(error) => {
            notice(world, state, &error);
            return;
        }
    };
    if state.preferences.baseline.as_ref() == Some(&bytes) {
        state.preferences.dirty = false;
        return;
    }
    let baseline = state.preferences.baseline.clone();
    state.preferences.dirty = false;
    state.preferences.task = Some(IoTaskPool::get().spawn(async move {
        write(&path, baseline.as_deref(), &bytes)?;
        Ok(bytes)
    }));
}
pub(super) fn save(world: &World, state: &mut EditorWorkspace) {
    if let Err(error) = save_checked(world, state) {
        warn!("Workspace preferences not saved: {error}");
    }
}
pub(super) fn save_checked(world: &World, state: &mut EditorWorkspace) -> Result<(), String> {
    if let Some(task) = state.preferences.task.take() {
        match block_on(task) {
            Ok(bytes) => state.preferences.baseline = Some(bytes),
            Err(error) => {
                state.preferences.error = Some(error);
            }
        }
    }
    if let Some(error) = &state.preferences.error {
        return Err(error.clone());
    }
    if let Some(path) = &state.preferences.path {
        let bytes = encoded(world, state).and_then(|bytes| {
            write(path, state.preferences.baseline.as_deref(), &bytes)?;
            Ok(bytes)
        })?;
        state.preferences.baseline = Some(bytes);
    }
    Ok(())
}
pub(super) fn save_on_exit(world: &mut World) {
    if world.resource::<Messages<AppExit>>().is_empty()
        || !world.contains_resource::<EditorWorkspace>()
    {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| save(world, &mut state));
}
pub(super) fn allow_reset(state: &mut EditorWorkspace) -> Result<(), String> {
    if let Some(task) = state.preferences.task.take() {
        state.preferences.baseline = Some(block_on(task)?);
    }
    if state.preferences.error.is_some()
        && let Some(path) = &state.preferences.path
    {
        if path.exists() {
            let _lock = PreferenceLock::acquire(path)?;
            let backup = path.with_extension(format!("rejected-{}.json", uuid::Uuid::new_v4()));
            // Rename preserves even oversized/corrupted originals without
            // parsing, truncating or allocating their contents. Explicit reset only.
            fs::rename(path, backup).map_err(|e| e.to_string())?;
        }
        state.preferences.baseline = None;
    }
    state.preferences.error = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preference_operation_unlocks_even_when_a_duplicate_handle_outlives_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("layout.json");
        fs::write(&path, b"original")?;
        let lock = PreferenceLock::acquire(&path)?;
        // Deterministically model an inherited descriptor without unsafe fork or
        // depending on concurrent process-spawn timing in the test runner.
        let duplicate = lock.0.try_clone()?;
        assert!(
            write(&path, Some(b"original"), b"blocked")
                .is_err_and(|error| error.contains("Another editor"))
        );
        assert_eq!(read(&path)?.as_deref(), Some(b"original".as_slice()));
        drop(lock);
        // Must succeed while the duplicate remains open, with no retry/sleep.
        write(&path, Some(b"original"), b"published")?;
        assert_eq!(read(&path)?.as_deref(), Some(b"published".as_slice()));
        drop(duplicate);
        Ok(())
    }

    #[test]
    fn bounded_read_preserves_absence_unicode_regular_files_and_refuses_directories()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("配置-é.json");
        assert_eq!(read(&path)?, None);
        fs::write(&path, b"original")?;
        assert_eq!(read(&path)?.as_deref(), Some(b"original".as_slice()));
        assert!(read(dir.path()).is_err_and(|error| error.contains("regular file")));
        Ok(())
    }

    // Run blocking-boundary probes in a killable subprocess. A regression to
    // File::open-before-preflight must fail on a deadline, not hang the suite.
    #[cfg(unix)]
    #[test]
    fn fifo_preferences_are_refused_without_blocking() -> Result<(), Box<dyn std::error::Error>> {
        use std::{process::Command, time::Instant};
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("配置-fifo.json");
        assert!(Command::new("mkfifo").arg(&path).status()?.success());
        let module = module_path!().split_once("::").ok_or("test module path")?.1;
        let mut child = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                &format!("{module}::fifo_preference_probe"),
                "--nocapture",
            ])
            .env("KEDRA_WORKSPACE_FIFO_PROBE", &path)
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait()? {
                assert!(status.success(), "FIFO preference probe failed");
                break;
            }
            if Instant::now() >= deadline {
                child.kill()?;
                child.wait()?;
                panic!("FIFO preference read blocked before regular-file rejection");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn fifo_preference_probe() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::FileTypeExt;
        let Some(path) = std::env::var_os("KEDRA_WORKSPACE_FIFO_PROBE") else {
            return Ok(());
        };
        let path = PathBuf::from(path);
        assert!(fs::metadata(&path)?.file_type().is_fifo());
        assert!(read(&path).is_err_and(|error| error.contains("regular file")));
        assert!(
            write(&path, None, b"replacement").is_err_and(|error| error.contains("regular file"))
        );
        let mut world = World::new();
        let window = world.spawn(Window::default()).id();
        let root = world.spawn_empty().id();
        super::super::bind_root(&mut world, window, root)?;
        world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            state.preferences.path = Some(path.clone());
            state.preferences.baseline = Some(b"previous baseline".to_vec());
            let baseline = state.preferences.baseline.clone();
            assert!(
                save_checked(world, &mut state).is_err_and(|error| error.contains("regular file"))
            );
            assert_eq!(state.preferences.baseline, baseline);
        });
        assert!(fs::metadata(&path)?.file_type().is_fifo());
        Ok(())
    }

    #[test]
    fn oversized_rejected_preferences_require_explicit_reset_and_keep_original() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let path = dir.path().join("layout.json");
        let original = vec![b'x'; MAX_BYTES + 1];
        fs::write(&path, &original).unwrap_or_else(|e| panic!("{e}"));
        assert!(read(&path).is_err());
        let mut world = World::new();
        let window = world.spawn(Window::default()).id();
        let root = world.spawn_empty().id();
        super::super::bind_root(&mut world, window, root).unwrap_or_else(|e| panic!("{e}"));
        let mut state = world.resource_mut::<EditorWorkspace>();
        state.preferences = Preferences {
            path: Some(path.clone()),
            error: Some("rejected".into()),
            ..default()
        };
        allow_reset(&mut state).unwrap_or_else(|e| panic!("{e}"));
        assert!(!path.exists());
        assert!(state.preferences.error.is_none());
        let backup = fs::read_dir(dir.path())
            .unwrap_or_else(|e| panic!("{e}"))
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_string_lossy().contains("rejected-"))
            .map(|entry| entry.path())
            .unwrap_or_else(|| panic!("Original was not retained"));
        assert_eq!(fs::read(backup).unwrap_or_else(|e| panic!("{e}")), original);
    }
    #[test]
    fn duplicate_or_invalid_host_metadata_is_rejected() {
        let host = HostId::new();
        let stored = Stored {
            main: host,
            layout: Workspace::new(host).snapshot().clone(),
            placements: BTreeMap::from([(
                host,
                Placement {
                    width: 800,
                    height: 560,
                    position: None,
                },
            )]),
        };
        let json = serde_json::to_string(&stored).unwrap_or_else(|e| panic!("{e}"));
        let map = serde_json::to_string(&stored.placements).unwrap_or_else(|e| panic!("{e}"));
        let entry = &map[1..map.len() - 1];
        assert!(
            decode(
                json.replacen(&map, &format!("{{{entry},{entry}}}"), 1)
                    .as_bytes()
            )
            .is_err()
        );
        assert!(decode(json.replace("\"width\":800", "\"width\":0").as_bytes()).is_err());
        assert!(decode(json.as_bytes()).is_ok());
    }
    #[test]
    fn durable_preferences_refuse_conflicts_and_preserve_rejected_input() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let path = dir.path().join("layout.json");
        write(&path, None, b"first").unwrap_or_else(|e| panic!("{e}"));
        assert!(write(&path, None, b"second").is_err());
        assert_eq!(
            read(&path).ok().flatten().as_deref(),
            Some(b"first".as_slice())
        );
        assert!(decode(b"first").is_err());
        write(&path, Some(b"first"), b"second").unwrap_or_else(|e| panic!("{e}"));
        assert!(write(&path, Some(b"first"), b"third").is_err());
    }
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod lifecycle_tests;
