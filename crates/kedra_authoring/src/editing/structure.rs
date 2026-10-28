//! Validated structural requests reuse scene operations and the same isolated lease.
use super::*;
use kedra_scene::document::Operation;

impl SourceEdit {
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
    /// Stage a move within one document/composition boundary. Cycles refuse.
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
