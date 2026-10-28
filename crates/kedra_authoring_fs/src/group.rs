//! Kedra request admission/correlation around the maintained transaction journal.
use super::*;
use fs_transaction::{ChangeSet, FileOp, Journal, Storage, exec::block_on};

const LIMIT: u64 = 16 * 1024 * 1024;
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
    fn pending(&self) -> Result<Option<CommitRequest>, SaveError> {
        match std::fs::symlink_metadata(&self.intent) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(uncertain(error)),
            Ok(_) => ron::from_str(&bounded_text(&self.intent)?)
                .map(Some)
                .map_err(uncertain),
        }
    }
    fn journal_present(&self) -> Result<bool, SaveError> {
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
        let text = file
            .source
            .as_ref()
            .ok_or_else(|| refused("Save cannot delete a source"))?;
        let doc = PrefabDocument::parse(text).map_err(refused)?;
        let suffix = match doc.kind {
            kedra_scene::document::DocumentKind::Prefab => ".prefab.ron",
            kedra_scene::document::DocumentKind::Scene => ".scene.ron",
        };
        if !paths.insert(&file.path)
            || !ids.insert(id)
            || doc.id != id
            || file.expected_hash.is_none()
            || !request.expected_sources.contains(&file.path)
            || !request
                .checks
                .iter()
                .any(|check| check.path == file.path && check.expected_hash == file.expected_hash)
            || !file.path.ends_with(suffix)
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
    let session = Session::new(root)?;
    let _lease = lease(&session.root, false)?;
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
        let actual = hash(&session.root.join(&file.path))?;
        let saved = file.source.as_deref().map(content_hash);
        if Some(&actual) == file.expected_hash.as_ref() && Some(&actual) != saved.as_ref() {
            old += 1;
        } else if Some(&actual) != file.expected_hash.as_ref() {
            new += 1;
        }
        if std::fs::metadata(session.root.join(&file.path))
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
            sync_existing_file(&path).map_err(uncertain)?;
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
pub fn recover_existing(root: &Path) -> Result<Option<Receipt>, SaveError> {
    let session = Session::new(root)?;
    let _lease = lease(&session.root, false)?;
    let Some(request) = session.pending()? else {
        if session.journal_present()? {
            return Err(uncertain("Save journal has no matching request"));
        }
        return Ok(None);
    };
    admit(&request).map_err(|error| uncertain(error.message))?;
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
        let source = file
            .source
            .as_ref()
            .ok_or_else(|| uncertain("Saved source missing"))?;
        if hash(&session.root.join(&file.path))? != content_hash(source) {
            changes.write(&file.path, source.as_bytes());
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
            hash(&session.root.join(&file.path)).ok().as_ref() == file.expected_hash.as_ref()
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
        if hash(&session.root.join(&file.path)).map_err(|error| uncertain(error.message))?
            != file
                .source
                .as_deref()
                .map(content_hash)
                .ok_or_else(|| uncertain("Saved source missing"))?
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
