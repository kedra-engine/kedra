//! Project-relative creation publishes through Save before admitting a document tab.
use super::*;
use kedra_scene::document::{DocumentKind, PrefabDocument};

type Selection =
    Result<Option<(DocumentKind, PathBuf, PathBuf, Option<source::Admission>)>, String>;
#[derive(Default)]
pub(super) struct Creation {
    task: Option<Task<Selection>>,
    publication: Option<(PathBuf, PathBuf)>,
    failed: bool,
}
impl Documents {
    pub(crate) fn creating(&self) -> bool {
        self.creation.task.is_some()
            || (self.creation.publication.is_some() && !self.creation.failed)
    }
    pub(crate) fn begin_document(
        &mut self,
        world: &World,
        root: PathBuf,
        path: PathBuf,
        kind: DocumentKind,
    ) -> Result<(), String> {
        if self.creation.failed
            && let Some((previous_root, previous_path)) = &self.creation.publication
        {
            if previous_root != &root || previous_path != &path {
                return Err(
                    "Retry the original document creation before choosing another path.".into(),
                );
            }
            let selected = self.paths.get(&path).copied();
            self.begin_save_for(world, root, selected, true)?;
            self.creation.failed = false;
            self.error = None;
            return Ok(());
        }
        if self.creating()
            || self.opening()
            || self.task.is_some()
            || self.save_pending()
            || self.save_busy()
        {
            return Err(
                "Wait for the current document operation before creating a document.".into(),
            );
        }
        if path.is_absolute()
            || path
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err("Choose a document path inside this project's assets directory.".into());
        }
        let admit = self.scope.is_none();
        let registry = world.resource::<AppTypeRegistry>().clone();
        let policies = world.resource::<ComponentPolicies>().clone();
        self.creation.task = Some(
            Wakeup::for_world(world).spawn(IoTaskPool::get(), async move {
                let root = std::fs::canonicalize(root).map_err(|error| error.to_string())?;
                let parent = root.join(path.parent().ok_or("Document parent missing")?);
                if std::fs::canonicalize(&parent).map_err(|error| error.to_string())? != parent {
                    return Err(
                        "Document parent must be an existing source directory without links."
                            .into(),
                    );
                }
                // Creation never replaces an existing source.
                match std::fs::symlink_metadata(root.join(&path)) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                    Err(error) => return Err(error.to_string()),
                    Ok(_) => {
                        return Err(
                            "The document path already exists. Choose a new filename.".into()
                        );
                    }
                }
                let admission = if admit {
                    Some(source::admit(
                        source_files::read(&root, "game")?,
                        &registry.read(),
                        &policies,
                    )?)
                } else {
                    None
                };
                Ok(Some((kind, root, path, admission)))
            }),
        );
        Ok(())
    }
    pub(crate) fn poll_creation(&mut self, world: &World) {
        if self.creation.publication.is_some() {
            if self.creation.failed || self.save_busy() {
                return;
            }
            if let Some(error) = self.save.error.clone() {
                self.creation.failed = true;
                self.error = Some(error);
            } else if let Some((_, path)) = self.creation.publication.take() {
                self.pending = Some(path.clone());
                self.checked = Some(path);
            }
            return;
        }
        let Some(result) = self.creation.task.as_mut().and_then(Task::poll) else {
            return;
        };
        self.creation.task = None;
        let result = result.and_then(|selected| {
            let Some((kind, root, path, admission)) = selected else {
                return Ok(());
            };
            let registry = world.resource::<AppTypeRegistry>().clone();
            self.create_document(
                kind,
                path.clone(),
                admission,
                &registry.read(),
                world.resource::<ComponentPolicies>(),
            )?;
            self.pending = None;
            self.checked = None;
            self.creation.publication = Some((root.clone(), path.clone()));
            let result = self.begin_save_for(world, root, self.paths.get(&path).copied(), true);
            if result.is_err() {
                self.creation.failed = true;
            }
            result
        });
        if let Err(error) = result {
            self.error = Some(error);
        }
    }
    fn create_document(
        &mut self,
        kind: DocumentKind,
        path: PathBuf,
        admission: Option<source::Admission>,
        types: &bevy::reflect::TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        let mut scope = self
            .scope
            .clone()
            .or_else(|| admission.map(|(scope, _)| scope))
            .ok_or("Source workspace unavailable")?;
        if scope.pending().is_some() {
            return Err("Resolve pending Save before creating a document.".into());
        }
        let portable = path
            .to_str()
            .ok_or("Document path must be Unicode")?
            .replace(std::path::MAIN_SEPARATOR, "/");
        let suffix = match kind {
            DocumentKind::Scene => ".scene.ron",
            DocumentKind::Prefab => ".prefab.ron",
        };
        let title = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(suffix))
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| format!("Use a nonempty document name ending in {suffix}"))?;
        let mut document = PrefabDocument::new(title);
        document.kind = kind;
        let id = document.id;
        let baseline = scope.source.clone();
        let compiled = source::create_document(&mut scope, document, portable, types, policies)?;
        if self.render_source.is_none() {
            self.render_source = Some(baseline);
        }
        self.scope = Some(scope);
        self.compiled = compiled;
        self.compiled_revisions = self.revisions();
        self.paths.insert(path.clone(), id);
        // Initial admission must also retain paths for the project's existing documents.
        if let Some(scope) = &self.scope {
            for id in scope.working.documents().keys() {
                if let Some(path) = scope.path(*id) {
                    self.paths.insert(path.into(), *id);
                }
            }
        }
        self.pending = Some(path.clone());
        self.checked = Some(path);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
