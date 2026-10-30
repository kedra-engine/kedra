//! Folder publication installs a prevalidated source workspace by persistent identity.
use super::*;
impl Documents {
    pub(crate) fn folder_scope(&self) -> Option<Workspace> {
        self.scope.clone()
    }
    pub(crate) fn install_folder(&mut self, (scope, compiled): source::Admission) -> Vec<ViewId> {
        self.environments = default();
        let removed = self
            .opened
            .iter()
            .filter(|(id, _)| !compiled.contains_key(id))
            .map(|(_, opened)| opened.view)
            .collect();
        self.opened.retain(|id, _| compiled.contains_key(id));
        for (id, opened) in &mut self.opened {
            opened.graph = std::sync::Arc::new(compiled[id].graph().clone());
            #[cfg(debug_assertions)]
            if let Some(path) = scope.path(*id) {
                opened.path = path.into();
            }
        }
        if self.active.is_some_and(|id| !compiled.contains_key(&id)) {
            self.active = None;
        }
        self.paths = scope
            .working
            .documents()
            .keys()
            .filter_map(|id| scope.path(*id).map(|p| (p.into(), *id)))
            .collect();
        self.scope = Some(scope);
        self.compiled = compiled;
        self.render_source = None;
        self.compiled_revisions = self.revisions();
        self.reconcile_selection();
        removed
    }
}
