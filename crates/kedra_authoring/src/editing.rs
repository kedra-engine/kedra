//! Typed native source leases shared by tools and language adapters.
use crate::{
    persistence::Workspace,
    source,
    transaction::EditTransaction,
    workspace::{HistoryId, WorkingSet},
};
use bevy::{
    ecs::reflect::ReflectComponent,
    reflect::{ParsedPath, ReflectPath, TypeRegistry, serde::TypedReflectDeserializer},
};
use kedra_scene::{document::DocumentId, reflection::ComponentPolicies};
use serde::de::{DeserializeSeed, IntoDeserializer};
use std::collections::BTreeMap;
use std::{collections::BTreeSet, sync::Arc};
mod structure;

/// Isolated typed edit over explicitly granted source paths. The working-set
/// lineage and complete revision inventory are checked on every operation.
/// No UI, filesystem or language runtime is involved; dropping never publishes.
pub struct SourceEdit {
    transaction: EditTransaction,
    paths: BTreeMap<String, DocumentId>,
    operations: usize,
}
/// One typed reflected property request. JSON is transport, not a source schema.
pub struct SourceProperty<'a> {
    /// Persistent receiving node from this edit's admitted documents.
    pub node: &'a SourceNode,
    /// Canonical component policy key.
    pub component: &'a str,
    /// Native Bevy field path.
    pub path: &'a str,
    /// Exact typed deserialization input, bounded to 64 KiB.
    pub value: serde_json::Value,
}
/// Persistent node description; never a UI entity or language-owned authority token.
#[derive(Clone)]
pub struct SourceNode {
    /// Document receiving the edit, not an implicit upstream prefab Apply.
    pub document: DocumentId,
    /// Full composed node address in the receiving document.
    pub address: kedra_scene::document::NodeAddress,
    /// Display-only name at enumeration time.
    pub name: String,
}
impl SourceEdit {
    /// Begin with 1..=128 exact admitted logical source grants and no pending Save.
    /// Admission never changes sources, revisions or history.
    pub fn begin(scope: &Workspace, label: &str, grants: &[String]) -> Result<Self, String> {
        if scope.pending().is_some() || grants.is_empty() || grants.len() > 128 {
            return Err("Source edit requires 1..=128 explicit grants and no pending Save".into());
        }
        let mut paths = BTreeMap::new();
        for path in grants {
            let id = scope
                .working
                .documents()
                .keys()
                .find(|id| {
                    scope.path(**id).is_some_and(|source| {
                        *path == format!("{}://{source}", scope.source.namespace)
                    })
                })
                .copied()
                .ok_or("Edit grant is not an exact admitted logical document path")?;
            if paths.insert(path.clone(), id).is_some() {
                return Err("Duplicate source edit grant".into());
            }
        }
        let allowed: BTreeSet<_> = paths.values().copied().collect();
        Ok(SourceEdit {
            transaction: EditTransaction::begin(&scope.working, label, allowed)
                .map_err(|error| error.to_string())?,
            paths,
            operations: 0,
        })
    }
}
impl SourceEdit {
    fn current<'a>(&self, scope: &'a Workspace) -> Result<&'a Workspace, String> {
        if scope.pending().is_some() {
            return Err("Resolve the pending Save before source edits".into());
        }
        self.transaction
            .check(&scope.working)
            .map_err(|error| error.to_string())?;
        Ok(scope)
    }
    fn node(&self, node: &SourceNode) -> Result<(), String> {
        if !self.paths.values().any(|id| *id == node.document) {
            return Err("Source node is outside the edit grant".into());
        }
        if self.operations >= 128 {
            return Err("Source edit exceeds 128 staged operations".into());
        }
        Ok(())
    }
    fn compile(
        &self,
        documents: &Workspace,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<BTreeMap<DocumentId, Arc<kedra_scene::instance::CompiledPrefab>>, String> {
        let scope = self.current(documents)?;
        source::compile_catalog(
            &scope.source,
            self.transaction
                .candidate()
                .map_err(|error| error.to_string())?,
            types,
            policies,
        )
    }
    /// Enumerate composed nodes within one exact grant, without native asset loading.
    pub fn nodes(&self, documents: &Workspace, path: &str) -> Result<Vec<SourceNode>, String> {
        self.current(documents)?;
        let id = self
            .paths
            .get(path)
            .ok_or("Document path is outside the edit grant")?;
        // Enumeration needs only composed addresses/names, not fresh native Worlds
        // for every document. Staging and commit still perform typed compilation.
        let graph = self
            .transaction
            .candidate()
            .map_err(|error| error.to_string())?
            .resolve(*id)
            .map_err(|error| error.to_string())?;
        Ok(graph
            .nodes
            .iter()
            .map(|(address, node)| SourceNode {
                document: *id,
                address: address.clone(),
                name: node.name.clone(),
            })
            .collect())
    }
    /// Stage an exactly typed property, validating the entire catalog. Errors leave
    /// both the prior candidate and working sources unchanged.
    pub fn set_property(
        &mut self,
        documents: &Workspace,
        request: SourceProperty<'_>,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        let SourceProperty {
            node,
            component,
            path,
            value,
        } = request;
        self.node(node)?;
        let scope = self.current(documents)?;
        if path.len() > 1024
            || component.len() > 256
            || serde_json::to_vec(&value)
                .map_err(|error| error.to_string())?
                .len()
                > 65_536
        {
            return Err("Source property request exceeds its byte limit".into());
        }
        let compiled = self.compile(documents, types, policies)?;
        let snapshot = compiled
            .get(&node.document)
            .ok_or("Edit document unavailable")?
            .clone();
        let data = snapshot
            .graph()
            .nodes
            .get(&node.address)
            .and_then(|node| node.components.get(component))
            .ok_or("Edit component unavailable")?;
        let policy = policies
            .policy(component, data.data.version)
            .map_err(|error| error.to_string())?;
        let reflect = types
            .get_type_data::<ReflectComponent>(policy.type_id)
            .ok_or("Component has no reflected fields")?;
        let entity = snapshot
            .source_entities()
            .get(&node.address)
            .ok_or("Edit node unavailable")?;
        let component_value = reflect
            .reflect(snapshot.authoring_world().entity(*entity))
            .ok_or("Native edit component unavailable")?;
        let path = ParsedPath::parse(path).map_err(|error| error.to_string())?;
        let field = (&path)
            .reflect_element(component_value)
            .map_err(|error| error.to_string())?;
        let info = field
            .get_represented_type_info()
            .ok_or("Native edit field type unavailable")?;
        let registration = types
            .get(info.type_id())
            .ok_or("Native edit type unregistered")?;
        let value = TypedReflectDeserializer::new(registration, types)
            .deserialize(value.into_deserializer())
            .map_err(|error| error.to_string())?;
        let mut scratch = WorkingSet::default();
        for document in self
            .transaction
            .candidate()
            .map_err(|error| error.to_string())?
            .documents
            .values()
        {
            scratch
                .open(document.clone())
                .map_err(|error| error.to_string())?;
        }
        let mut property = crate::properties::PropertySession::begin(
            &scratch,
            snapshot,
            crate::properties::PropertyTarget {
                document: node.document,
                node: node.address.clone(),
                component: component.into(),
            },
            types,
            policies,
        )
        .map_err(|error| error.to_string())?;
        let validate = |catalog: &kedra_scene::composition::Catalog| {
            source::compile_catalog(&scope.source, catalog, types, policies)
                .map(|_| ())
                .map_err(kedra_scene::Error)
        };
        property
            .propose(&scratch, path, value.as_ref(), types, policies, validate)
            .map_err(|error| error.to_string())?;
        let candidate = property
            .candidate(&scratch, validate)
            .map_err(|error| error.to_string())?;
        self.transaction
            .stage(candidate)
            .map_err(|error| error.to_string())?;
        self.operations += 1;
        Ok(())
    }
    /// Stage a valid source-only rename; rejection retains the previous candidate.
    pub fn rename(
        &mut self,
        documents: &Workspace,
        node: &SourceNode,
        name: &str,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        self.node(node)?;
        let scope = self.current(documents)?;
        if name.trim().is_empty() || name.len() > 1024 {
            return Err("Source node name must contain 1..=1024 bytes".into());
        }
        self.transaction
            .rename(
                node.document,
                node.address.clone(),
                name.into(),
                |candidate| {
                    source::compile_catalog(&scope.source, candidate, types, policies)
                        .map(|_| ())
                        .map_err(kedra_scene::Error)
                },
            )
            .map_err(|error| error.to_string())?;
        self.operations += 1;
        Ok(())
    }
    /// Validate and publish one named source operation. Success closes this lease;
    /// failure retains the candidate and source/history. Never writes files or Saves.
    pub fn commit(
        &mut self,
        documents: &mut Workspace,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<EditOutcome, String> {
        let scope = self.current(documents)?;
        let mut working = scope.working.clone();
        let previous = working.undo_entry().map(|entry| entry.id);
        let mut compiled = None;
        self.transaction
            .commit(&mut working, |catalog| {
                compiled = Some(
                    source::compile_catalog(&scope.source, catalog, types, policies)
                        .map_err(kedra_scene::Error)?,
                );
                Ok(())
            })
            .map_err(|error| error.to_string())?;
        let id = working
            .undo_entry()
            .map(|entry| entry.id)
            .filter(|id| Some(*id) != previous);
        let compiled = compiled.ok_or("Edit transaction compilation unavailable")?;
        documents.working = working;
        Ok(EditOutcome {
            history: id,
            compiled,
        })
    }
    /// Discard this candidate. Successful earlier commits are never rolled back.
    pub fn cancel(&mut self) {
        self.transaction.cancel();
    }
}
/// Exact compiled result of a successful synchronous source publication.
pub struct EditOutcome {
    /// Accepted history identity, or None for a validated no-op.
    pub history: Option<HistoryId>,
    /// Immutable compilations matching the newly published working sources.
    pub compiled: source::CompiledSources,
}
