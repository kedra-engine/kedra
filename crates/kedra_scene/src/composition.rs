//! Deterministic prefab composition with stable source addresses and override provenance.
use crate::{Result, document::*, failure};
use ron::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};

/// Source provenance retained through variants and nested composition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    /// Document that authored this value.
    pub document: DocumentId,
    /// Node in that source.
    pub node: NodeAddress,
    /// Scope used when resolving source-local references.
    pub scope: Vec<InstanceId>,
}
/// A validated non-overlapping field edit awaiting typed reflection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldPatch {
    /// Exact component schema version.
    pub version: u32,
    /// Reflected field path.
    pub path: String,
    /// Typed source RON.
    pub value: Box<RawValue>,
    /// Reference-resolution scope for the replacement value.
    pub origin: Origin,
}
/// One effective component plus its layered field edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedComponent {
    /// Whole-component source value.
    pub data: ComponentData,
    /// Source and reference scope of the component value.
    pub origin: Origin,
    /// Ordered field edits and their independent source scopes.
    pub fields: Vec<FieldPatch>,
}
/// Effective source node after composition, not an ECS entity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedNode {
    /// Fully scoped source identity.
    pub address: NodeAddress,
    /// Effective authored name.
    pub name: String,
    /// Effective parent, absent only on the root.
    pub parent: Option<NodeAddress>,
    /// Stable effective child order.
    pub children: Vec<NodeAddress>,
    /// Effective allowed component data.
    pub components: BTreeMap<String, ResolvedComponent>,
    /// Node's source provenance.
    pub origin: Origin,
}
/// A symbolic reference together with the scope where it was authored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopedReference {
    /// Owning source document.
    pub document: DocumentId,
    /// Source-local or external reference.
    pub reference: Reference,
    /// Source nesting scope.
    pub scope: Vec<InstanceId>,
}
/// Validated composition snapshot. Component values still require typed compilation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedGraph {
    // Exact transitive composition sources, including shadowed fields and metadata.
    // Kept in equality so effective-graph comparisons also guard source freshness.
    pub(crate) sources: BTreeMap<DocumentId, std::sync::Arc<PrefabDocument>>,
    /// Entry document identity.
    pub document: DocumentId,
    /// Effective root address.
    pub root: NodeAddress,
    /// Effective nodes indexed by scoped identity.
    pub nodes: BTreeMap<NodeAddress, ResolvedNode>,
    /// Nested external port declarations and bindings.
    pub bindings: BTreeMap<(Vec<InstanceId>, String), Option<ScopedReference>>,
    /// Root port requirements.
    pub ports: BTreeMap<String, bool>,
    /// Complete composition dependency identities.
    pub dependencies: BTreeSet<DocumentId>,
    /// Source identity at each nesting boundary.
    pub frames: BTreeMap<Vec<InstanceId>, DocumentId>,
    /// Explicit authored remaps, never legacy format aliases or runtime forwarding.
    pub aliases: BTreeMap<NodeAddress, NodeAddress>,
}

#[derive(bevy::prelude::Resource, Default, Clone)]
/// One source revision's document set. Duplicate identities are rejected on insertion.
pub struct Catalog {
    /// Source documents; edits are candidates until validated and published.
    pub documents: BTreeMap<DocumentId, PrefabDocument>,
}

// One request owns its cache, so a later edit cannot reuse stale transitive sources.
// Cache only unmodified document resolutions; instance-site overrides edit a clone.
#[derive(Default)]
struct Resolution {
    // Active document, greatest traversed subtree height, logical entry depth.
    // Site override scopes contribute depth without pretending to own a source.
    stack: Vec<(DocumentId, usize, usize)>,
    cache: BTreeMap<DocumentId, (ResolvedGraph, usize)>,
    work: usize,
    depth: usize,
}
impl Resolution {
    fn charge(&mut self, count: usize) -> Result<()> {
        self.work = self.work.saturating_add(count);
        if self.work > 65_536 {
            return Err(failure("prefab composition work budget exhausted"));
        }
        Ok(())
    }
    fn graph_cost(graph: &ResolvedGraph) -> usize {
        graph.nodes.len()
            + graph.bindings.len()
            + graph.frames.len()
            + graph.aliases.len()
            + graph.sources.len()
            + graph.ports.len()
            + graph.dependencies.len()
            + graph
                .nodes
                .values()
                .map(|node| {
                    node.components.len()
                        + node
                            .components
                            .values()
                            .map(|component| component.fields.len())
                            .sum::<usize>()
                })
                .sum::<usize>()
    }
    fn record_height(&mut self, height: usize) {
        if let Some((_, parent_height, entry_depth)) = self.stack.last_mut() {
            *parent_height = (*parent_height).max(height + self.depth - *entry_depth);
        }
    }
}
impl Catalog {
    /// Insert a source once; nil/duplicate document identities fail without replacement.
    pub fn insert(&mut self, document: PrefabDocument) -> Result<()> {
        if document.id.0.is_nil() || self.documents.contains_key(&document.id) {
            return Err(failure("duplicate document ID"));
        }
        self.documents.insert(document.id, document);
        Ok(())
    }
    /// Compose an entry point, rejecting missing sources, invalid edits and cycles/depth.
    /// A request-local immutable-document cache avoids repeated dependency traversal.
    /// At most 65536 source/operation/graph items may be processed in one request;
    /// exhausting this aggregate work budget returns an error, never a partial graph.
    pub fn resolve(&self, id: DocumentId) -> Result<ResolvedGraph> {
        if self.documents.len() > 128
            || self
                .documents
                .iter()
                .any(|(id, doc)| id.0.is_nil() || *id != doc.id)
        {
            return Err(failure("catalog document identities or size are invalid"));
        }
        self.resolve_inner(id, &mut Resolution::default())
    }
    /// Inspect the base chain; a missing or cyclic chain does not imply inheritance.
    pub fn inherits(&self, mut document: DocumentId, base: DocumentId) -> bool {
        let mut seen = BTreeSet::new();
        loop {
            if document == base {
                return true;
            }
            if !seen.insert(document) {
                return false;
            }
            let Some(parent) = self.documents.get(&document).and_then(|d| d.base) else {
                return false;
            };
            document = parent;
        }
    }
    fn resolve_inner(&self, id: DocumentId, state: &mut Resolution) -> Result<ResolvedGraph> {
        state.charge(1)?;
        if state.stack.iter().any(|(active, _, _)| *active == id) || state.depth >= 64 {
            return Err(failure("cyclic/deep prefab inheritance or nesting"));
        }
        if let Some((graph, height)) = state.cache.get(&id) {
            // A cached shallow visit must not make the same subtree legal deeper
            // down another branch. Height includes removed/overridden dependencies.
            if state.depth + height > 64
                || state
                    .stack
                    .iter()
                    .any(|(active, _, _)| graph.dependencies.contains(active))
            {
                return Err(failure("cyclic/deep prefab inheritance or nesting"));
            }
            let height = *height;
            let cost = Resolution::graph_cost(graph);
            state.charge(cost)?;
            let graph = state.cache[&id].0.clone();
            state.record_height(height);
            return Ok(graph);
        }
        let document = self
            .documents
            .get(&id)
            .ok_or_else(|| failure(format!("missing prefab {id}")))?;
        if document.version != 1 {
            return Err(failure("unsupported document version"));
        }
        state.charge(document.nodes.len() + document.instances.len())?;
        state.stack.push((id, 1, state.depth));
        state.depth += 1;
        let result = (|| {
            let mut graph = if let Some(base) = document.base {
                let base_document = self
                    .documents
                    .get(&base)
                    .ok_or_else(|| failure("missing base"))?;
                if base_document.kind != document.kind {
                    return Err(failure("base document kind mismatch"));
                }
                let graph = self.resolve_inner(base, state)?;
                if graph.root.node != document.root {
                    return Err(failure("variant changed inherited root identity"));
                }
                graph
            } else {
                ResolvedGraph {
                    sources: BTreeMap::new(),
                    document: id,
                    root: document.root.into(),
                    nodes: BTreeMap::new(),
                    bindings: BTreeMap::new(),
                    ports: BTreeMap::new(),
                    dependencies: BTreeSet::new(),
                    frames: BTreeMap::new(),
                    aliases: BTreeMap::new(),
                }
            };
            graph.document = id;
            graph
                .sources
                .insert(id, std::sync::Arc::new(document.clone()));
            graph.frames.insert(Vec::new(), id);
            graph.dependencies.insert(id);
            graph.ports.extend(document.ports.clone());
            for node in &document.nodes {
                graph.add_node(node, id)?;
            }
            for instance in &document.instances {
                self.nest(&mut graph, instance, id, state)?;
            }
            self.operations(&mut graph, &document.overrides, id, state)?;
            graph.validate()?;
            state.charge(Resolution::graph_cost(&graph))?;
            Ok(graph)
        })();
        state.depth -= 1;
        let height = state.stack.pop().map_or(1, |(_, height, _)| height);
        if let Ok(graph) = &result {
            state.cache.insert(id, (graph.clone(), height));
            state.record_height(height);
        }
        result
    }
    fn nest(
        &self,
        graph: &mut ResolvedGraph,
        instance: &NestedInstance,
        owner: DocumentId,
        state: &mut Resolution,
    ) -> Result<()> {
        if instance.id.0.is_nil() {
            return Err(failure("nil nested instance identity"));
        }
        if self
            .documents
            .get(&instance.asset)
            .is_some_and(|document| document.kind != DocumentKind::Prefab)
        {
            return Err(failure(
                "nested instances must reference a prefab, not a scene document",
            ));
        }
        if !graph.nodes.contains_key(&instance.parent) {
            return Err(failure("nested instance parent missing"));
        }
        if graph
            .nodes
            .keys()
            .any(|a| a.instances.first() == Some(&instance.id))
        {
            return Err(failure("duplicate nested instance identity"));
        }
        let mut child = self.resolve_inner(instance.asset, state)?;
        // Override-added sites are still composition scopes. Account for their
        // depth while retaining the actual source owner for references/cycles.
        state.depth += 1;
        let result = self.operations(&mut child, &instance.overrides, owner, state);
        state.depth -= 1;
        result?;
        child.validate()?;
        if graph.nodes.len().saturating_add(child.nodes.len()) > 4096 {
            return Err(failure("prefab exceeds 4096 nodes"));
        }
        let prefix = [instance.id];
        for (address, mut node) in child.nodes {
            node.address = address.scoped(&prefix);
            node.parent = if address == child.root {
                Some(instance.parent.clone())
            } else {
                node.parent.map(|p| p.scoped(&prefix))
            };
            node.children = node
                .children
                .into_iter()
                .map(|a| a.scoped(&prefix))
                .collect();
            node.origin.scope.splice(0..0, prefix);
            for component in node.components.values_mut() {
                if component.origin.document != owner {
                    component.origin.scope.splice(0..0, prefix);
                } else {
                    component.origin.node = component.origin.node.scoped(&prefix);
                }
                for field in &mut component.fields {
                    if field.origin.document != owner {
                        field.origin.scope.splice(0..0, prefix);
                    } else {
                        field.origin.node = field.origin.node.scoped(&prefix);
                    }
                }
            }
            if graph.nodes.insert(node.address.clone(), node).is_some() {
                return Err(failure("duplicate resolved node"));
            }
        }
        for ((scope, port), mut binding) in child.bindings {
            if let Some(binding) = &mut binding
                && binding.document != owner
            {
                binding.scope.splice(0..0, prefix);
            }
            let scope = prefix.iter().chain(scope.iter()).copied().collect();
            graph.bindings.insert((scope, port), binding);
        }
        for (port, required) in child.ports {
            let binding = instance
                .bindings
                .get(&port)
                .cloned()
                .map(|reference| ScopedReference {
                    document: owner,
                    reference,
                    scope: Vec::new(),
                });
            if required && binding.is_none() {
                return Err(failure(format!("required nested port {port} is unbound")));
            }
            graph.bindings.insert((prefix.to_vec(), port), binding);
        }
        if instance
            .bindings
            .keys()
            .any(|p| !graph.bindings.contains_key(&(prefix.to_vec(), p.clone())))
        {
            return Err(failure("unknown nested port binding"));
        }
        graph.dependencies.extend(child.dependencies);
        graph.sources.extend(child.sources);
        for (from, to) in child.aliases {
            graph
                .aliases
                .insert(from.scoped(&prefix), to.scoped(&prefix));
        }
        for (scope, document) in child.frames {
            graph.frames.insert(
                prefix.iter().chain(scope.iter()).copied().collect(),
                document,
            );
        }
        Ok(())
    }
    fn operations(
        &self,
        graph: &mut ResolvedGraph,
        operations: &[Operation],
        owner: DocumentId,
        state: &mut Resolution,
    ) -> Result<()> {
        state.charge(operations.len())?;
        let mut fields = BTreeSet::<(NodeAddress, String, String)>::new();
        for operation in operations {
            match operation {
                Operation::AddNode(node) => graph.add_node(node, owner)?,
                Operation::AddNestedInstance(instance) => {
                    self.nest(graph, instance, owner, state)?
                }
                Operation::RemoveNestedInstance(id) => {
                    self.operations(
                        graph,
                        &[Operation::RemoveNestedInstanceAt(vec![*id])],
                        owner,
                        state,
                    )?;
                }
                Operation::RemoveNestedInstanceAt(prefix) => {
                    if prefix.is_empty() {
                        return Err(failure("cannot unpack the document root"));
                    }
                    let asset = graph
                        .frames
                        .get(prefix)
                        .ok_or_else(|| failure("nested instance no longer exists"))?;
                    let root = NodeAddress {
                        instances: prefix.clone(),
                        node: self
                            .documents
                            .get(asset)
                            .ok_or_else(|| failure("missing instance source"))?
                            .root,
                    };
                    let mut removed = BTreeSet::from([root]);
                    loop {
                        let before = removed.len();
                        for node in graph.nodes.values() {
                            if node.parent.as_ref().is_some_and(|p| removed.contains(p)) {
                                removed.insert(node.address.clone());
                            }
                        }
                        if before == removed.len() {
                            break;
                        }
                    }
                    graph.nodes.retain(|address, _| !removed.contains(address));
                    let scopes: BTreeSet<_> = graph
                        .frames
                        .iter()
                        .filter_map(|(scope, asset)| {
                            self.documents
                                .get(asset)
                                .filter(|document| {
                                    removed.contains(&NodeAddress {
                                        instances: scope.clone(),
                                        node: document.root,
                                    })
                                })
                                .map(|_| scope.clone())
                        })
                        .collect();
                    graph
                        .bindings
                        .retain(|(scope, _), _| !scopes.contains(scope));
                    graph.frames.retain(|scope, _| !scopes.contains(scope));
                }
                Operation::RemapReferences(aliases) => {
                    for (from, to) in aliases {
                        if graph.nodes.contains_key(from) || graph.aliases.contains_key(from) {
                            return Err(failure(
                                "reference remap shadows a live/existing identity",
                            ));
                        }
                        graph.aliases.insert(from.clone(), to.clone());
                    }
                }
                Operation::RemoveNode(address) => {
                    let address = graph.canonical(address)?;
                    if address == graph.root {
                        return Err(failure("cannot delete prefab root"));
                    }
                    graph.node(&address)?;
                    let mut removed = BTreeSet::from([address.clone()]);
                    loop {
                        let before = removed.len();
                        for node in graph.nodes.values() {
                            if node.parent.as_ref().is_some_and(|p| removed.contains(p)) {
                                removed.insert(node.address.clone());
                            }
                        }
                        if before == removed.len() {
                            break;
                        }
                    }
                    graph.nodes.retain(|key, _| !removed.contains(key));
                }
                Operation::RenameNode { node, name } => graph.node_mut(node)?.name = name.clone(),
                Operation::ReparentNode { node, parent } => {
                    let instance_root = graph
                        .frames
                        .get(&node.instances)
                        .and_then(|id| graph.sources.get(id))
                        .is_some_and(|source| {
                            source.root == node.node && !node.instances.is_empty()
                        });
                    if node.instances != parent.instances && !instance_root {
                        return Err(failure(
                            "Move the whole prefab instance; internal nodes cannot cross its boundary",
                        ));
                    }
                    if *node == graph.root {
                        return Err(failure("cannot reparent root"));
                    }
                    graph.node(parent)?;
                    graph.node_mut(node)?.parent = Some(parent.clone());
                }
                Operation::SetComponent {
                    node,
                    component,
                    data,
                } => {
                    if fields.iter().any(|(n, c, _)| n == node && c == component) {
                        return Err(failure("overlapping component/field patches in one layer"));
                    }
                    fields.insert((node.clone(), component.clone(), String::new()));
                    graph.node_mut(node)?.components.insert(
                        component.clone(),
                        ResolvedComponent {
                            data: data.clone(),
                            origin: Origin {
                                document: owner,
                                node: node.clone(),
                                scope: Vec::new(),
                            },
                            fields: Vec::new(),
                        },
                    );
                }
                Operation::RemoveComponent { node, component } => {
                    if graph.node_mut(node)?.components.remove(component).is_none() {
                        return Err(failure("component override target no longer exists"));
                    }
                }
                Operation::SetField {
                    node,
                    component,
                    version,
                    path,
                    value,
                } => {
                    if path.is_empty()
                        || path.contains('[')
                        || path.contains('#')
                        || fields.iter().any(|(n, c, p)| {
                            n == node
                                && c == component
                                && (p == path
                                    || p.is_empty()
                                    || path.starts_with(&format!("{p}."))
                                    || p.starts_with(&format!("{path}.")))
                        })
                    {
                        return Err(failure("ambiguous overlapping field patches"));
                    }
                    fields.insert((node.clone(), component.clone(), path.clone()));
                    graph
                        .node_mut(node)?
                        .components
                        .get_mut(component)
                        .ok_or_else(|| failure("field patch component no longer exists"))?
                        .fields
                        .push(FieldPatch {
                            version: *version,
                            path: path.clone(),
                            value: value.clone(),
                            origin: Origin {
                                document: owner,
                                node: node.clone(),
                                scope: Vec::new(),
                            },
                        });
                }
                Operation::SetChildOrder { node, children } => {
                    graph.rebuild_children();
                    let current: BTreeSet<_> = graph.node(node)?.children.iter().cloned().collect();
                    if children.iter().cloned().collect::<BTreeSet<_>>() != current
                        || children.len() != current.len()
                    {
                        return Err(failure(
                            "child order must contain each direct child exactly once",
                        ));
                    }
                    graph.node_mut(node)?.children = children.clone();
                }
                Operation::SetBinding {
                    instance,
                    port,
                    target,
                } => {
                    let binding = graph
                        .bindings
                        .get_mut(&(vec![*instance], port.clone()))
                        .ok_or_else(|| failure("unknown instance port"))?;
                    *binding = Some(ScopedReference {
                        document: owner,
                        reference: target.clone(),
                        scope: Vec::new(),
                    });
                }
            }
            if graph.nodes.len() > 4096 {
                return Err(failure("prefab exceeds 4096 nodes"));
            }
        }
        Ok(())
    }
}
impl ResolvedGraph {
    fn add_node(&mut self, node: &Node, owner: DocumentId) -> Result<()> {
        if self.nodes.len() >= 4096 {
            return Err(failure("prefab exceeds 4096 nodes"));
        }
        let address: NodeAddress = node.id.into();
        if node.id.0.is_nil() || self.nodes.contains_key(&address) {
            return Err(failure("duplicate/nil node identity"));
        }
        let origin = Origin {
            document: owner,
            node: address.clone(),
            scope: Vec::new(),
        };
        let components = node
            .components
            .iter()
            .map(|(key, data)| {
                (
                    key.clone(),
                    ResolvedComponent {
                        data: data.clone(),
                        origin: origin.clone(),
                        fields: Vec::new(),
                    },
                )
            })
            .collect();
        self.nodes.insert(
            address.clone(),
            ResolvedNode {
                address,
                name: node.name.clone(),
                parent: node.parent.clone(),
                children: Vec::new(),
                components,
                origin,
            },
        );
        Ok(())
    }
    fn node(&self, address: &NodeAddress) -> Result<&ResolvedNode> {
        let address = self.canonical(address)?;
        self.nodes
            .get(&address)
            .ok_or_else(|| failure("override target no longer exists"))
    }
    fn node_mut(&mut self, address: &NodeAddress) -> Result<&mut ResolvedNode> {
        let address = self.canonical(address)?;
        self.nodes
            .get_mut(&address)
            .ok_or_else(|| failure("override target no longer exists"))
    }
    fn rebuild_children(&mut self) {
        let links: Vec<_> = self
            .nodes
            .values()
            .filter_map(|n| n.parent.clone().map(|p| (p, n.address.clone())))
            .collect();
        for node in self.nodes.values_mut() {
            node.children
                .retain(|c| links.contains(&(node.address.clone(), c.clone())));
        }
        for (parent, child) in links {
            if let Some(node) = self.nodes.get_mut(&parent)
                && !node.children.contains(&child)
            {
                node.children.push(child);
            }
        }
    }
    /// Validate hierarchy/remaps and rebuild child links without changing sources.
    /// Hierarchy parents must name live nodes directly: remaps apply to symbolic
    /// component/port references, not to hierarchy topology or ownership.
    pub fn validate(&mut self) -> Result<()> {
        if !self.nodes.contains_key(&self.root)
            || self
                .nodes
                .iter()
                .any(|(address, node)| *address != node.address)
        {
            return Err(failure("resolved graph node/root identities are invalid"));
        }
        for address in self.aliases.keys() {
            if self.nodes.contains_key(address) {
                return Err(failure("reference remap shadows a live node"));
            }
            let target = self.canonical(address)?;
            if !self.nodes.contains_key(&target) {
                return Err(failure("dangling reference remap"));
            }
        }
        if self.nodes.len() > 4096 || self.node(&self.root)?.parent.is_some() {
            return Err(failure("invalid prefab root/size"));
        }
        for node in self.nodes.values() {
            if node
                .parent
                .as_ref()
                .is_some_and(|parent| !self.nodes.contains_key(parent))
            {
                return Err(failure(
                    "hierarchy parent must name a live node, not a reference alias",
                ));
            }
            let mut cursor = node.address.clone();
            let mut seen = BTreeSet::new();
            while cursor != self.root {
                if !seen.insert(cursor.clone()) {
                    return Err(failure("cyclic node hierarchy"));
                }
                cursor = self
                    .node(&cursor)?
                    .parent
                    .clone()
                    .ok_or_else(|| failure("disconnected node hierarchy"))?;
            }
        }
        self.rebuild_children();
        Ok(())
    }
    /// Resolve explicit source remaps, rejecting cycles. Does not invent missing targets.
    pub fn canonical(&self, address: &NodeAddress) -> Result<NodeAddress> {
        let mut target = address.clone();
        let mut seen = BTreeSet::new();
        while let Some(next) = self.aliases.get(&target) {
            if !seen.insert(target) {
                return Err(failure("cyclic reference remap"));
            }
            target = next.clone();
        }
        Ok(target)
    }
    pub(crate) fn port(
        &self,
        scope: &[InstanceId],
        name: &str,
        root: &BTreeMap<String, Option<bevy::prelude::Entity>>,
        entities: &BTreeMap<NodeAddress, bevy::prelude::Entity>,
        seen: &mut BTreeSet<(Vec<InstanceId>, String)>,
    ) -> Result<Option<bevy::prelude::Entity>> {
        if !seen.insert((scope.to_vec(), name.into())) {
            return Err(failure("cyclic port bindings"));
        }
        if scope.is_empty() {
            return root
                .get(name)
                .copied()
                .ok_or_else(|| failure(format!("undeclared/unbound port {name}")));
        }
        match self
            .bindings
            .get(&(scope.to_vec(), name.into()))
            .ok_or_else(|| failure("unknown nested port"))?
        {
            None => Ok(None),
            Some(binding) => match &binding.reference {
                Reference::Local(address) => entities
                    .get(&self.canonical(&address.scoped(&binding.scope))?)
                    .copied()
                    .map(Some)
                    .ok_or_else(|| failure("dangling port target")),
                Reference::Port(name) => self.port(&binding.scope, name, root, entities, seen),
            },
        }
    }
}

#[cfg(test)]
#[path = "composition_tests.rs"]
mod tests;
