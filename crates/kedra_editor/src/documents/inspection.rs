//! Editor lifetime authorization around shared immutable source inspection.
use super::Documents;
use bevy::reflect::TypeRegistry;
use kedra_authoring::inspection::Inspection;
use kedra_scene::{
    document::{DocumentId, NodeAddress},
    reflection::ComponentPolicies,
};

use kedra_editor_context::ScopeId;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    scope: ScopeId,
    pub document: DocumentId,
    pub node: NodeAddress,
}
impl Documents {
    pub(crate) fn refresh_inspection(
        &mut self,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        let scope = self.scope.as_ref().ok_or("No admitted source workspace")?;
        let revisions = scope
            .working
            .documents()
            .iter()
            .map(|(id, doc)| (*id, doc.revision))
            .collect();
        if self.compiled_revisions != revisions {
            self.compiled = kedra_authoring::source::compile_working(scope, types, policies)?;
            self.compiled_revisions = revisions;
        }
        Ok(())
    }
    pub(crate) fn target(&self, document: DocumentId, node: NodeAddress) -> Option<Target> {
        self.compiled.get(&document)?.graph().nodes.get(&node)?;
        Some(Target {
            scope: self.scope_identity,
            document,
            node,
        })
    }
    pub(crate) fn inspect(&self, target: &Target) -> Result<Inspection<'_>, String> {
        if target.scope != self.scope_identity {
            return Err("Inspector source scope was retired".into());
        }
        Inspection::new(
            self.scope.as_ref().ok_or("No admitted source workspace")?,
            self.compiled
                .get(&target.document)
                .ok_or("Source compilation is unavailable")?,
            &target.node,
            &self.compiled_revisions,
        )
    }
}
#[cfg(test)]
#[path = "inspection_tests.rs"]
mod tests;
