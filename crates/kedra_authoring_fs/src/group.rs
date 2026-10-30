//! Kedra request admission/correlation around the maintained transaction journal.
use super::*;
use fs_transaction::{ChangeSet, FileOp, Journal, Storage, exec::block_on};

const LIMIT: u64 = 16 * 1024 * 1024;
#[cfg(test)]
mod inventory_tests;
#[cfg(test)]
mod tests;
pub(super) struct Session {
    pub root: PathBuf,
    pub intent: PathBuf,
    pub journal: Journal,
    pub staging: PathBuf,
}
impl Session {
    pub fn new(root: &Path) -> Result<Self, SaveError> {
        let metadata = std::fs::symlink_metadata(root).map_err(refused)?;
        if !metadata.is_dir() || metadata.is_symlink() {
            return Err(refused("Save root is not a regular directory"));
        }
        let root = std::fs::canonicalize(root).map_err(refused)?;
        directories(&root)?;
        let parent = root
            .parent()
            .ok_or_else(|| refused("Save root has no parent"))?;
        let name = root
            .file_name()
            .ok_or_else(|| refused("Save root has no name"))?;
        let mut intent = name.to_os_string();
        intent.push(".kedra-save.request.ron");
        let mut staging = name.to_os_string();
        staging.push(".kedra-save.staging");
        let staging = parent.join(staging);
        let journal = Journal::named(".kedra-save.journal")
            .and_then(|journal| journal.kept_in(&staging))
            .map_err(refused)?;
        Ok(Self {
            intent: parent.join(intent),
            staging,
            root,
            journal,
        })
    }
    pub(super) fn pending(&self) -> Result<Option<CommitRequest>, SaveError> {
        match std::fs::symlink_metadata(&self.intent) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(uncertain(error)),
            Ok(_) => ron::from_str(&bounded_text(&self.intent)?)
                .map(Some)
                .map_err(uncertain),
        }
    }
    pub(super) fn journal_present(&self) -> Result<bool, SaveError> {
        match std::fs::symlink_metadata(self.journal.path_in(&self.root)) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(uncertain(error)),
        }
    }
    fn clear(&self) -> Result<(), SaveError> {
        std::fs::remove_file(&self.intent).map_err(uncertain)?;
        sync_directory(
            self.intent
                .parent()
                .ok_or_else(|| uncertain("Intent parent missing"))?,
        )
        .map_err(uncertain)
    }
}
fn bounded_text(path: &Path) -> Result<String, SaveError> {
    let metadata = std::fs::symlink_metadata(path).map_err(uncertain)?;
    if !metadata.is_file() || metadata.is_symlink() || metadata.len() > LIMIT {
        return Err(uncertain(
            "Save recovery record is not a bounded regular file",
        ));
    }
    let mut text = String::new();
    File::open(path)
        .map_err(uncertain)?
        .take(LIMIT + 1)
        .read_to_string(&mut text)
        .map_err(uncertain)?;
    if text.len() as u64 > LIMIT {
        return Err(uncertain("Save recovery record exceeds its byte limit"));
    }
    Ok(text)
}
fn same(a: &CommitRequest, b: &CommitRequest) -> Result<bool, SaveError> {
    // The writer additionally freezes discovered sidecars for restart recovery.
    // They supplement, never replace, explicit caller preconditions.
    let mut pinned = b.clone();
    pinned.checks.retain(|check| {
        !check.path.ends_with(".meta")
            || a.checks.iter().any(|original| original.path == check.path)
    });
    Ok(ron::to_string(a).map_err(refused)? == ron::to_string(&pinned).map_err(refused)?)
}
fn pin_sidecars(root: &Path, request: &CommitRequest) -> Result<CommitRequest, SaveError> {
    let mut pinned = request.clone();
    for (count, entry) in walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_depth(65)
        .into_iter()
        .enumerate()
    {
        if count >= 8192 {
            return Err(refused("Save inventory exceeds its entry limit"));
        }
        let entry = entry.map_err(refused)?;
        if entry.file_type().is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "meta")
        {
            if entry.metadata().map_err(refused)?.len() > 65_536 {
                return Err(refused("Source sidecar exceeds its byte limit"));
            }
            let path = entry
                .path()
                .strip_prefix(root)
                .map_err(refused)?
                .to_str()
                .ok_or_else(|| refused("Source sidecar path must be Unicode"))?
                .replace(std::path::MAIN_SEPARATOR, "/");
            if !pinned.checks.iter().any(|check| check.path == path) {
                pinned.checks.push(kedra_authoring::persistence::FileCheck {
                    path,
                    expected_hash: Some(hash(entry.path())?),
                });
            }
        }
    }
    Ok(pinned)
}
fn admit(request: &CommitRequest) -> Result<(), SaveError> {
    admit_profile(request, false)
}
fn admit_profile(request: &CommitRequest, new_document: bool) -> Result<(), SaveError> {
    if request.files.is_empty()
        || request.files.len() > 128
        || request.checks.len() > 8192
        || request.expected_sources.len() > 128
    {
        return Err(refused(
            "Save requires 1..=128 existing document replacements",
        ));
    }
    request
        .expected_generation
        .checked_add(1)
        .ok_or_else(|| refused("Save generation exhausted"))?;
    let mut paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for file in &request.files {
        portable(&file.path)?;
        let id = file
            .document
            .ok_or_else(|| refused("Save cannot write a catalog"))?;
        let doc = file
            .source
            .as_deref()
            .map(PrefabDocument::parse)
            .transpose()
            .map_err(refused)?;
        let suffix_matches = doc.as_ref().map_or_else(
            || file.path.ends_with(".scene.ron") || file.path.ends_with(".prefab.ron"),
            |doc| {
                file.path.ends_with(match doc.kind {
                    kedra_scene::document::DocumentKind::Prefab => ".prefab.ron",
                    kedra_scene::document::DocumentKind::Scene => ".scene.ron",
                })
            },
        );
        if !paths.insert(&file.path)
            || !ids.insert(id)
            || id.0.is_nil()
            || doc.as_ref().is_some_and(|doc| doc.id != id)
            || (!new_document && (file.expected_hash.is_none() || file.source.is_none()))
            || (file.expected_hash.is_none() && file.source.is_none())
            || request.expected_sources.contains(&file.path) != file.expected_hash.is_some()
            || !request
                .checks
                .iter()
                .any(|check| check.path == file.path && check.expected_hash == file.expected_hash)
            || !suffix_matches
        {
            return Err(refused(
                "Save requires unique admitted identities, paths and exact existing-source preconditions",
            ));
        }
    }
    Ok(())
}

/// Hold a shared admission lease while reading sources. Unfinished Save records
/// refuse ordinary admission; only an exact pinned Save may inspect its own group.
/// This never recovers or changes authoring data. Drop releases the lease.
pub fn read_guard(root: &Path, pending: Option<&CommitRequest>) -> Result<File, SaveError> {
    let session = Session::new(root)?;
    let guard = lease(&session.root, true)?;
    if let Some(request) = session.pending()? {
        if !pending
            .map(|value| same(value, &request))
            .transpose()?
            .unwrap_or(false)
        {
            return Err(uncertain(
                "Unfinished source Save. Run kedra recover-save <source-root>, then reopen the project",
            ));
        }
    } else if session.journal_present()? {
        return Err(uncertain(
            "Save journal has no matching request; preserve recovery records for inspection",
        ));
    }
    Ok(guard)
}

/// Save the complete existing-source group under one exclusive lease. The durable
/// intent and upstream write-ahead journal survive interruption. Caught failures
/// roll back; uncertain outcomes retain exact request bytes for Retry Save/recovery.
/// Ordinary filesystem readers are not isolated; Kedra admission uses read_guard.
pub fn commit_existing(root: &Path, request: &CommitRequest) -> Result<Receipt, SaveError> {
    admit(request)?;
    commit(root, request, None)
}
/// Publish a coupled document group, including creations and history removals.
/// Uses the same lease, exact request, interruption journal and receipt protocol as
/// existing Save. New names use exclusive creation. Catalogs, sidecar changes and
/// directory creation are refused; target parent directories must already exist.
/// An uncertain result must be retried with the identical request or explicitly recovered.
pub fn commit_documents(root: &Path, request: &CommitRequest) -> Result<Receipt, SaveError> {
    admit_profile(request, true)?;
    commit(root, request, None)
}
/// Save one existing environment source through the ordinary source lease,
/// conflict checks, sidecar pinning and interruption journal. This never creates
/// a new identity or publishes a runtime delivery. Retry uncertain outcomes with
/// the identical request; explicit recovery uses `recover_documents`.
pub fn commit_environment(
    root: &Path,
    request: &CommitRequest,
    baseline: &kedra_authoring::persistence::SourceAsset,
) -> Result<Receipt, SaveError> {
    admit_environment(request)?;
    if request.files[0].path != baseline.path
        || request.files[0].expected_hash.as_ref() != Some(&baseline.hash)
    {
        return Err(refused(
            "Environment identity baseline differs from the Save request",
        ));
    }
    commit(root, request, Some(baseline))
}
fn admit_environment(request: &CommitRequest) -> Result<(), SaveError> {
    if request.files.len() != 1
        || request.checks.len() > 8192
        || request.expected_sources.len() > 128
    {
        return Err(refused("Environment Save requires one existing source"));
    }
    request
        .expected_generation
        .checked_add(1)
        .ok_or_else(|| refused("Save generation exhausted"))?;
    let file = &request.files[0];
    portable(&file.path)?;
    if file.document.is_some()
        || !file.path.ends_with(".environment.ron")
        || file.expected_hash.is_none()
        || !request
            .checks
            .iter()
            .any(|check| check.path == file.path && check.expected_hash == file.expected_hash)
    {
        return Err(refused(
            "Environment Save requires exact existing-source preconditions",
        ));
    }
    kedra_scene::environment::Environment::parse(
        file.source
            .as_deref()
            .ok_or_else(|| refused("Environment deletion is not a Save operation"))?,
    )
    .map_err(refused)?;
    Ok(())
}
fn commit(
    root: &Path,
    request: &CommitRequest,
    environment: Option<&kedra_authoring::persistence::SourceAsset>,
) -> Result<Receipt, SaveError> {
    let session = Session::new(root)?;
    let _lease = lease(&session.root, false)?;
    if let Some(baseline) = environment {
        let file = &request.files[0];
        let saved = file.source.as_deref().map(content_hash);
        let actual = optional_hash(&session.root.join(&file.path))?;
        let expected = if actual == saved {
            saved.as_deref()
        } else {
            file.expected_hash.as_deref()
        }
        .ok_or_else(|| refused("Environment source disappeared"))?;
        crate::source::check_inventory(&session.root, Path::new(&file.path), expected, baseline.id)
            .map_err(refused)?;
        if baseline.id.is_none()
            && std::fs::symlink_metadata(session.root.join(format!("{}.meta", file.path))).is_ok()
        {
            return Err(refused("Environment sidecar appeared after admission"));
        }
    }
    if let Some(pending) = session.pending()? {
        if !same(request, &pending)? {
            return Err(uncertain("Another Save request requires recovery"));
        }
        return publish(&session, &pending, true).map_err(|mut error| {
            error.uncertain = true;
            error
        });
    }
    if session.journal_present()? {
        return Err(uncertain("Save journal has no matching request"));
    }
    let pinned = pin_sidecars(&session.root, request)?;
    let request = &pinned;
    validate(&session.root, request)?;
    let mut old = 0;
    let mut new = 0;
    for file in &request.files {
        let actual = optional_hash(&session.root.join(&file.path))?;
        let saved = file.source.as_deref().map(content_hash);
        if actual == file.expected_hash && actual != saved {
            old += 1;
        } else if actual != file.expected_hash {
            new += 1;
        }
        if actual.is_some()
            && std::fs::metadata(session.root.join(&file.path))
                .map_err(refused)?
                .permissions()
                .readonly()
        {
            return Err(refused("Source is read-only"));
        }
    }
    if old > 0 && new > 0 {
        return Err(refused(
            "Mixed published source group has no matching recovery request",
        ));
    }
    if old == 0 {
        for file in &request.files {
            let path = session.root.join(&file.path);
            if file.source.is_some() {
                sync_existing_file(&path).map_err(uncertain)?;
            }
            sync_directory(
                path.parent()
                    .ok_or_else(|| uncertain("Source parent missing"))?,
            )
            .map_err(uncertain)?;
        }
        return receipt(request);
    }
    let text = ron::to_string(request).map_err(refused)?;
    if text.len() as u64 > LIMIT {
        return Err(refused("Save recovery request exceeds its byte limit"));
    }
    let mut intent = tempfile::NamedTempFile::new_in(
        session
            .intent
            .parent()
            .ok_or_else(|| refused("Intent parent missing"))?,
    )
    .map_err(refused)?;
    intent
        .write_all(text.as_bytes())
        .and_then(|()| intent.as_file().sync_all())
        .map_err(refused)?;
    intent
        .persist_noclobber(&session.intent)
        .map_err(|error| uncertain(error.error))?;
    sync_directory(
        session
            .intent
            .parent()
            .ok_or_else(|| uncertain("Intent parent missing"))?,
    )
    .map_err(uncertain)?;
    publish(&session, request, false)
}

/// Explicitly authorize completion of the pinned interrupted Save, without loading
/// project code or restoring unsaved editor history. Conflicts preserve recovery
/// records and external bytes. No records means no work. Never called by Open.
pub fn recover_documents(root: &Path) -> Result<Option<Receipt>, SaveError> {
    let session = Session::new(root)?;
    let _lease = lease(&session.root, false)?;
    let Some(request) = session.pending()? else {
        if session.journal_present()? {
            return Err(uncertain("Save journal has no matching request"));
        }
        return Ok(None);
    };
    if request.files.iter().any(|file| file.document.is_none()) {
        admit_environment(&request)
    } else {
        admit_profile(&request, true)
    }
    .map_err(|error| uncertain(error.message))?;
    publish(&session, &request, true)
        .map(Some)
        .map_err(|mut error| {
            error.uncertain = true;
            error
        })
}
fn publish(
    session: &Session,
    request: &CommitRequest,
    recovering: bool,
) -> Result<Receipt, SaveError> {
    let backend = storage::Guarded { session, request };
    validate(&session.root, request).map_err(|error| uncertain(error.message))?;
    if session.journal_present()? {
        let bytes =
            storage::read_journal(&session.journal.path_in(&session.root)).map_err(uncertain)?;
        let ops = fs_transaction::journal::decode(&bytes).map_err(uncertain)?;
        let mut paths = BTreeSet::new();
        if ops.is_empty()
            || ops.iter().any(|op| match op {
                FileOp::Write { path, bytes } => {
                    !paths.insert(path)
                        || !request.files.iter().any(|file| {
                            Path::new(&file.path) == path
                                && file
                                    .source
                                    .as_ref()
                                    .is_some_and(|text| text.as_bytes() == bytes)
                        })
                }
                FileOp::Remove { path } => {
                    !paths.insert(path)
                        || !request
                            .files
                            .iter()
                            .any(|file| Path::new(&file.path) == path && file.source.is_none())
                }
                _ => true,
            })
        {
            return Err(uncertain(
                "Save journal does not match its pinned document replacements",
            ));
        }
        block_on(session.journal.recover(&backend, &session.root)).map_err(uncertain)?;
    }
    let mut changes = ChangeSet::new();
    for file in &request.files {
        if optional_hash(&session.root.join(&file.path))?
            != file.source.as_deref().map(content_hash)
        {
            if let Some(source) = &file.source {
                changes.write(&file.path, source.as_bytes());
            } else {
                changes.remove(&file.path);
            }
        }
    }
    validate(&session.root, request).map_err(|error| uncertain(error.message))?;
    if !changes.ops().is_empty() {
        block_on(backend.create_dir_all(&session.staging)).map_err(uncertain)?;
    }
    if !changes.ops().is_empty()
        && let Err(error) = block_on(session.journal.apply(&changes, &backend, &session.root))
    {
        let restored = request.files.iter().all(|file| {
            optional_hash(&session.root.join(&file.path))
                .is_ok_and(|hash| hash == file.expected_hash)
        });
        if !recovering
            && !matches!(error, fs_transaction::Error::Torn { .. })
            && !session.journal_present()?
            && restored
        {
            session.clear()?;
            return Err(refused(error));
        }
        return Err(uncertain(error));
    }
    validate(&session.root, request).map_err(|error| uncertain(error.message))?;
    for file in &request.files {
        if optional_hash(&session.root.join(&file.path))
            .map_err(|error| uncertain(error.message))?
            != file.source.as_deref().map(content_hash)
        {
            return Err(uncertain("Saved group changed before acknowledgment"));
        }
    }
    let receipt = receipt(request)?;
    session.clear()?;
    Ok(receipt)
}
fn receipt(request: &CommitRequest) -> Result<Receipt, SaveError> {
    Ok(Receipt {
        operation: request.operation,
        generation: request
            .expected_generation
            .checked_add(1)
            .ok_or_else(|| refused("Save generation exhausted"))?,
        files: request
            .files
            .iter()
            .map(|file| SavedFile {
                document: file.document,
                path: file.path.clone(),
                revision: file.revision,
                hash: file.source.as_deref().map(content_hash),
            })
            .collect(),
    })
}
