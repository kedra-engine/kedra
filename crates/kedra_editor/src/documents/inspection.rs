//! Immutable source-bound inspection, independent of viewport entities or frames.
use super::Documents;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScopeIdentity(uuid::Uuid);
impl Default for ScopeIdentity {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    scope: ScopeIdentity,
    pub document: DocumentId,
    pub node: NodeAddress,
}

pub(crate) struct Inspection<'a> {
    compiled: &'a CompiledPrefab,
    scope: &'a kedra_authoring::persistence::Workspace,
    pub components: &'a BTreeMap<String, ResolvedComponent>,
    pub source_path: &'a str,
    pub name: &'a str,
    node: NodeAddress,
}
impl Documents {
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
        let scope = self.scope.as_ref().ok_or("No admitted source workspace")?;
        let compiled = self
            .compiled
            .get(&target.document)
            .ok_or("Source compilation is unavailable")?;
        if compiled.graph().dependencies.iter().any(|id| {
            !scope
                .working
                .documents()
                .get(id)
                .is_some_and(|d| d.revision == 1)
        }) {
            return Err(
                "Source changed after read-only admission; inspection snapshot is stale".into(),
            );
        }
        let node = compiled
            .graph()
            .nodes
            .get(&target.node)
            .ok_or("Inspector target is unavailable")?;
        Ok(Inspection {
            compiled,
            scope,
            components: &node.components,
            source_path: scope
                .path(target.document)
                .ok_or("Document source path is unavailable")?,
            name: &node.name,
            node: target.node.clone(),
        })
    }
}
impl Inspection<'_> {
    pub(crate) fn metadata(&self, key: &str) -> Result<SourceMetadata<'_>, String> {
        Ok(SourceMetadata {
            component: self
                .components
                .get(key)
                .ok_or("Component provenance is unavailable")?,
            scope: self.scope,
            document: self.compiled.graph().document,
            nested: !self.node.instances.is_empty(),
        })
    }
    pub(crate) fn value(
        &self,
        key: &str,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<&dyn PartialReflect, String> {
        let component = self
            .components
            .get(key)
            .ok_or("Source component is unavailable")?;
        let policy = policies
            .policy(key, component.data.version)
            .map_err(|e| format!("Missing source capability: {e}"))?;
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
            .map(|v| v.as_partial_reflect())
            .ok_or("Reflected source value is unavailable".into())
    }
}

pub(crate) struct SourceMetadata<'a> {
    component: &'a ResolvedComponent,
    scope: &'a kedra_authoring::persistence::Workspace,
    document: DocumentId,
    nested: bool,
}
impl kedra_inspector::binding::ValueMetadata for SourceMetadata<'_> {
    fn describe(&self, path: Option<&bevy::reflect::ParsedPath>) -> Option<String> {
        let mut origin = &self.component.origin;
        if let Some(path) = path {
            let accesses: Vec<_> = path.0.iter().map(|a| &a.access).collect();
            for field in &self.component.fields {
                let field_path = bevy::reflect::ParsedPath::parse(&field.path).ok()?;
                let field_access: Vec<_> = field_path.0.iter().map(|a| &a.access).collect();
                if accesses.starts_with(&field_access) {
                    origin = &field.origin;
                } else if field_access.starts_with(&accesses) {
                    return Some("Mixed source provenance; inspect child fields".into());
                }
            }
        }
        let kind = if origin.document != self.document {
            "Inherited prefab"
        } else if self.nested {
            "Instance override"
        } else if self.component.origin.document != self.document {
            "Variant override"
        } else {
            "Local source"
        };
        Some(format!(
            "{kind}: {}",
            self.scope
                .path(origin.document)
                .unwrap_or("Source path unavailable")
        ))
    }
    fn limitation(
        &self,
        value: &dyn PartialReflect,
        _: Option<&bevy::reflect::ParsedPath>,
    ) -> Option<String> {
        if value.try_downcast_ref::<bevy::prelude::Entity>().is_some()
            || value
                .try_downcast_ref::<Option<bevy::prelude::Entity>>()
                .is_some()
        {
            Some("Symbolic source reference needs a source-reference presentation; effective optional presence is not authored intent".into())
        } else {
            None
        }
    }
}
#[cfg(test)]
#[path = "inspection_tests.rs"]
mod tests;
