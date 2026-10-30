//! Validated structural requests reuse scene operations and the same isolated lease.
use super::*;
use kedra_scene::document::Operation;

impl SourceEdit {
    /// Stage a child in the receiving document, including beneath a composed parent.
    /// A fresh persistent ID is allocated once; commit/Undo/Redo retain it. Invalid
    /// parents, stale leases or compilation failures leave the candidate unchanged.
    /// Dropping this lease never publishes the returned node or writes files.
    pub fn add_node(
        &mut self,
        documents: &Workspace,
        parent: &SourceNode,
        name: &str,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<SourceNode, String> {
        if name.trim().is_empty() || name.len() > 1024 || name.chars().any(char::is_control) {
            return Err(
                "Node name must be nonempty, without control characters, and at most 1024 bytes"
                    .into(),
            );
        }
        let id = kedra_scene::document::NodeId::new();
        self.structure(
            documents,
            parent,
            Operation::AddNode(kedra_scene::document::Node {
                id,
                name: name.into(),
                parent: Some(parent.address.clone()),
                components: BTreeMap::new(),
            }),
            types,
            policies,
        )?;
        Ok(SourceNode {
            document: parent.document,
            address: id.into(),
            name: name.into(),
        })
    }

    /// Add one registered component to an existing candidate node. Existing keys
    /// refuse rather than replacing values. The complete typed catalog validates
    /// versions, fields and references before staging; failure changes neither
    /// candidate nor working state. Publication and Save remain separate.
    pub fn add_component(
        &mut self,
        documents: &Workspace,
        node: &SourceNode,
        component: &str,
        data: kedra_scene::document::ComponentData,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        if component.is_empty() || component.len() > 256 {
            return Err("Component key requires 1..=256 bytes".into());
        }
        let compiled = self.compile(documents, types, policies)?;
        let source = compiled
            .get(&node.document)
            .and_then(|prefab| prefab.graph().nodes.get(&node.address))
            .ok_or("Edit node unavailable")?;
        if source.components.contains_key(component) {
            return Err("Component already exists on this node".into());
        }
        self.structure(
            documents,
            node,
            Operation::SetComponent {
                node: node.address.clone(),
                component: component.into(),
                data,
            },
            types,
            policies,
        )
    }
    fn structure(
        &mut self,
        documents: &Workspace,
        node: &SourceNode,
        operation: Operation,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        self.node(node)?;
        let scope = self.current(documents)?;
        let compiled = self.compile(documents, types, policies)?;
        if !compiled
            .get(&node.document)
            .is_some_and(|prefab| prefab.graph().nodes.contains_key(&node.address))
        {
            return Err("Structural edit node unavailable".into());
        }
        self.transaction
            .operation(node.document, operation, |catalog| {
                source::compile_catalog(&scope.source, catalog, types, policies)
                    .map(|_| ())
                    .map_err(kedra_scene::Error)
            })
            .map_err(|error| error.to_string())?;
        self.operations += 1;
        Ok(())
    }
    /// Stage non-root subtree removal, refusing invalid composition atomically.
    pub fn remove_node(
        &mut self,
        documents: &Workspace,
        node: &SourceNode,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        self.structure(
            documents,
            node,
            Operation::RemoveNode(node.address.clone()),
            types,
            policies,
        )
    }
    /// Stage a move within one document: local nodes stay within their composition
    /// boundary; complete nested instance roots may change parents. Cycles refuse.
    pub fn reparent(
        &mut self,
        documents: &Workspace,
        node: &SourceNode,
        parent: &SourceNode,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        if node.document != parent.document {
            return Err("Reparent requires one receiving document".into());
        }
        self.structure(
            documents,
            node,
            Operation::ReparentNode {
                node: node.address.clone(),
                parent: parent.address.clone(),
            },
            types,
            policies,
        )
    }
    /// Stage removal of an existing component after complete typed validation.
    pub fn remove_component(
        &mut self,
        documents: &Workspace,
        node: &SourceNode,
        component: &str,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        if component.len() > 256 {
            return Err("Component key exceeds its byte limit".into());
        }
        let compiled = self.compile(documents, types, policies)?;
        if !compiled
            .get(&node.document)
            .and_then(|prefab| prefab.graph().nodes.get(&node.address))
            .is_some_and(|node| node.components.contains_key(component))
        {
            return Err("Edit component unavailable".into());
        }
        self.structure(
            documents,
            node,
            Operation::RemoveComponent {
                node: node.address.clone(),
                component: component.into(),
            },
            types,
            policies,
        )
    }
}
