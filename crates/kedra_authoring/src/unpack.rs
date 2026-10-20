//! Complete scoped unpack with explicit source identity remapping and undo history.
use crate::{failure, workspace::WorkingSet};
use bevy::reflect::TypeRegistry;
use kedra_scene::{
    Result, composition::Catalog, document::*, instance::CompiledPrefab,
    reflection::ComponentPolicies,
};
use std::collections::{BTreeMap, BTreeSet};

impl WorkingSet {
    /// Explicit complete unpack, including descendants. Other instances keep their links.
    /// Requires an exact current composition snapshot, including transitive source values.
    pub fn unpack_complete(
        &mut self,
        request: Unpack,
        snapshot: &CompiledPrefab,
        policies: &ComponentPolicies,
        types: &TypeRegistry,
        validate: impl FnOnce(&Catalog) -> Result<()>,
    ) -> Result<BTreeMap<NodeAddress, NodeAddress>> {
        if request.scope.is_empty() || snapshot.graph().document != request.document {
            return Err(failure("select a nested instance to unpack"));
        }
        self.check_snapshot(snapshot)?;
        let symbolic = snapshot.symbolic_source(types, policies)?;
        let snapshot = &symbolic;
        let catalog = self.catalog();
        let asset = snapshot
            .graph()
            .frames
            .get(&request.scope)
            .ok_or_else(|| failure("instance missing"))?;
        let source = catalog
            .documents
            .get(asset)
            .ok_or_else(|| failure("instance source missing"))?;
        let root = NodeAddress {
            instances: request.scope.clone(),
            node: source.root,
        };
        let mut members = BTreeSet::from([root.clone()]);
        loop {
            let before = members.len();
            for node in snapshot.graph().nodes.values() {
                if node.parent.as_ref().is_some_and(|p| members.contains(p)) {
                    members.insert(node.address.clone());
                }
            }
            if before == members.len() {
                break;
            }
        }
        let remap: BTreeMap<_, _> = members
            .iter()
            .map(|address| {
                (
                    address.clone(),
                    if address.instances.is_empty() {
                        address.clone()
                    } else {
                        NodeId::new().into()
                    },
                )
            })
            .collect();
        let mut copies = Vec::new();
        for address in &members {
            let node = &snapshot.graph().nodes[address];
            let components = node
                .components
                .keys()
                .map(|key| {
                    snapshot
                        .component(address, key, policies, types, &remap)
                        .map(|data| (key.clone(), data))
                })
                .collect::<Result<_>>()?;
            copies.push(Node {
                id: remap[address].node,
                name: node.name.clone(),
                parent: node
                    .parent
                    .as_ref()
                    .map(|parent| remap.get(parent).unwrap_or(parent).clone()),
                components,
            });
        }
        let orders: Vec<_> = snapshot
            .graph()
            .nodes
            .values()
            .filter(|node| {
                members.contains(&node.address)
                    || node.children.iter().any(|child| members.contains(child))
            })
            .map(|node| Operation::SetChildOrder {
                node: remap.get(&node.address).unwrap_or(&node.address).clone(),
                children: node
                    .children
                    .iter()
                    .map(|child| remap.get(child).unwrap_or(child).clone())
                    .collect(),
            })
            .collect();
        let aliases: BTreeMap<_, _> = remap
            .iter()
            .filter(|(from, to)| from != to)
            .map(|(from, to)| (from.clone(), to.clone()))
            .collect();
        let expected = BTreeMap::from([(request.document, request.revision)]);
        self.transact_checked(&expected,|documents| {
            let document=documents.get_mut(&request.document).ok_or_else(||failure("document missing"))?;
            let declared=request.scope.len()==1&&(document.instances.iter().any(|i|i.id==request.scope[0])||document.overrides.iter().any(|op|matches!(op,Operation::AddNestedInstance(i) if i.id==request.scope[0])));
            document.nodes.retain(|node|!members.contains(&node.id.into()));
            document.instances.retain(|instance|!members.contains(&NodeAddress {instances:vec![instance.id],node:catalog.documents[&instance.asset].root}));
            let mut retained=Vec::new();
            for operation in std::mem::take(&mut document.overrides) {
                if touches(&operation,&request.scope,&members,&catalog){continue;}
                let operation=match operation {
                    Operation::SetChildOrder {node,children}=>Operation::SetChildOrder {node,children:children.into_iter().map(|child|remap.get(&child).unwrap_or(&child).clone()).collect()},
                    other=>other,
                };
                retained.push(operation);
            }
            document.nodes.extend(copies);
            if !declared {document.overrides.push(Operation::RemoveNestedInstanceAt(request.scope.clone()));}
            document.overrides.push(Operation::RemapReferences(aliases));
            document.overrides.extend(retained);
            document.overrides.extend(orders);
            Ok(())
        },validate)?;
        let mapping = Some((request.document, remap.clone()));
        if let Some(history) = self.undo.last_mut() {
            history.mapping = mapping.clone();
        }
        self.last_mapping = mapping;
        Ok(remap)
    }
}
/// Complete unpack request against one exact source revision and nesting scope.
pub struct Unpack {
    /// Source document to edit.
    pub document: DocumentId,
    /// Expected current source revision.
    pub revision: u64,
    /// Nonempty outer-to-inner composition site identities.
    pub scope: Vec<InstanceId>,
}

fn touches(
    operation: &Operation,
    prefix: &[InstanceId],
    members: &BTreeSet<NodeAddress>,
    catalog: &Catalog,
) -> bool {
    let target = match operation {
        Operation::SetField { node, .. }
        | Operation::SetComponent { node, .. }
        | Operation::RemoveComponent { node, .. }
        | Operation::RemoveNode(node)
        | Operation::RenameNode { node, .. }
        | Operation::SetChildOrder { node, .. }
        | Operation::ReparentNode { node, .. } => Some(node.clone()),
        Operation::AddNode(node) => Some(node.id.into()),
        Operation::AddNestedInstance(instance) => {
            catalog.documents.get(&instance.asset).map(|d| NodeAddress {
                instances: vec![instance.id],
                node: d.root,
            })
        }
        Operation::RemoveNestedInstance(id) => return prefix.len() == 1 && prefix[0] == *id,
        Operation::RemoveNestedInstanceAt(scope) => return scope.starts_with(prefix),
        Operation::SetBinding { instance, .. } => {
            return prefix.len() == 1 && prefix[0] == *instance;
        }
        _ => None,
    };
    target.is_some_and(|node| node.instances.starts_with(prefix) || members.contains(&node))
}
