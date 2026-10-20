//! Bounded local recent-project history. It is not part of project sources.
use crate::{Project, ProjectError, io};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Maximum retained entries, newest first.
pub const MAX_RECENT: usize = 20;

/// Last successful project identity and location; existence is checked on open.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentProject {
    /// Manifest name when last opened.
    pub name: String,
    /// Canonical root when last opened; moved projects are not deleted silently.
    pub root: PathBuf,
}

/// Versioned history. Mutating memory does not implicitly write a file.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecentProjects {
    format: u32,
    entries: Vec<RecentProject>,
}

impl Default for RecentProjects {
    fn default() -> Self {
        Self {
            format: 1,
            entries: Vec::new(),
        }
    }
}

impl RecentProjects {
    /// Entries in most-recently-opened order.
    pub fn entries(&self) -> &[RecentProject] {
        &self.entries
    }

    /// Add a successfully opened project, deduplicating by canonical root.
    pub fn remember(&mut self, project: &Project) {
        self.entries.retain(|entry| entry.root != project.root);
        self.entries.insert(
            0,
            RecentProject {
                name: project.manifest.name.clone(),
                root: project.root.clone(),
            },
        );
        self.entries.truncate(MAX_RECENT);
    }

    /// Read bounded local history. Missing files mean a first launch; corrupted
    /// or unsupported history is reported, not overwritten as an empty list.
    /// Static nonregular inputs are rejected before opening; callers must not
    /// concurrently replace input paths. This is not a filesystem sandbox.
    pub fn load(path: &Path) -> Result<Self, ProjectError> {
        let file = match crate::storage::open_regular(path, "Cannot read recent projects") {
            Ok(file) => file,
            Err(ProjectError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                return Ok(Self::default());
            }
            Err(error) => return Err(error),
        };
        let mut source = Vec::new();
        file.take(65537)
            .read_to_end(&mut source)
            .map_err(|e| io("Cannot read recent projects", e))?;
        if source.len() > 64 * 1024 {
            return Err(ProjectError::Invalid(
                "Recent-project history exceeds 64 KiB.".into(),
            ));
        }
        let source =
            std::str::from_utf8(&source).map_err(|e| ProjectError::Invalid(e.to_string()))?;
        let history: Self = toml::from_str(source)
            .map_err(|e| ProjectError::Invalid(format!("Cannot read recent projects: {e}")))?;
        if history.format != 1 || history.entries.len() > MAX_RECENT {
            return Err(ProjectError::Invalid(
                "Unsupported recent-project history.".into(),
            ));
        }
        let mut roots = std::collections::BTreeSet::new();
        for entry in &history.entries {
            crate::validate_name(&entry.name)?;
            if !entry.root.is_absolute() || !roots.insert(&entry.root) {
                return Err(ProjectError::Invalid(
                    "Recent projects must have unique absolute roots.".into(),
                ));
            }
        }
        Ok(history)
    }

    /// Atomically replace local history via a synchronized temporary file.
    /// The caller must surface errors; a failed save does not close a project.
    pub fn save(&self, path: &Path) -> Result<(), ProjectError> {
        let parent = path
            .parent()
            .ok_or_else(|| ProjectError::Invalid("Missing history directory.".into()))?;
        let source = toml::to_string(self).map_err(|e| ProjectError::Invalid(e.to_string()))?;
        fs::create_dir_all(parent).map_err(|e| io("Cannot create preferences directory", e))?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| io("Cannot prepare recent-project history", e))?;
        temp.write_all(source.as_bytes())
            .map_err(|e| io("Cannot write recent-project history", e))?;
        temp.as_file()
            .sync_all()
            .map_err(|e| io("Cannot synchronize recent-project history", e))?;
        temp.persist(path)
            .map_err(|e| io("Cannot publish recent-project history", e.error))?;
        Ok(())
    }
}
