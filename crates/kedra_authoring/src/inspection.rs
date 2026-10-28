//! Immutable revision-checked source inspection, without widgets or render frames.
use crate::persistence::Workspace;
use bevy::{
    ecs::reflect::ReflectComponent,
    reflect::{PartialReflect, TypeRegistry},
};
use kedra_scene::{
    composition::ResolvedComponent,
    document::{DocumentId, NodeAddress},
    instance::CompiledPrefab,
    reflection::ComponentPolicies,
};
use std::collections::BTreeMap;
/// Borrowed source values and provenance for one exact compiled node.
pub struct Inspection<'a> {
    compiled: &'a CompiledPrefab,
    scope: &'a Workspace,
    /// Effective components and their source provenance.
    pub components: &'a BTreeMap<String, ResolvedComponent>,
    /// Project-relative authored document path, not a native filesystem locator.
    pub source_path: &'a str,
    /// Effective node name.
    pub name: &'a str,
    /// Complete host compilation revision inventory.
    pub revisions: &'a BTreeMap<DocumentId, u64>,
    node: NodeAddress,
}
impl<'a> Inspection<'a> {
    /// Bind a node only if the compiled target and all composition dependencies
    /// still match the working revision inventory. Failure publishes nothing.
    pub fn new(
        scope: &'a Workspace,
        compiled: &'a CompiledPrefab,
        node: &NodeAddress,
        revisions: &'a BTreeMap<DocumentId, u64>,
    ) -> Result<Self, String> {
        let document = compiled.graph().document;
        if compiled
            .graph()
            .dependencies
            .iter()
            .chain(std::iter::once(&document))
            .any(|id| {
                !scope
                    .working
                    .documents()
                    .get(id)
                    .is_some_and(|doc| revisions.get(id) == Some(&doc.revision))
            })
        {
            return Err("Source compilation is not current for the working revision".into());
        }
        let data = compiled
            .graph()
            .nodes
            .get(node)
            .ok_or("Source target is unavailable")?;
        Ok(Self {
            compiled,
            scope,
            components: &data.components,
            source_path: scope
                .path(document)
                .ok_or("Document source path is unavailable")?,
            name: &data.name,
            revisions,
            node: node.clone(),
        })
    }
    /// Exact logical native dependency path recorded by the source compiler.
    pub fn dependency_path(&self, handle: &bevy::asset::UntypedHandle) -> Option<String> {
        self.compiled
            .dependency_path(handle)
            .map(ToString::to_string)
    }
    /// Source-only metadata; presentation adapters choose labels/icons separately.
    pub fn metadata(&self, key: &str) -> Result<Metadata<'a>, String> {
        Ok(Metadata {
            component: self
                .components
                .get(key)
                .ok_or("Component provenance is unavailable")?,
            scope: self.scope,
            document: self.compiled.graph().document,
            nested: !self.node.instances.is_empty(),
            compiled: self.compiled,
        })
    }
    /// Borrow an exact native reflected component from the isolated source World.
    /// Missing policies/registration/components refuse without loading native assets.
    pub fn value(
        &self,
        key: &str,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<&'a dyn PartialReflect, String> {
        let component = self
            .components
            .get(key)
            .ok_or("Source component is unavailable")?;
        let policy = policies
            .policy(key, component.data.version)
            .map_err(|error| format!("Missing source capability: {error}"))?;
        let reflect = types
            .get_type_data::<ReflectComponent>(policy.type_id)
            .ok_or("Component registration lacks reflected ECS access")?;
        let entity = self
            .compiled
            .source_entities()
            .get(&self.node)
            .ok_or("Compiled source node is unavailable")?;
        reflect
            .reflect(self.compiled.authoring_world().entity(*entity))
            .map(|value| value.as_partial_reflect())
            .ok_or("Reflected source value is unavailable".into())
    }
}
/// Borrowed provenance data, without Inspector interfaces or delivery UI state.
pub struct Metadata<'a> {
    /// Effective component and field origins.
    pub component: &'a ResolvedComponent,
    /// Admitted source path/baseline authority.
    pub scope: &'a Workspace,
    /// Receiving source document.
    pub document: DocumentId,
    /// Whether the receiving address is inside a nested prefab scope.
    pub nested: bool,
    /// Immutable source compilation recording symbolic dependency paths.
    pub compiled: &'a CompiledPrefab,
}
