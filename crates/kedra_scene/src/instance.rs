//! Isolated compilation and transactional instantiation using Bevy DynamicWorld.
use crate::{Result, composition::*, document::*, failure, reflection::*};
use bevy::{
    asset::{AssetPath, LoadFromPath, LoadState, UntypedHandle},
    ecs::{entity::EntityHashMap, reflect::ReflectComponent},
    prelude::*,
    reflect::{ReflectFromReflect, ReflectPath, TypeRegistry, serde::TypedReflectDeserializer},
    world_serialization::{DynamicWorld, DynamicWorldBuilder, WorldFilter},
};
use serde::de::DeserializeSeed;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Component, Clone, Debug)]
/// Live node's ownership and source identity. Removed with its instance subtree.
pub struct PrefabNode {
    /// Runtime instance owner, unique for each materialization.
    pub instance: InstanceId,
    /// Fully scoped source address.
    pub address: NodeAddress,
}
#[derive(Component, Clone)]
/// Live instance root and complete owned node mapping. Despawning its hierarchy
/// must not despawn external port targets, source assets or another instance.
/// Ownership does not impose spatial/visibility components: source components or
/// their native presentation adapter own those, including non-spatial UI prefabs.
pub struct PrefabInstance {
    /// This materialization's identity.
    pub id: InstanceId,
    /// Source entry document identity.
    pub document: DocumentId,
    /// Source-to-live mapping for owned nodes only.
    pub nodes: BTreeMap<NodeAddress, Entity>,
    /// The exact immutable compilation owned by this instance, including dependencies.
    pub source: std::sync::Arc<CompiledPrefab>,
}
#[derive(EntityEvent)]
/// Emitted only after complete successful instantiation. Not a GPU/frame-ready signal.
pub struct PrefabReady {
    /// Complete live root.
    pub entity: Entity,
    /// Materialization identity.
    pub instance: InstanceId,
}
/// CPU render assets have been materialized. Shader/GPU readiness still needs a frame.
#[derive(Component)]
pub struct PrefabRenderReady;

/// Authored reflected value and the symbolic identity of its entity references.
pub type AuthoredValue = (
    Box<dyn bevy::reflect::PartialReflect>,
    BTreeMap<Entity, Reference>,
);

/// Immutable compilation result. Source edits create another snapshot, never mutate this one.
pub struct CompiledPrefab {
    /// Effective source composition used to produce this immutable compilation.
    graph: ResolvedGraph,
    /// Asset links may form cycles (for example scene transitions); composition may not.
    document_links: BTreeSet<DocumentId>,
    authoring: World,
    dynamic: DynamicWorld,
    entities: BTreeMap<NodeAddress, Entity>,
    ports: BTreeMap<String, Option<Entity>>,
    dependencies: Vec<UntypedHandle>,
    dependency_paths: Vec<(AssetPath<'static>, UntypedHandle)>,
}
impl CompiledPrefab {
    /// Compile in an isolated world using registered Bevy component policies.
    /// Invalid fields, references, schemas or dependencies never mutate the live world.
    /// All nested bindings are checked, including ports unused by effective components.
    pub fn compile(
        mut graph: ResolvedGraph,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
        loader: &mut dyn LoadFromPath,
        port_presence: &BTreeMap<String, bool>,
    ) -> Result<Self> {
        graph.validate()?;
        crate::environment::validate_assignment(&graph)?;
        let mut tracking = TrackingLoader {
            inner: loader,
            dependencies: Vec::new(),
            paths: Vec::new(),
        };
        let mut world = World::new();
        let mut document_links = BTreeSet::new();
        let entities: BTreeMap<_, _> = graph
            .nodes
            .iter()
            .map(|(address, node)| {
                (
                    address.clone(),
                    world.spawn(Name::new(node.name.clone())).id(),
                )
            })
            .collect();
        let mut ports = BTreeMap::new();
        for (name, required) in &graph.ports {
            let present = port_presence.get(name).copied().unwrap_or(*required);
            if *required && !present {
                return Err(failure(format!("required port {name} missing")));
            }
            ports.insert(name.clone(), present.then(|| world.spawn_empty().id()));
        }
        if port_presence
            .keys()
            .any(|name| !graph.ports.contains_key(name))
        {
            return Err(failure("unknown external port"));
        }
        let mut reverse: BTreeMap<_, _> = entities
            .iter()
            .map(|(address, entity)| (*entity, Reference::Local(address.clone())))
            .collect();
        reverse.extend(
            ports
                .iter()
                .filter_map(|(name, entity)| entity.map(|e| (e, Reference::Port(name.clone())))),
        );
        let mut references = entities.clone();
        for address in graph.aliases.keys() {
            references.insert(address.clone(), entities[&graph.canonical(address)?]);
        }
        // Validate all bindings, including unused ports in component-free scopes.
        // Otherwise adding a component later could reveal a previously admitted
        // dangling local/forwarded target or a binding cycle.
        for (scope, name) in graph.bindings.keys() {
            graph.port(scope, name, &ports, &references, &mut BTreeSet::new())?;
        }
        for (address, node) in &graph.nodes {
            let entity = entities[address];
            for (key, component) in &node.components {
                let scoped_ports = ports_for(&graph, &component.origin.scope, &ports, &references)?;
                let mut context = ReferenceContext {
                    entities: &references,
                    ports: &scoped_ports,
                    prefix: &component.origin.scope,
                    loader: &mut tracking,
                };
                let mut value = policies.decode(key, &component.data, types, &mut context)?;
                for field in &component.fields {
                    policies.policy(key, field.version)?;
                    let target = field
                        .path
                        .as_str()
                        .reflect_element_mut(value.as_mut())
                        .map_err(|e| failure(format!("{key}.{}: {e}", field.path)))?;
                    let info = target
                        .get_represented_type_info()
                        .ok_or_else(|| failure("field lacks reflected type"))?;
                    let registration = types
                        .get(info.type_id())
                        .ok_or_else(|| failure("field type is unregistered"))?;
                    let scoped_ports = ports_for(&graph, &field.origin.scope, &ports, &references)?;
                    let mut context = ReferenceContext {
                        entities: &references,
                        ports: &scoped_ports,
                        prefix: &field.origin.scope,
                        loader: &mut tracking,
                    };
                    let mut de = ron::Deserializer::from_str(field.value.get_ron())?;
                    let patch =
                        TypedReflectDeserializer::with_processor(registration, types, &mut context)
                            .deserialize(&mut de)?;
                    de.end()?;
                    target
                        .try_apply(patch.as_ref())
                        .map_err(|e| failure(e.to_string()))?;
                }
                let policy = policies.policy(key, component.data.version)?;
                (policy.validate)(value.as_ref())?;
                collect_document_links(value.as_ref(), &mut document_links);
                // Also prove that Component::map_entities visits every persisted Entity.
                policies.encode(key, value.as_ref(), types, &reverse)?;
                let registration = types
                    .get(policy.type_id)
                    .ok_or_else(|| failure("missing component type"))?;
                let concrete = registration
                    .data::<ReflectFromReflect>()
                    .and_then(|f| f.from_reflect(value.as_ref()))
                    .ok_or_else(|| {
                        failure("component cannot be constructed from authored fields")
                    })?;
                registration
                    .data::<ReflectComponent>()
                    .ok_or_else(|| failure("not a component"))?
                    .insert(
                        &mut world.entity_mut(entity),
                        concrete.as_partial_reflect(),
                        types,
                    );
            }
        }
        // Enforce child order in the same operation which creates the hierarchy.
        for node in graph.nodes.values() {
            let parent = entities[&node.address];
            for child in &node.children {
                world.entity_mut(entities[child]).insert(ChildOf(parent));
            }
        }
        let mut filter = WorldFilter::deny_all()
            .allow::<Name>()
            .allow::<ChildOf>()
            .allow::<Children>();
        for policy in policies.0.values() {
            filter = filter.allow_by_id(policy.type_id);
        }
        let dynamic = DynamicWorldBuilder::from_world(&world, types)
            .with_component_filter(filter)
            .with_resource_filter(WorldFilter::deny_all())
            .extract_entities(entities.values().copied())
            .build();
        Ok(Self {
            graph,
            document_links,
            authoring: world,
            dynamic,
            entities,
            ports,
            dependencies: tracking.dependencies,
            dependency_paths: tracking.paths,
        })
    }
    /// Whether every recorded native asset dependency is loaded; failures retain their cause.
    pub fn asset_status(&self, server: &AssetServer) -> Result<bool> {
        let mut ready = true;
        for handle in &self.dependencies {
            if let Some(LoadState::Failed(error)) = server.get_load_state(handle.id()) {
                return Err(failure(error.to_string()));
            }
            if let Some(bevy::asset::RecursiveDependencyLoadState::Failed(error)) =
                server.get_recursive_dependency_load_state(handle.id())
            {
                return Err(failure(error.to_string()));
            }
            ready &= server.is_loaded_with_dependencies(handle.id());
        }
        Ok(ready)
    }
    /// Whether materialization requires native asset readiness.
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
    }
    /// Test ownership of a tracked dependency, including subasset identities.
    pub fn depends_on(&self, id: bevy::asset::UntypedAssetId) -> bool {
        self.dependencies.iter().any(|handle| handle.id() == id)
    }
    /// Read the isolated compiled source world, never the gameplay instance world.
    pub fn authoring_world(&self) -> &World {
        &self.authoring
    }
    /// Read the immutable composed source graph of this compilation.
    pub fn graph(&self) -> &ResolvedGraph {
        &self.graph
    }
    /// Referenced source documents, distinct from native asset dependencies.
    pub fn document_links(&self) -> &BTreeSet<DocumentId> {
        &self.document_links
    }
    /// Encode an effective field in its author's reference scope for source editing.
    /// Missing nodes/components/fields or invalid symbolic references fail explicitly.
    pub fn field_value(
        &self,
        node: &NodeAddress,
        key: &str,
        path: &str,
        scope: &[InstanceId],
        policies: &ComponentPolicies,
        types: &TypeRegistry,
    ) -> Result<Box<ron::value::RawValue>> {
        let entity = *self
            .entities
            .get(node)
            .ok_or_else(|| failure("node missing"))?;
        let policy = policies
            .0
            .get(key)
            .ok_or_else(|| failure("unknown component"))?;
        let reflect = types
            .get_type_data::<ReflectComponent>(policy.type_id)
            .ok_or_else(|| failure("not a reflected component"))?;
        let component = reflect
            .reflect(self.authoring.entity(entity))
            .ok_or_else(|| failure("component missing"))?;
        let field = if path.is_empty() {
            component.as_partial_reflect()
        } else {
            path.reflect_element(component.as_partial_reflect())
                .map_err(|e| failure(e.to_string()))?
        };
        let mut reverse: BTreeMap<_, _> = self
            .entities
            .iter()
            .filter(|(address, _)| address.instances.starts_with(scope))
            .map(|(address, entity)| {
                (
                    *entity,
                    Reference::Local(NodeAddress {
                        instances: address.instances[scope.len()..].to_vec(),
                        node: address.node,
                    }),
                )
            })
            .collect();
        for (name, target) in ports_for(&self.graph, scope, &self.ports, &self.entities)? {
            if let Some(entity) = target {
                reverse.entry(entity).or_insert(Reference::Port(name));
            }
        }
        policies.encode_value_paths(field, types, &reverse, &self.dependency_paths)
    }
    /// Immutable mapping into the isolated source world, not live instance handles.
    pub fn source_entities(&self) -> &BTreeMap<NodeAddress, Entity> {
        &self.entities
    }
    /// Exact logical loader request recorded by this immutable compilation for a
    /// native handle, including its source and subasset label. Returns `None` for
    /// foreign handles or when a host collapsed distinct paths onto one identity.
    /// This borrows the existing dependency records; it neither resolves a newer
    /// source catalog nor loads assets or grants assignment/publication authority.
    pub fn dependency_path(&self, handle: &UntypedHandle) -> Option<&AssetPath<'static>> {
        let mut paths = self
            .dependency_paths
            .iter()
            .filter(|(_, recorded)| recorded.id() == handle.id());
        let (path, _) = paths.next()?;
        paths.all(|(other, _)| other == path).then_some(path)
    }
    /// Symbolic references for encoding isolated values, including explicit ports.
    pub fn references(&self) -> BTreeMap<Entity, Reference> {
        let mut values: BTreeMap<_, _> = self
            .entities
            .iter()
            .map(|(address, entity)| (*entity, Reference::Local(address.clone())))
            .collect();
        values.extend(self.ports.iter().filter_map(|(name, entity)| {
            entity.map(|entity| (entity, Reference::Port(name.clone())))
        }));
        values
    }
    /// Decode source data without lowering an unbound optional port to runtime None.
    /// Synthetic IDs exist only in this serialization context and always encode as ports.
    pub fn authored_value(
        &self,
        key: &str,
        data: &ComponentData,
        scope: &[InstanceId],
        policies: &ComponentPolicies,
        types: &TypeRegistry,
        loader: &mut dyn LoadFromPath,
    ) -> Result<(
        Box<dyn bevy::reflect::PartialReflect>,
        BTreeMap<Entity, Reference>,
    )> {
        let mut entities = self.entities.clone();
        for address in self.graph.aliases.keys() {
            entities.insert(
                address.clone(),
                self.entities[&self.graph.canonical(address)?],
            );
        }
        let mut ports = ports_for(&self.graph, scope, &self.ports, &entities)?;
        let mut temporary = World::new();
        let mut used = self
            .entities
            .values()
            .copied()
            .chain(self.ports.values().flatten().copied())
            .collect::<BTreeSet<_>>();
        let mut reverse = self
            .entities
            .iter()
            .filter(|(address, _)| address.instances.starts_with(scope))
            .map(|(address, entity)| {
                (
                    *entity,
                    Reference::Local(NodeAddress {
                        instances: address.instances[scope.len()..].to_vec(),
                        node: address.node,
                    }),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for (name, target) in &mut ports {
            let entity = loop {
                let entity = temporary.spawn_empty().id();
                if used.insert(entity) {
                    break entity;
                }
            };
            *target = Some(entity);
            reverse.insert(entity, Reference::Port(name.clone()));
        }
        let mut context = ReferenceContext {
            entities: &entities,
            ports: &ports,
            prefix: scope,
            loader,
        };
        let value = policies.decode(key, data, types, &mut context)?;
        Ok((value, reverse))
    }
    /// Recover a local effective source component for an authoring transaction.
    /// Returns None for inherited/scoped values that need an explicit composition edit.
    pub fn authored_component(
        &self,
        node: &NodeAddress,
        key: &str,
        policies: &ComponentPolicies,
        types: &TypeRegistry,
        loader: &mut dyn LoadFromPath,
    ) -> Result<Option<AuthoredValue>> {
        let component = self
            .graph
            .nodes
            .get(node)
            .and_then(|node| node.components.get(key))
            .ok_or_else(|| failure("component missing"))?;
        if !component.origin.scope.is_empty()
            || component
                .fields
                .iter()
                .any(|field| !field.origin.scope.is_empty())
        {
            return Ok(None);
        }
        let (mut value, references) =
            self.authored_value(key, &component.data, &[], policies, types, loader)?;
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
        for field in &component.fields {
            policies.policy(key, field.version)?;
            let target = field
                .path
                .as_str()
                .reflect_element_mut(value.as_mut())
                .map_err(|error| failure(error.to_string()))?;
            let info = target
                .get_represented_type_info()
                .ok_or_else(|| failure("field type missing"))?;
            let registration = types
                .get(info.type_id())
                .ok_or_else(|| failure("field type unregistered"))?;
            let mut context = ReferenceContext {
                entities: &entities,
                ports: &ports,
                prefix: &[],
                loader,
            };
            let mut de = ron::Deserializer::from_str(field.value.get_ron())?;
            let patch = TypedReflectDeserializer::with_processor(registration, types, &mut context)
                .deserialize(&mut de)?;
            de.end()?;
            target
                .try_apply(patch.as_ref())
                .map_err(|error| failure(error.to_string()))?;
        }
        Ok(Some((value, references)))
    }
    /// Compile an edited graph against this snapshot's existing dependency set.
    /// Introducing an undeclared asset dependency fails rather than inventing a loader.
    pub fn recompile(
        &self,
        graph: ResolvedGraph,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<Self> {
        let presence = self
            .ports
            .iter()
            .map(|(name, entity)| (name.clone(), entity.is_some()))
            .collect();
        self.recompile_with_presence(graph, types, policies, &presence)
    }
    /// Reconstruct source-editing values with symbolic root ports, including absent
    /// optional bindings. Explicit None and unbound nested ports remain absent.
    /// Uses only this snapshot's existing native asset handles; errors leave it unchanged.
    /// This source-only projection must not replace a runtime presence contract.
    pub fn symbolic_source(
        &self,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<Self> {
        let presence = self.ports.keys().map(|name| (name.clone(), true)).collect();
        self.recompile_with_presence(self.graph.clone(), types, policies, &presence)
    }
    fn recompile_with_presence(
        &self,
        graph: ResolvedGraph,
        types: &TypeRegistry,
        policies: &ComponentPolicies,
        presence: &BTreeMap<String, bool>,
    ) -> Result<Self> {
        struct Existing<'a> {
            paths: &'a [(AssetPath<'static>, UntypedHandle)],
            missing: bool,
        }
        impl LoadFromPath for Existing<'_> {
            fn load_from_path_erased(
                &mut self,
                type_id: std::any::TypeId,
                path: AssetPath<'static>,
            ) -> UntypedHandle {
                if let Some((_, handle)) = self
                    .paths
                    .iter()
                    .find(|(requested, handle)| handle.type_id() == type_id && requested == &path)
                {
                    return handle.clone();
                }
                self.missing = true;
                UntypedHandle::Uuid {
                    type_id,
                    uuid: uuid::Uuid::new_v4(),
                }
            }
        }
        let mut loader = Existing {
            paths: &self.dependency_paths,
            missing: false,
        };
        let compiled = Self::compile(graph, types, policies, &mut loader, presence)?;
        if loader.missing {
            return Err(failure("Apply introduced an untracked asset dependency"));
        }
        Ok(compiled)
    }
    /// Serialize an edited value from this isolated snapshot using its symbolic refs.
    pub fn encode_value(
        &self,
        value: &dyn bevy::reflect::PartialReflect,
        policies: &ComponentPolicies,
        types: &TypeRegistry,
    ) -> Result<Box<ron::value::RawValue>> {
        let mut reverse: BTreeMap<_, _> = self
            .entities
            .iter()
            .map(|(address, entity)| (*entity, Reference::Local(address.clone())))
            .collect();
        reverse.extend(self.ports.iter().filter_map(|(name, entity)| {
            entity.map(|entity| (entity, Reference::Port(name.clone())))
        }));
        policies.encode_value_paths(value, types, &reverse, &self.dependency_paths)
    }
    /// Preflight dependency/port readiness, then instantiate through Bevy's entity mapping.
    /// Failure removes owned allocations; external parent/port targets remain untouched.
    /// On success the returned root owns the new hierarchy and emits PrefabReady.
    pub fn instantiate(
        self: &std::sync::Arc<Self>,
        world: &mut World,
        parent: Option<Entity>,
        bindings: &BTreeMap<String, Option<Entity>>,
    ) -> Result<Entity> {
        if self.has_dependencies() {
            let server = world
                .get_resource::<AssetServer>()
                .ok_or_else(|| failure("AssetServer required for prefab dependencies"))?;
            if !self.asset_status(server)? {
                return Err(failure("prefab dependencies are not ready"));
            }
        }
        if parent.is_some_and(|p| !world.entities().contains(p)) {
            return Err(failure("instance parent is not live"));
        }
        let registry = world
            .get_resource::<AppTypeRegistry>()
            .cloned()
            .ok_or_else(|| failure("AppTypeRegistry required"))?;
        let types = registry.read();
        // Preflight refs before allocating anything in the live world.
        let mut mapping = EntityHashMap::default();
        for (name, source) in &self.ports {
            match (source, bindings.get(name).copied().flatten()) {
                (Some(source), Some(target)) if world.entities().contains(target) => {
                    mapping.insert(*source, target);
                }
                (None, None) => {}
                _ => {
                    return Err(failure(format!(
                        "port {name}: binding missing or presence differs from compiled contract"
                    )));
                }
            }
        }
        if bindings.keys().any(|name| !self.ports.contains_key(name)) {
            return Err(failure("unknown instance binding"));
        }
        let mut allocated = Vec::new();
        for source in self.entities.values() {
            let entity = world.spawn_empty().id();
            allocated.push(entity);
            mapping.insert(*source, entity);
        }
        if let Err(error) = self
            .dynamic
            .write_to_world_with(world, &mut mapping, &types)
        {
            for entity in allocated {
                if let Ok(entity) = world.get_entity_mut(entity) {
                    entity.despawn();
                }
            }
            return Err(failure(error.to_string()));
        }
        let id = InstanceId::new();
        let nodes: BTreeMap<_, _> = self
            .entities
            .iter()
            .map(|(address, source)| (address.clone(), mapping[source]))
            .collect();
        for (address, entity) in &nodes {
            world.entity_mut(*entity).insert(PrefabNode {
                instance: id,
                address: address.clone(),
            });
        }
        let root = nodes[&self.graph.root];
        if let Some(parent) = parent {
            world.entity_mut(root).insert(ChildOf(parent));
        }
        world.entity_mut(root).insert(PrefabInstance {
            id,
            document: self.graph.document,
            nodes,
            source: self.clone(),
        });
        world.trigger(PrefabReady {
            entity: root,
            instance: id,
        });
        Ok(root)
    }
    /// Encode a component with an explicit authoring identity remap, for unpack/edit operations.
    /// This never changes or serializes a live instance.
    pub fn component(
        &self,
        node: &NodeAddress,
        key: &str,
        policies: &ComponentPolicies,
        types: &TypeRegistry,
        remap: &BTreeMap<NodeAddress, NodeAddress>,
    ) -> Result<ComponentData> {
        let entity = *self
            .entities
            .get(node)
            .ok_or_else(|| failure("node missing"))?;
        let policy = policies
            .0
            .get(key)
            .ok_or_else(|| failure("unknown component"))?;
        let reflect = types
            .get_type_data::<ReflectComponent>(policy.type_id)
            .ok_or_else(|| failure("component is not reflected"))?;
        let value = reflect
            .reflect(self.authoring.entity(entity))
            .ok_or_else(|| failure("component missing"))?;
        let mut reverse: BTreeMap<_, _> = self
            .entities
            .iter()
            .map(|(a, e)| (*e, Reference::Local(remap.get(a).unwrap_or(a).clone())))
            .collect();
        reverse.extend(
            self.ports
                .iter()
                .filter_map(|(name, entity)| entity.map(|e| (e, Reference::Port(name.clone())))),
        );
        policies.encode(key, value, types, &reverse)
    }
}

fn collect_document_links(
    value: &dyn bevy::reflect::PartialReflect,
    links: &mut BTreeSet<DocumentId>,
) {
    use bevy::reflect::ReflectRef;
    if value
        .get_represented_type_info()
        .is_some_and(|info| info.type_id() == std::any::TypeId::of::<DocumentId>())
    {
        if let Some(id) = DocumentId::from_reflect(value) {
            links.insert(id);
        }
        return;
    }
    let children: Vec<&dyn bevy::reflect::PartialReflect> = match value.reflect_ref() {
        ReflectRef::Struct(value) => (0..value.field_len())
            .filter_map(|index| value.field_at(index))
            .collect(),
        ReflectRef::TupleStruct(value) => (0..value.field_len())
            .filter_map(|index| value.field(index))
            .collect(),
        ReflectRef::Tuple(value) => (0..value.field_len())
            .filter_map(|index| value.field(index))
            .collect(),
        ReflectRef::Enum(value) => (0..value.field_len())
            .filter_map(|index| value.field_at(index))
            .collect(),
        ReflectRef::List(value) => value.iter().collect(),
        ReflectRef::Array(value) => value.iter().collect(),
        ReflectRef::Map(value) => value.iter().flat_map(|(key, value)| [key, value]).collect(),
        ReflectRef::Set(value) => value.iter().collect(),
        _ => Vec::new(),
    };
    for child in children {
        collect_document_links(child, links);
    }
}

struct TrackingLoader<'a> {
    inner: &'a mut dyn LoadFromPath,
    dependencies: Vec<UntypedHandle>,
    paths: Vec<(AssetPath<'static>, UntypedHandle)>,
}
impl LoadFromPath for TrackingLoader<'_> {
    fn load_from_path_erased(
        &mut self,
        type_id: std::any::TypeId,
        path: AssetPath<'static>,
    ) -> UntypedHandle {
        let handle = self.inner.load_from_path_erased(type_id, path.clone());
        if !self
            .paths
            .iter()
            .any(|(requested, existing)| *requested == path && existing.type_id() == type_id)
        {
            self.paths.push((path, handle.clone()));
        }
        if !self.dependencies.iter().any(|h| h.id() == handle.id()) {
            self.dependencies.push(handle.clone());
        }
        handle
    }
}
fn ports_for(
    graph: &ResolvedGraph,
    scope: &[InstanceId],
    root: &BTreeMap<String, Option<Entity>>,
    entities: &BTreeMap<NodeAddress, Entity>,
) -> Result<BTreeMap<String, Option<Entity>>> {
    let names: BTreeSet<_> = if scope.is_empty() {
        graph.ports.keys().cloned().collect()
    } else {
        graph
            .bindings
            .keys()
            .filter(|(s, _)| s == scope)
            .map(|(_, name)| name.clone())
            .collect()
    };
    names
        .into_iter()
        .map(|name| {
            graph
                .port(scope, &name, root, entities, &mut BTreeSet::new())
                .map(|entity| (name, entity))
        })
        .collect()
}
