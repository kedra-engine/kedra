//! Source browsing state and read-only directory effects; no ECS or UI authority.
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

pub(super) const MAX_ENTRIES: usize = 512;
const MAX_SCAN: usize = 4096;
const MAX_NAME_BYTES: usize = 1024 * 1024;
const MAX_HISTORY: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Folder,
    Scene,
    Prefab,
    File,
    Unavailable,
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::Scene => "Scene",
            Self::Prefab => "Prefab",
            Self::File => "Source file",
            Self::Unavailable => "Unavailable",
        }
    }
}
#[derive(Clone, Debug)]
pub(super) struct Entry {
    pub name: OsString,
    pub kind: Kind,
    pub bytes: Option<u64>,
    pub tracked: bool,
}
#[derive(Debug)]
pub(super) struct Listing {
    pub entries: Vec<Entry>,
    pub truncated: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Request {
    pub generation: u64,
    pub path: PathBuf,
}
#[derive(Default)]
pub(super) struct Model {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub selected: Option<OsString>,
    pub filter: String,
    pub loading: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub truncated: bool,
    generation: u64,
    history: Vec<PathBuf>,
    position: usize,
    pending: Option<(Request, Option<usize>)>,
    visits: BTreeMap<PathBuf, Visit>,
    pub mode: Mode,
    pub documents_only: bool,
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Mode {
    #[default]
    Grid,
    List,
}
#[derive(Clone, Default)]
pub(super) struct Visit {
    pub filter: String,
    pub selected: Option<OsString>,
    pub scroll: f32,
}
impl Model {
    pub fn remember(&mut self, scroll: f32) {
        if self.visits.len() >= MAX_HISTORY && !self.visits.contains_key(&self.path) {
            self.visits.clear();
        }
        self.visits.insert(
            self.path.clone(),
            Visit {
                filter: self.filter.clone(),
                selected: self.selected.clone(),
                scroll,
            },
        );
    }
    pub fn scroll(&self) -> f32 {
        self.visits.get(&self.path).map_or(0., |v| v.scroll)
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn request(&mut self, path: PathBuf, history: Option<usize>) -> Result<Request, String> {
        if !relative(&path) {
            return Err("Location must stay inside the source root".into());
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("Browser request counter exhausted")?;
        let request = Request {
            generation: self.generation,
            path,
        };
        self.pending = Some((request.clone(), history));
        self.loading = true;
        self.error = None;
        self.notice = None;
        Ok(request)
    }
    pub fn complete(&mut self, request: &Request, result: Result<Listing, String>) -> bool {
        let Some((current, history)) = &self.pending else {
            return false;
        };
        if current != request {
            return false;
        }
        let history = *history;
        self.pending = None;
        self.loading = false;
        match result {
            Err(error) => self.error = Some(error),
            Ok(listing) => {
                if self.path != request.path {
                    let visit = self.visits.get(&request.path).cloned().unwrap_or_default();
                    self.selected = visit.selected;
                    self.filter = visit.filter;
                }
                self.path = request.path.clone();
                if let Some(position) = history {
                    self.position = position;
                } else if self.history.get(self.position) != Some(&self.path) {
                    self.history.truncate(self.position.saturating_add(1));
                    self.history.push(self.path.clone());
                    if self.history.len() > MAX_HISTORY {
                        self.history.remove(0);
                    }
                    self.position = self.history.len() - 1;
                }
                if !listing
                    .entries
                    .iter()
                    .any(|e| Some(&e.name) == self.selected.as_ref())
                {
                    self.selected = None;
                }
                self.entries = listing.entries;
                self.truncated = listing.truncated;
            }
        }
        true
    }
    pub fn history(&self, forward: bool) -> Option<(PathBuf, usize)> {
        let position = if forward {
            self.position.checked_add(1)?
        } else {
            self.position.checked_sub(1)?
        };
        self.history
            .get(position)
            .cloned()
            .map(|path| (path, position))
    }
    pub fn visible(&self) -> impl Iterator<Item = &Entry> {
        let filter = unicase::UniCase::unicode(&self.filter).to_folded_case();
        self.entries.iter().filter(move |entry| {
            (!self.documents_only
                || matches!(entry.kind, Kind::Folder | Kind::Prefab | Kind::Scene))
                && unicase::UniCase::unicode(entry.name.to_string_lossy())
                    .to_folded_case()
                    .contains(&filter)
        })
    }
    pub fn select(&mut self, name: &OsString) -> bool {
        if self.loading || self.error.is_some() || !self.entries.iter().any(|e| &e.name == name) {
            return false;
        }
        self.selected = Some(name.clone());
        self.notice = None;
        true
    }
    pub fn selected_entry(&self) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|e| Some(&e.name) == self.selected.as_ref())
    }
}
fn relative(path: &Path) -> bool {
    path.components().all(|c| matches!(c, Component::Normal(_)))
}

/// Does not follow links or read file content. The root is project-validated.
/// Rechecks every directory ancestor; concurrent filesystem replacement is not sandboxed.
pub(super) fn list(root: &Path, path: &Path) -> Result<Listing, String> {
    let directory = directory(root, path)?;
    enumerate(directory)
}
pub(super) fn directory(root: &Path, path: &Path) -> Result<PathBuf, String> {
    if !relative(path) {
        return Err("Location escapes the source root".into());
    }
    let mut directory = root.to_path_buf();
    for part in std::iter::once(None).chain(path.components().map(Some)) {
        if let Some(part) = part {
            directory.push(part);
        }
        let metadata = std::fs::symlink_metadata(&directory)
            .map_err(|e| format!("Cannot read source directory: {e}"))?;
        if metadata.is_symlink() || !metadata.is_dir() {
            return Err("Source navigation requires a regular directory, not a link".into());
        }
    }
    Ok(directory)
}
fn enumerate(directory: PathBuf) -> Result<Listing, String> {
    let mut entries = Vec::new();
    let mut bytes = 0usize;
    for (count, entry) in std::fs::read_dir(&directory)
        .map_err(|e| format!("Cannot read source directory: {e}"))?
        .enumerate()
    {
        if count >= MAX_SCAN {
            return Err(
                "Folder exceeds the 4096-entry scan limit; narrow the source directory".into(),
            );
        }
        let entry = entry.map_err(|e| format!("Cannot list source entry: {e}"))?;
        let name = entry.file_name();
        bytes = bytes.saturating_add(name.as_encoded_bytes().len());
        if bytes > MAX_NAME_BYTES {
            return Err("Folder names exceed the 1 MiB listing budget".into());
        }
        if name.to_str().is_some_and(|n| n.ends_with(".meta")) {
            continue;
        }
        let kind = match entry.file_type() {
            Ok(t) if t.is_symlink() => Kind::Unavailable,
            Ok(t) if t.is_dir() || t.is_file() => classify(&name, t.is_dir()),
            _ => Kind::Unavailable,
        };
        let bytes = (kind != Kind::Folder && kind != Kind::Unavailable)
            .then(|| entry.metadata().ok().map(|m| m.len()))
            .flatten();
        let mut sidecar = name.clone();
        sidecar.push(".meta");
        let tracked = std::fs::symlink_metadata(directory.join(sidecar))
            .is_ok_and(|m| m.is_file() && !m.is_symlink());
        entries.push(Entry {
            name,
            kind,
            bytes,
            tracked,
        });
    }
    entries
        .sort_by(|a, b| (a.kind != Kind::Folder, &a.name).cmp(&(b.kind != Kind::Folder, &b.name)));
    let truncated = entries.len() > MAX_ENTRIES;
    entries.truncate(MAX_ENTRIES);
    Ok(Listing { entries, truncated })
}

fn classify(name: &std::ffi::OsStr, directory: bool) -> Kind {
    match name.to_str() {
        Some(n) if !n.contains(['#', ':', '\\']) && !n.chars().any(char::is_control) => {
            if directory {
                Kind::Folder
            } else if n.ends_with(".scene.ron") {
                Kind::Scene
            } else if n.ends_with(".prefab.ron") {
                Kind::Prefab
            } else {
                Kind::File
            }
        }
        _ => Kind::Unavailable,
    }
}

#[cfg(test)]
mod tests;
