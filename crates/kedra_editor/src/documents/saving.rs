//! Async source-owned Save, with pinned retries and exact acknowledgment.
use super::*;
use kedra_authoring::persistence::{CommitRequest, Receipt, SourceSnapshot};
use kedra_authoring_fs::SaveError;
#[cfg(test)]
mod tests;
type SaveResult = Result<Receipt, SaveError>;
#[derive(Default)]
pub(super) struct State {
    task: Option<Task<SaveResult>>,
    environment: bool,
    all: bool,
    pub error: Option<String>,
}
impl Documents {
    pub(crate) fn save_error(&self) -> Option<&str> {
        self.save.error.as_deref()
    }
    pub(crate) fn source_titles(&self) -> Vec<(ViewId, String)> {
        let Some(scope) = &self.scope else {
            return Vec::new();
        };
        self.opened
            .values()
            .map(|opened| {
                let document = scope.working.documents().get(&opened.document);
                let title = document
                    .map(|doc| doc.document.title.clone())
                    .unwrap_or_else(|| {
                        format!("{} (removed)", opened.graph.nodes[&opened.graph.root].name)
                    });
                (
                    opened.view,
                    format!(
                        "{}{}",
                        title,
                        if scope.working.dirty(opened.document) {
                            " *"
                        } else {
                            ""
                        }
                    ),
                )
            })
            .collect()
    }
    pub(crate) fn save_busy(&self) -> bool {
        self.save.task.is_some() || self.save.all
    }
    pub(crate) fn save_pending(&self) -> bool {
        self.environments.pending().is_some()
            || self
                .scope
                .as_ref()
                .is_some_and(|scope| scope.pending().is_some())
    }
    pub(crate) fn can_save(&self) -> bool {
        if let Some(path) = self.selected_environment() {
            return !self.save_busy()
                && (self.environments.pending().is_some() || self.environments.dirty(&path));
        }
        !self.save_busy()
            && self.scope.as_ref().is_some_and(|scope| {
                scope.pending().is_some()
                    || self.save_target().is_some_and(|id| {
                        scope
                            .working
                            .save_group(id)
                            .iter()
                            .any(|member| scope.working.dirty(*member))
                    })
            })
    }
    pub(crate) fn begin_save(&mut self, world: &World, root: PathBuf) -> Result<(), String> {
        if let Some(path) = self
            .environments
            .pending()
            .map(|request| request.files[0].path.clone())
            .or_else(|| self.selected_environment())
        {
            return self.begin_environment_save(world, root, path);
        }
        self.begin_save_for(world, root, self.save_target(), false)
    }
    fn begin_environment_save(
        &mut self,
        world: &World,
        root: PathBuf,
        path: String,
    ) -> Result<(), String> {
        if self.save_busy()
            || self.creating()
            || self.opening()
            || self
                .scope
                .as_ref()
                .is_some_and(|scope| scope.pending().is_some())
        {
            return Err("Wait for the current source operation before Save".into());
        }
        let baseline = self
            .scope
            .as_ref()
            .ok_or("Source workspace unavailable")?
            .source
            .clone();
        let retry = self.environments.pending().is_some();
        let request = self.environments.prepare_save(&baseline, &path)?;
        let asset = baseline
            .assets
            .iter()
            .find(|asset| asset.path == path)
            .ok_or("Environment source unavailable")?
            .clone();
        self.save.error = None;
        self.save.environment = true;
        self.save.task = Some(
            Wakeup::for_world(world).spawn(IoTaskPool::get(), async move {
                verify(&root, &baseline, &request)
                    .and_then(|()| kedra_authoring_fs::commit_environment(&root, &request, &asset))
                    .map_err(|mut error| {
                        error.uncertain |= retry;
                        error
                    })
            }),
        );
        Ok(())
    }
    pub(crate) fn unsaved_paths(&self) -> Vec<String> {
        let Some(scope) = &self.scope else {
            return vec![];
        };
        let mut paths: Vec<_> = scope
            .working
            .documents()
            .keys()
            .filter(|id| scope.working.dirty(**id))
            .filter_map(|id| scope.path(*id).map(|path| format!("assets/{path}")))
            .collect();
        paths.extend(
            scope
                .source
                .environments
                .keys()
                .filter(|path| self.environments.dirty(path))
                .map(|path| format!("assets/{path}")),
        );
        paths.sort();
        paths
    }
    pub(crate) fn can_save_all(&self) -> bool {
        !self.save_busy() && (self.save_pending() || !self.unsaved_paths().is_empty())
    }
    pub(crate) fn begin_save_all(&mut self, world: &World, root: PathBuf) -> Result<(), String> {
        if self.save_busy() {
            return Err("Save is already in progress.".into());
        }
        self.save.error = None;
        self.save.all = true;
        self.advance_save_all(world, root)
    }
    pub(crate) fn advance_save_all(&mut self, world: &World, root: PathBuf) -> Result<(), String> {
        if !self.save.all || self.save.task.is_some() {
            return Ok(());
        }
        self.save.all = false;
        if self.save.error.is_some() {
            return Ok(());
        }
        let Some(scope) = &self.scope else {
            return Ok(());
        };
        // Finish pinned publication before beginning another source group. Groups
        // are acknowledged individually; a later refusal never rolls back a saved group.
        let pending_document = scope.pending().is_some();
        let document = scope
            .working
            .documents()
            .keys()
            .find(|id| scope.working.dirty(**id))
            .copied();
        let environment = self
            .environments
            .pending()
            .map(|r| r.files[0].path.clone())
            .or_else(|| {
                scope
                    .source
                    .environments
                    .keys()
                    .find(|p| self.environments.dirty(p))
                    .cloned()
            });
        let result = if pending_document {
            self.begin_save_for(world, root, document, false)
        } else if let Some(path) = environment {
            self.begin_environment_save(world, root, path)
        } else if let Some(id) = document {
            self.begin_save_for(world, root, Some(id), false)
        } else {
            return Ok(());
        };
        match result {
            Ok(()) => {
                self.save.all = true;
                Ok(())
            }
            Err(error) => {
                self.save.error = Some(error.clone());
                Err(error)
            }
        }
    }
    fn save_target(&self) -> Option<DocumentId> {
        if matches!(
            self.selection.active(),
            Some(kedra_editor_context::SelectionItem::Asset(_))
        ) {
            self.selection_target()
                .map(|target| target.document)
                .or(self.active)
        } else {
            self.active
        }
    }
    pub(super) fn begin_save_for(
        &mut self,
        world: &World,
        root: PathBuf,
        selected: Option<DocumentId>,
        creation: bool,
    ) -> Result<(), String> {
        if self.save_busy() {
            return Err("Save is already in progress.".into());
        }
        if (!creation && (self.opening() || self.creating())) || self.task.is_some() {
            return Err("Wait for source admission before Save.".into());
        }
        let scope = self
            .scope
            .as_mut()
            .ok_or("No admitted source workspace is open.")?;
        let retry = scope.pending().is_some();
        let request = if let Some(request) = scope.pending() {
            request.clone()
        } else {
            let selected = selected.ok_or("Activate a source document before Save.")?;
            if !scope
                .working
                .save_group(selected)
                .iter()
                .any(|id| scope.working.dirty(*id))
            {
                return Err("The active document group has no unsaved changes.".into());
            }
            let registry = world.resource::<AppTypeRegistry>().clone();
            source::prepare_document_save(
                scope,
                selected,
                &registry.read(),
                world.resource::<ComponentPolicies>(),
            )?
        };
        let baseline = scope.source.clone();
        self.save.error = None;
        self.save.environment = false;
        self.save.task = Some(
            Wakeup::for_world(world).spawn(IoTaskPool::get(), async move {
                let result = verify(&root, &baseline, &request)
                    .and_then(|()| kedra_authoring_fs::commit_documents(&root, &request));
                result.map_err(|mut error| {
                    error.uncertain |= retry;
                    error
                })
            }),
        );
        Ok(())
    }
    pub(crate) fn poll_save(&mut self) -> Option<String> {
        let result = self.save.task.as_mut().and_then(Task::poll)?;
        self.save.task = None;
        let scope = self.scope.as_mut()?;
        if self.save.environment {
            let result = result.and_then(|receipt| {
                self.environments
                    .acknowledge(&mut scope.source, &receipt)
                    .map_err(|message| SaveError {
                        uncertain: true,
                        message,
                    })
            });
            return Some(match result {
                Ok(()) => "Saved environment source.".into(),
                Err(error) => {
                    if !error.uncertain {
                        self.environments.refused();
                    }
                    let message = format!("Environment Save failed: {}", error.message);
                    self.save.error = Some(message.clone());
                    message
                }
            });
        }
        let result = match result {
            Ok(receipt) => scope
                .acknowledge(&receipt)
                .map(|()| format!("Saved {} source document(s). Newer accepted edits, if any, remain unsaved.", receipt.files.len()))
                .map_err(|error| SaveError {
                    uncertain: true,
                    message: error.to_string(),
                }),
            Err(error) => Err(error),
        };
        Some(match result {
            Ok(message) => message,
            Err(error) => {
                if !error.uncertain
                    && let Some(operation) = scope.pending().map(|request| request.operation)
                {
                    let _ = scope.refused(operation);
                }
                let message = if error.uncertain {
                    format!(
                        "Save outcome is unresolved: {}. Use Retry Save; discard and close remain blocked.",
                        error.message
                    )
                } else {
                    format!(
                        "Save refused: {}. Working changes were kept.",
                        error.message
                    )
                };
                self.save.error = Some(message.clone());
                message
            }
        })
    }
}
fn verify(
    root: &std::path::Path,
    baseline: &SourceSnapshot,
    request: &CommitRequest,
) -> Result<(), SaveError> {
    let mut observed =
        source_files::read_for_save(root, "game", Some(request)).map_err(|message| SaveError {
            uncertain: false,
            message,
        })?;
    // A retry may observe the exact already-published environment bytes. Keep
    // identity checks intact while comparing its pinned pre-publication inventory.
    for file in request.files.iter().filter(|file| file.document.is_none()) {
        if let Some(asset) = observed
            .assets
            .iter_mut()
            .find(|asset| asset.path == file.path)
            && let Some(text) = &file.source
            && asset.hash == kedra_authoring::persistence::content_hash(text)
            && let Some(old) = baseline
                .assets
                .iter()
                .find(|old| old.path == file.path && old.id == asset.id)
        {
            asset.hash.clone_from(&old.hash);
            asset.bytes = old.bytes;
        }
    }
    let assets = |source: &SourceSnapshot| {
        source
            .assets
            .iter()
            .map(|asset| (asset.path.clone(), asset.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    if assets(baseline) != assets(&observed) {
        return Err(SaveError {
            uncertain: false,
            message: "Asset inventory or sidecar identity changed since admission.".into(),
        });
    }
    // The writer independently rechecks all exact hashes and source membership.
    // This pass additionally reuses admission's source/sidecar identity policy.
    let mut expected: std::collections::BTreeSet<_> = baseline
        .documents
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    for file in &request.files {
        if file.expected_hash.is_none()
            && observed.documents.iter().any(|doc| doc.path == file.path)
        {
            expected.insert(&file.path);
        }
        if file.source.is_none() && !observed.documents.iter().any(|doc| doc.path == file.path) {
            expected.remove(file.path.as_str());
        }
    }
    if observed
        .documents
        .iter()
        .map(|file| file.path.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        != expected
    {
        return Err(SaveError {
            uncertain: false,
            message: "Source inventory changed since admission.".into(),
        });
    }
    Ok(())
}
