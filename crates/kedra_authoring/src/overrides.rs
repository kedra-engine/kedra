//! Revision-guarded override Apply/Revert; validation precedes every source publication.
use crate::{failure, workspace::WorkingSet};
use bevy::reflect::{ReflectPath, TypeRegistry};
use kedra_scene::{
    Result,
    composition::Catalog,
    document::*,
    instance::CompiledPrefab,
    reflection::{ComponentPolicies, ReferenceContext},
};
use std::collections::BTreeMap;

/// Explicit source/destination and revision preconditions for Apply operations.
pub struct ApplyField {
    /// Variant/instance source containing the override.
    pub origin: DocumentId,
    /// Base/prefab source to receive the effective value.
    pub target: DocumentId,
    /// Effective scoped source node.
    pub node: NodeAddress,
    /// Canonical component schema key.
    pub component: String,
    /// Field path, empty only for a whole-component operation.
    pub path: String,
    /// Every existing document the transaction may modify and its expected revision.
    pub expected: BTreeMap<DocumentId, u64>,
}
impl WorkingSet {
    /// Remove a component's originating overrides atomically; absent overrides are errors.
    /// The complete address selects top-level or nested instance-site override owners,
    /// including instances introduced by AddNestedInstance operations.
    pub fn revert_component(
        &mut self,
        id: DocumentId,
        revision: u64,
        node: &NodeAddress,
        component: &str,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        self.transact_checked(
            &BTreeMap::from([(id, revision)]),
            |documents| {
                let document = documents
                    .get_mut(&id)
                    .ok_or_else(|| failure("document missing"))?;
                if remove_component(document, node, component, true) == 0 {
                    return Err(failure("no component override"));
                }
                Ok(())
            },
            validate,
        )
    }
    /// Apply an effective component to its actual source and remove its originating override.
    /// Reference/representation changes or a shadowed destination reject both edits.
    /// The snapshot must match the current origin and all transitive composition sources.
    /// Scoped addresses also select source-owned nested instance override vectors.
    pub fn apply_component(
        &mut self,
        request: ApplyField,
        snapshot: &CompiledPrefab,
        policies: &ComponentPolicies,
        types: &TypeRegistry,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        if request.origin == request.target || snapshot.graph().document != request.origin {
            return Err(failure("invalid Apply source/destination"));
        }
        self.check_snapshot(snapshot)?;
        let symbolic = snapshot.symbolic_source(types, policies)?;
        let snapshot = &symbolic;
        let catalog = self.catalog();
        let scope = snapshot
            .graph()
            .frames
            .iter()
            .filter(|(scope, asset)| {
                request.node.instances.starts_with(scope)
                    && catalog.inherits(**asset, request.target)
            })
            .max_by_key(|(scope, _)| scope.len())
            .map(|(scope, _)| scope.clone())
            .ok_or_else(|| failure("target is not a source of this instance"))?;
        let version = snapshot
            .graph()
            .nodes
            .get(&request.node)
            .and_then(|node| node.components.get(&request.component))
            .ok_or_else(|| failure("component missing"))?
            .data
            .version;
        let data = ComponentData {
            version,
            value: snapshot.field_value(
                &request.node,
                &request.component,
                "",
                &scope,
                policies,
                types,
            )?,
        };
        let original = snapshot.component(
            &request.node,
            &request.component,
            policies,
            types,
            &BTreeMap::new(),
        )?;
        let target_node = NodeAddress {
            instances: request.node.instances[scope.len()..].to_vec(),
            node: request.node.node,
        };
        self.transact_checked(
            &request.expected,
            |documents| {
                let origin = documents
                    .get_mut(&request.origin)
                    .ok_or_else(|| failure("origin missing"))?;
                if remove_component(origin, &request.node, &request.component, false) == 0 {
                    return Err(failure("no originating component override"));
                }
                let target = documents
                    .get_mut(&request.target)
                    .ok_or_else(|| failure("target missing"))?;
                upsert(
                    &mut target.overrides,
                    Operation::SetComponent {
                        node: target_node,
                        component: request.component.clone(),
                        data,
                    },
                );
                Ok(())
            },
            |catalog| {
                validate(catalog)?;
                let next = snapshot.recompile(catalog.resolve(request.origin)?, types, policies)?;
                if next.component(
                    &request.node,
                    &request.component,
                    policies,
                    types,
                    &BTreeMap::new(),
                )? != original
                {
                    return Err(failure(
                        "Apply destination is shadowed or cannot represent references",
                    ));
                }
                Ok(())
            },
        )
    }
    /// Upsert one source operation under an exact document revision and validation contract.
    pub fn patch(
        &mut self,
        id: DocumentId,
        revision: u64,
        operation: Operation,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        self.transact_checked(
            &BTreeMap::from([(id, revision)]),
            |documents| {
                let document = documents
                    .get_mut(&id)
                    .ok_or_else(|| failure("document is not open"))?;
                upsert(&mut document.overrides, operation);
                Ok(())
            },
            validate,
        )
    }
    /// Revert one existing field override without changing inherited source values.
    pub fn revert_field(
        &mut self,
        id: DocumentId,
        revision: u64,
        node: &NodeAddress,
        component: &str,
        path: &str,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        self.transact_checked(
            &BTreeMap::from([(id, revision)]),
            |documents| {
                let document = documents
                    .get_mut(&id)
                    .ok_or_else(|| failure("document missing"))?;
                if remove_field(document, node, component, path) == 0 {
                    return Err(failure("no field override to revert"));
                }
                Ok(())
            },
            validate,
        )
    }
    /// Apply one effective field to its source while preserving symbolic references and value.
    /// Stale origin/dependency snapshots reject without publishing edits or consuming history.
    pub fn apply_field(
        &mut self,
        request: ApplyField,
        snapshot: &CompiledPrefab,
        policies: &ComponentPolicies,
        types: &TypeRegistry,
        loader: &mut dyn bevy::asset::LoadFromPath,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<()> {
        if request.origin == request.target || snapshot.graph().document != request.origin {
            return Err(failure("invalid Apply source/destination"));
        }
        self.check_snapshot(snapshot)?;
        let symbolic = snapshot.symbolic_source(types, policies)?;
        let snapshot = &symbolic;
        let catalog = self.catalog();
        let scope = snapshot
            .graph()
            .frames
            .iter()
            .filter(|(scope, asset)| {
                request.node.instances.starts_with(scope)
                    && catalog.inherits(**asset, request.target)
            })
            .max_by_key(|(scope, _)| scope.len())
            .map(|(scope, _)| scope.clone())
            .ok_or_else(|| failure("target is not a source of this instance"))?;
        let component = snapshot
            .graph()
            .nodes
            .get(&request.node)
            .and_then(|n| n.components.get(&request.component))
            .ok_or_else(|| failure("component missing"))?;
        for patch in &component.fields {
            let overlaps = patch.path == request.path
                || patch.path.starts_with(&format!("{}.", request.path))
                || request.path.starts_with(&format!("{}.", patch.path));
            if overlaps
                && patch.origin.document != request.origin
                && (patch.origin.scope.len() < scope.len()
                    || (patch.origin.scope.len() == scope.len()
                        && !catalog.inherits(request.target, patch.origin.document)))
            {
                return Err(failure(
                    "Apply destination is shadowed by an intervening override",
                ));
            }
        }
        let value = snapshot.field_value(
            &request.node,
            &request.component,
            &request.path,
            &scope,
            policies,
            types,
        )?;
        let destination = NodeAddress {
            instances: request.node.instances[scope.len()..].to_vec(),
            node: request.node.node,
        };
        let existing = catalog.documents.get(&request.target).and_then(|document| {
            document
                .overrides
                .iter()
                .find_map(|operation| match operation {
                    Operation::SetComponent {
                        node,
                        component,
                        data,
                    } if *node == destination && *component == request.component => Some(data),
                    _ => None,
                })
        });
        let target_operation = if let Some(data) = existing {
            let (mut component, references) = snapshot.authored_value(
                &request.component,
                data,
                &scope,
                policies,
                types,
                loader,
            )?;
            let field = request
                .path
                .as_str()
                .reflect_element_mut(component.as_mut())
                .map_err(|error| failure(error.to_string()))?;
            let info = field
                .get_represented_type_info()
                .ok_or_else(|| failure("field type missing"))?;
            let registration = types
                .get(info.type_id())
                .ok_or_else(|| failure("field type unregistered"))?;
            let mut entities = BTreeMap::new();
            let mut ports = BTreeMap::new();
            for (entity, reference) in &references {
                match reference {
                    Reference::Local(address) => {
                        entities.insert(address.clone(), *entity);
                    }
                    Reference::Port(name) => {
                        ports.insert(name.clone(), Some(*entity));
                    }
                }
            }
            let mut context = ReferenceContext {
                entities: &entities,
                ports: &ports,
                prefix: &[],
                loader,
            };
            let mut de = ron::Deserializer::from_str(value.get_ron())?;
            use serde::de::DeserializeSeed;
            let patch = bevy::reflect::serde::TypedReflectDeserializer::with_processor(
                registration,
                types,
                &mut context,
            )
            .deserialize(&mut de)?;
            de.end()?;
            field
                .try_apply(patch.as_ref())
                .map_err(|error| failure(error.to_string()))?;
            let data =
                policies.encode(&request.component, component.as_ref(), types, &references)?;
            Operation::SetComponent {
                node: destination,
                component: request.component.clone(),
                data,
            }
        } else {
            Operation::SetField {
                node: destination,
                component: request.component.clone(),
                version: component.data.version,
                path: request.path.clone(),
                value,
            }
        };
        let original_value = snapshot.field_value(
            &request.node,
            &request.component,
            &request.path,
            &[],
            policies,
            types,
        )?;
        let (origin_id, origin_node, origin_component, origin_path) = (
            request.origin,
            request.node.clone(),
            request.component.clone(),
            request.path.clone(),
        );
        self.transact_checked(&request.expected,|documents| {
            let origin=documents.get_mut(&request.origin).ok_or_else(||failure("origin document missing"))?;
            if remove_field(origin,&request.node,&request.component,&request.path)==0{return Err(failure("no originating override"));}
            let target=documents.get_mut(&request.target).ok_or_else(||failure("target document missing"))?;
            upsert(&mut target.overrides,target_operation);
            Ok(())
        },|catalog| {
            validate(catalog)?;
            let next=snapshot.recompile(catalog.resolve(origin_id)?,types,policies)?;
            let next_value=next.field_value(&origin_node,&origin_component,&origin_path,&[],policies,types)?;
            if original_value!=next_value{return Err(failure("Apply would change the originating effective value; destination is shadowed or cannot represent the reference"));}Ok(())
        })
    }
}
fn remove_field(
    document: &mut PrefabDocument,
    node: &NodeAddress,
    component: &str,
    path: &str,
) -> usize {
    remove_matching(document, node, |operation, node| {
        matches!(operation, Operation::SetField {node:n,component:c,path:p,..}
            if n == node && c == component && (p == path || p.starts_with(&format!("{path}."))))
    })
}

fn remove_component(
    document: &mut PrefabDocument,
    node: &NodeAddress,
    component: &str,
    include_removal: bool,
) -> usize {
    remove_matching(document, node, |operation, node| match operation {
        Operation::SetComponent {
            node: n,
            component: c,
            ..
        }
        | Operation::SetField {
            node: n,
            component: c,
            ..
        } => n == node && c == component,
        Operation::RemoveComponent {
            node: n,
            component: c,
        } => include_removal && n == node && c == component,
        _ => false,
    })
}

// The address is relative to each successive source-owned instance boundary.
// Never visit a referenced document: it is a separate transaction participant.
fn remove_matching(
    document: &mut PrefabDocument,
    node: &NodeAddress,
    matches: impl Fn(&Operation, &NodeAddress) -> bool,
) -> usize {
    fn operations(
        ops: &mut Vec<Operation>,
        node: &NodeAddress,
        matches: &impl Fn(&Operation, &NodeAddress) -> bool,
    ) -> usize {
        let before = ops.len();
        ops.retain(|operation| !matches(operation, node));
        let mut removed = before - ops.len();
        if let Some(id) = node.instances.first() {
            let local = NodeAddress {
                instances: node.instances[1..].to_vec(),
                node: node.node,
            };
            for operation in ops {
                if let Operation::AddNestedInstance(instance) = operation
                    && instance.id == *id
                {
                    removed += operations(&mut instance.overrides, &local, matches);
                }
            }
        }
        removed
    }
    let mut removed = operations(&mut document.overrides, node, &matches);
    if let Some(id) = node.instances.first() {
        let local = NodeAddress {
            instances: node.instances[1..].to_vec(),
            node: node.node,
        };
        for instance in &mut document.instances {
            if instance.id == *id {
                removed += operations(&mut instance.overrides, &local, &matches);
            }
        }
    }
    removed
}
fn upsert(operations: &mut Vec<Operation>, operation: Operation) {
    match &operation {
        Operation::SetField {node,component,path,..}=>operations.retain(|op|!matches!(op,Operation::SetField {node:n,component:c,path:p,..} if n==node&&c==component&&(p==path||p.starts_with(&format!("{path}."))))),
        Operation::SetComponent {node,component,..}=>operations.retain(|op|!matches!(op,Operation::SetComponent {node:n,component:c,..}|Operation::SetField {node:n,component:c,..} if n==node&&c==component)),
        Operation::RemoveComponent {node,component}=>operations.retain(|op|!matches!(op,Operation::SetComponent {node:n,component:c,..}|Operation::SetField {node:n,component:c,..}|Operation::RemoveComponent {node:n,component:c} if n==node&&c==component)),
        _=>{},
    }
    operations.push(operation);
}
