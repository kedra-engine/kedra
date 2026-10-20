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
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
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
fn write(path: &Path, baseline: Option<&[u8]>, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("Workspace preference path has no parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("lock"))
        .map_err(|e| e.to_string())?;
    lock.try_lock()
        .map_err(|e| format!("Another editor is writing this workspace: {e}"))?;
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
    if let Some(task) = state.preferences.task.take() {
        match block_on(task) {
            Ok(bytes) => state.preferences.baseline = Some(bytes),
            Err(error) => {
                state.preferences.error = Some(error);
            }
        }
    }
    if state.preferences.error.is_some() {
        return;
    }
    if let Some(path) = &state.preferences.path {
        match encoded(world, state).and_then(|bytes| {
            write(path, state.preferences.baseline.as_deref(), &bytes)?;
            Ok(bytes)
        }) {
            Ok(bytes) => state.preferences.baseline = Some(bytes),
            Err(error) => warn!("Workspace preferences not saved: {error}"),
        }
    }
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
            let lock = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(path.with_extension("lock"))
                .map_err(|e| e.to_string())?;
            lock.try_lock()
                .map_err(|e| format!("Another editor is writing this workspace: {e}"))?;
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
