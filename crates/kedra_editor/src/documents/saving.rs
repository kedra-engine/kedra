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
    pub error: Option<String>,
}
impl Documents {
    pub(crate) fn source_titles(&self) -> Vec<(ViewId, String)> {
        let Some(scope) = &self.scope else {
            return Vec::new();
        };
        self.opened
            .values()
            .filter_map(|opened| {
                let document = scope.working.documents().get(&opened.document)?;
                Some((
                    opened.view,
                    format!(
                        "{}{}",
                        document.document.title,
                        if scope.working.dirty(opened.document) {
                            " *"
                        } else {
                            ""
                        }
                    ),
                ))
            })
            .collect()
    }
    pub(crate) fn save_busy(&self) -> bool {
        self.save.task.is_some()
    }
    pub(crate) fn save_pending(&self) -> bool {
        self.scope
            .as_ref()
            .is_some_and(|scope| scope.pending().is_some())
    }
    pub(crate) fn can_save(&self) -> bool {
        !self.save_busy()
            && self.scope.as_ref().is_some_and(|scope| {
                scope.pending().is_some()
                    || self.active.is_some_and(|id| {
                        scope
                            .working
                            .save_group(id)
                            .iter()
                            .any(|member| scope.working.dirty(*member))
                    })
            })
    }
    pub(crate) fn begin_save(&mut self, world: &World, root: PathBuf) -> Result<(), String> {
        if self.save_busy() {
            return Err("Save is already in progress.".into());
        }
        if self.opening() || self.task.is_some() {
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
            let selected = self
                .active
                .ok_or("Activate a source document before Save.")?;
            if !scope
                .working
                .save_group(selected)
                .iter()
                .any(|id| scope.working.dirty(*id))
            {
                return Err("The active document group has no unsaved changes.".into());
            }
            let registry = world.resource::<AppTypeRegistry>().clone();
            source::prepare_existing_save(
                scope,
                selected,
                &registry.read(),
                world.resource::<ComponentPolicies>(),
            )?
        };
        let baseline = scope.source.clone();
        self.save.error = None;
        self.save.task = Some(
            Wakeup::for_world(world).spawn(IoTaskPool::get(), async move {
                let result = verify(&root, &baseline, &request)
                    .and_then(|()| kedra_authoring_fs::commit_existing(&root, &request));
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
    let observed =
        source_files::read_for_save(root, "game", Some(request)).map_err(|message| SaveError {
            uncertain: false,
            message,
        })?;
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
    if observed.documents.len() != baseline.documents.len() {
        return Err(SaveError {
            uncertain: false,
            message: "Source inventory changed since admission.".into(),
        });
    }
    Ok(())
}
