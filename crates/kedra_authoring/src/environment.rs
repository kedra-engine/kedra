//! Source-owned edits of existing environment assets. No renderer or filesystem access.
use crate::persistence::{
    CommitFile, CommitRequest, FileCheck, Receipt, SourceSnapshot, content_hash,
};
use bevy::reflect::{ParsedPath, PartialReflect, ReflectPath};
use kedra_scene::environment::Environment;
use std::collections::BTreeMap;

/// One project's environment working copies, shared by all inspecting views.
/// Values remain separate from the published inventory and runtime assets. Drop
/// discards drafts; hosts must settle dirty copies before retiring the project.
#[derive(Default, Clone)]
pub struct Edits {
    entries: BTreeMap<String, Entry>,
    pending: Option<CommitRequest>,
}
#[derive(Clone)]
struct Entry {
    value: Environment,
    saved: String,
    revision: u64,
    undo: Vec<Environment>,
    redo: Vec<Environment>,
}
impl Edits {
    /// Admit one bounded source from the existing catalog, preserving malformed
    /// source bytes on failure. Repeated inspection reuses the same working copy.
    pub fn open(&mut self, source: &SourceSnapshot, path: &str) -> Result<(), String> {
        if self.entries.contains_key(path) {
            return Ok(());
        }
        let text = source
            .environments
            .get(path)
            .ok_or("Environment source is unavailable")?;
        let asset = source
            .assets
            .iter()
            .find(|asset| asset.path == path)
            .ok_or("Environment is outside the source catalog")?;
        if text.len() > 65_536
            || asset.hash != content_hash(text)
            || asset.bytes != text.len() as u64
        {
            return Err("Environment source differs from its admitted inventory".into());
        }
        let value = Environment::parse(text).map_err(|e| e.to_string())?;
        let saved = encode(&value)?;
        self.entries.insert(
            path.into(),
            Entry {
                value,
                saved,
                revision: 1,
                undo: vec![],
                redo: vec![],
            },
        );
        Ok(())
    }
    /// Borrow the current root and its monotonic revision, never a runtime instance.
    pub fn value(&self, path: &str) -> Option<(&Environment, u64)> {
        self.entries
            .get(path)
            .map(|entry| (&entry.value, entry.revision))
    }
    /// Whether a source has accepted edits not yet acknowledged by Save.
    pub fn dirty(&self, path: &str) -> bool {
        self.entries
            .get(path)
            .is_some_and(|entry| encode(&entry.value).is_ok_and(|text| text != entry.saved))
    }
    /// Whether project retirement would lose accepted edits or a pinned Save.
    pub fn unsettled(&self) -> bool {
        self.pending.is_some() || self.entries.keys().any(|path| self.dirty(path))
    }
    /// Apply one exact reflected field proposal. Invalid values, stale revisions,
    /// schema-version edits and pending Save leave all values/history unchanged.
    pub fn edit(
        &mut self,
        path: &str,
        revision: u64,
        field: &ParsedPath,
        value: &dyn PartialReflect,
    ) -> Result<(), String> {
        if self.pending.is_some() {
            return Err("Finish environment Save before editing.".into());
        }
        let entry = self
            .entries
            .get_mut(path)
            .ok_or("Environment is not admitted")?;
        if entry.revision != revision {
            return Err("Environment field binding is stale".into());
        }
        let mut candidate = entry.value.clone();
        field
            .reflect_element_mut(&mut candidate)
            .map_err(|e| e.to_string())?
            .try_apply(value)
            .map_err(|e| e.to_string())?;
        candidate.validate().map_err(|e| e.to_string())?;
        if encode(&candidate)? == encode(&entry.value)? {
            return Ok(());
        }
        let revision = entry
            .revision
            .checked_add(1)
            .ok_or("Environment revision exhausted")?;
        entry.undo.push(entry.value.clone());
        if entry.undo.len() > 64 {
            entry.undo.remove(0);
        }
        entry.redo.clear();
        entry.value = candidate;
        entry.revision = revision;
        Ok(())
    }
    /// Source history availability for the inspected asset (not the active scene).
    pub fn can_history(&self, path: &str, redo: bool) -> bool {
        self.pending.is_none()
            && self.entries.get(path).is_some_and(|e| {
                if redo {
                    !e.redo.is_empty()
                } else {
                    !e.undo.is_empty()
                }
            })
    }
    /// Undo/Redo one accepted root edit without writing any files.
    pub fn history(&mut self, path: &str, redo: bool) -> Result<(), String> {
        if !self.can_history(path, redo) {
            return Err("No environment history is available".into());
        }
        let entry = self
            .entries
            .get_mut(path)
            .ok_or("Environment is not admitted")?;
        let revision = entry
            .revision
            .checked_add(1)
            .ok_or("Environment revision exhausted")?;
        let (from, to) = if redo {
            (&mut entry.redo, &mut entry.undo)
        } else {
            (&mut entry.undo, &mut entry.redo)
        };
        let value = from.pop().ok_or("Environment history is empty")?;
        to.push(std::mem::replace(&mut entry.value, value));
        entry.revision = revision;
        Ok(())
    }
    /// Pin an existing-source Save against the complete admitted baseline. Retries
    /// return the identical request; another source cannot steal a pending Save.
    pub fn prepare_save(
        &mut self,
        source: &SourceSnapshot,
        path: &str,
    ) -> Result<CommitRequest, String> {
        if let Some(request) = &self.pending {
            if request.files[0].path != path {
                return Err("Another environment Save is pending".into());
            }
            return Ok(request.clone());
        }
        source
            .generation
            .checked_add(1)
            .ok_or("Source generation exhausted")?;
        if !self.dirty(path) {
            return Err("Environment has no unsaved changes".into());
        }
        let entry = self
            .entries
            .get(path)
            .ok_or("Environment is not admitted")?;
        let asset = source
            .assets
            .iter()
            .find(|asset| asset.path == path)
            .ok_or("Environment left the source catalog")?;
        let request = CommitRequest {
            operation: uuid::Uuid::new_v4(),
            expected_generation: source.generation,
            expected_sources: source
                .documents
                .iter()
                .map(|doc| doc.path.clone())
                .collect(),
            checks: source
                .documents
                .iter()
                .map(|doc| FileCheck {
                    path: doc.path.clone(),
                    expected_hash: Some(doc.hash.clone()),
                })
                .chain(source.assets.iter().map(|asset| FileCheck {
                    path: asset.path.clone(),
                    expected_hash: Some(asset.hash.clone()),
                }))
                .collect(),
            files: vec![CommitFile {
                document: None,
                path: path.into(),
                revision: entry.revision,
                expected_hash: Some(asset.hash.clone()),
                source: Some(encode(&entry.value)?),
            }],
        };
        self.pending = Some(request.clone());
        Ok(request)
    }
    /// Inspect a pinned request for exact retry/recovery correlation.
    pub fn pending(&self) -> Option<&CommitRequest> {
        self.pending.as_ref()
    }
    /// Release a definitely refused request; uncertain publication must retain it.
    pub fn refused(&mut self) {
        self.pending = None;
    }
    /// Acknowledge only the exact submitted bytes/revision and update the shared
    /// source inventory. A foreign or malformed receipt leaves all state intact.
    pub fn acknowledge(
        &mut self,
        source: &mut SourceSnapshot,
        receipt: &Receipt,
    ) -> Result<(), String> {
        let request = self
            .pending
            .as_ref()
            .ok_or("No environment Save is pending")?;
        let file = &request.files[0];
        let text = file
            .source
            .as_ref()
            .ok_or("Environment Save has no source")?;
        let hash = content_hash(text);
        if receipt.operation != request.operation
            || receipt.generation != request.expected_generation + 1
            || receipt.files.len() != 1
            || receipt.files[0].document.is_some()
            || receipt.files[0].path != file.path
            || receipt.files[0].revision != file.revision
            || receipt.files[0].hash.as_ref() != Some(&hash)
        {
            return Err("Environment Save receipt differs from its pinned request".into());
        }
        let asset = source
            .assets
            .iter_mut()
            .find(|asset| asset.path == file.path)
            .ok_or("Saved environment left the catalog")?;
        asset.hash = hash;
        asset.bytes = text.len() as u64;
        source.environments.insert(file.path.clone(), text.clone());
        source.generation = receipt.generation;
        self.entries
            .get_mut(&file.path)
            .ok_or("Saved environment was retired")?
            .saved = text.clone();
        self.pending = None;
        Ok(())
    }
}
fn encode(value: &Environment) -> Result<String, String> {
    value.validate().map_err(|e| e.to_string())?;
    ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())
}
