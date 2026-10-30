//! Object presets stage ordinary source nodes/components in one shared transaction.
use super::*;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(crate) enum ObjectKind {
    Empty,
    Cube,
    Sphere,
    Plane,
    Sun,
    Camera,
}

impl Documents {
    pub(crate) fn create_child(
        &mut self,
        kind: ObjectKind,
        target: &inspection::Target,
        types: &bevy::reflect::TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        if self.creating() || self.opening() || self.save_busy() || self.save_pending() {
            return Err(
                "Wait for the current document operation before creating an object.".into(),
            );
        }
        self.inspect(target)?;
        let id = target.document;
        let opened = self
            .opened
            .get(&id)
            .ok_or("Active document view unavailable")?;
        let parent = target.node.clone();
        if opened
            .graph
            .nodes
            .get(&parent)
            .is_some_and(|node| node.components.contains_key("ui.layout"))
        {
            return Err("Choose a spatial parent for a 3D object.".into());
        }
        let scope = self.scope.as_ref().ok_or("Source workspace unavailable")?;
        let path = format!(
            "{}://{}",
            scope.source.namespace,
            scope.path(id).ok_or("Document path unavailable")?
        );
        let title = match kind {
            ObjectKind::Empty => "Object",
            ObjectKind::Cube => "Cube",
            ObjectKind::Sphere => "Sphere",
            ObjectKind::Plane => "Plane",
            ObjectKind::Sun => "Directional light",
            ObjectKind::Camera => "Camera",
        };
        let mut edit = SourceEdit::begin(
            scope,
            &format!("Create {title}"),
            std::slice::from_ref(&path),
        )?;
        let parent = edit
            .nodes(scope, &path)?
            .into_iter()
            .find(|node| node.address == parent)
            .ok_or("Selected parent was removed")?;
        let node = edit.add_node(scope, &parent, title, types, policies)?;
        let mut component = |key: &str| -> Result<(), String> {
            let value: Box<dyn PartialReflect> = match (kind, key) {
                (ObjectKind::Sun, "scene.transform") => Box::new(Transform::from_rotation(
                    Quat::from_euler(EulerRot::YXZ, -0.6, -0.8, 0.),
                )),
                (ObjectKind::Camera, "scene.transform") => {
                    Box::new(Transform::from_xyz(0., 2., 5.).looking_at(Vec3::ZERO, Vec3::Y))
                }
                (ObjectKind::Sphere, "scene.primitive") => {
                    Box::new(kedra_scene::spatial::Primitive::Sphere { radius: 0.5 })
                }
                (ObjectKind::Plane, "scene.primitive") => {
                    Box::new(kedra_scene::spatial::Primitive::Cuboid {
                        size: Vec3::new(10., 0.01, 10.),
                    })
                }
                _ => super::components::constructor(key, types, policies)
                    .ok_or("Component constructor unavailable")?,
            };
            let data = policies
                .encode(key, value.as_ref(), types, &BTreeMap::new())
                .map_err(|error| error.to_string())?;
            edit.add_component(scope, &node, key, data, types, policies)
        };
        component("scene.transform")?;
        match kind {
            ObjectKind::Empty => {}
            ObjectKind::Cube | ObjectKind::Sphere | ObjectKind::Plane => {
                component("scene.primitive")?;
                component("scene.surface")?;
            }
            ObjectKind::Sun => {
                component("scene.sun")?;
            }
            ObjectKind::Camera => component("scene.camera")?,
        }
        self.commit_edit(&mut edit, types, policies)?;
        self.select(id, node.address);
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct NodeCommand {
    pub target: inspection::Target,
    pub expected: BTreeMap<DocumentId, u64>,
    pub nodes: Vec<NodeAddress>,
    pub operation: NodeOperation,
}
#[derive(Clone)]
pub(crate) enum NodeOperation {
    Create(ObjectKind),
    Rename(String),
    Delete,
    Reparent,
}

impl Documents {
    pub(crate) fn edit_nodes(
        &mut self,
        command: &NodeCommand,
        types: &bevy::reflect::TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        if self.revisions() != command.expected
            || self.active != Some(command.target.document)
            || self.creating()
            || self.opening()
            || self.save_busy()
            || self.save_pending()
        {
            return Err("Node command is stale or a document operation is pending.".into());
        }
        self.inspect(&command.target)?;
        if let NodeOperation::Create(kind) = command.operation {
            return self.create_child(kind, &command.target, types, policies);
        }
        let scope = self.scope.as_ref().ok_or("Source workspace unavailable")?;
        let id = command.target.document;
        let path = format!(
            "{}://{}",
            scope.source.namespace,
            scope.path(id).ok_or("Document path unavailable")?
        );
        let title = match command.operation {
            NodeOperation::Rename(_) => "Rename node",
            NodeOperation::Delete => "Delete nodes",
            _ => "Reparent nodes",
        };
        let mut edit = SourceEdit::begin(scope, title, std::slice::from_ref(&path))?;
        let available = edit.nodes(scope, &path)?;
        let find = |address: &NodeAddress| {
            available
                .iter()
                .find(|node| &node.address == address)
                .ok_or("Node was removed")
        };
        let graph = &self.opened.get(&id).ok_or("Document unavailable")?.graph;
        let nodes: Vec<_> = command
            .nodes
            .iter()
            .filter(|address| {
                let mut parent = graph
                    .nodes
                    .get(*address)
                    .and_then(|node| node.parent.as_ref());
                while let Some(ancestor) = parent {
                    if command.nodes.contains(ancestor) {
                        return false;
                    }
                    parent = graph
                        .nodes
                        .get(ancestor)
                        .and_then(|node| node.parent.as_ref());
                }
                true
            })
            .collect();
        if nodes.is_empty() {
            return Err("Select a node first.".into());
        }
        for address in nodes {
            let node = find(address)?;
            match &command.operation {
                NodeOperation::Rename(name) => {
                    if command.nodes.len() != 1 || name.chars().any(char::is_control) {
                        return Err("Enter one valid node name.".into());
                    }
                    edit.rename(scope, node, name, types, policies)?;
                }
                NodeOperation::Delete => edit.remove_node(scope, node, types, policies)?,
                NodeOperation::Reparent => {
                    let parent = find(&command.target.node)?;
                    if graph.nodes[&parent.address]
                        .components
                        .contains_key("ui.layout")
                        && !graph.nodes[&node.address]
                            .components
                            .contains_key("ui.layout")
                    {
                        return Err("Spatial nodes cannot descend from a UI layout.".into());
                    }
                    let compiled = self
                        .compiled
                        .get(&id)
                        .ok_or("Source compilation unavailable")?;
                    let transform = reparent_transform(compiled, address, &parent.address)?;
                    edit.reparent(scope, node, parent, types, policies)?;
                    if let Some(transform) = transform {
                        if !graph.nodes[&node.address]
                            .components
                            .contains_key("scene.transform")
                        {
                            let data = policies
                                .encode("scene.transform", &transform, types, &BTreeMap::new())
                                .map_err(|error| error.to_string())?;
                            edit.add_component(
                                scope,
                                node,
                                "scene.transform",
                                data,
                                types,
                                policies,
                            )?;
                            continue;
                        }
                        for (path, value) in [
                            ("translation", serde_json::to_value(transform.translation)),
                            ("rotation", serde_json::to_value(transform.rotation)),
                            ("scale", serde_json::to_value(transform.scale)),
                        ] {
                            edit.set_property(
                                scope,
                                kedra_authoring::editing::SourceProperty {
                                    node,
                                    component: "scene.transform",
                                    path,
                                    value: value.map_err(|e| e.to_string())?,
                                },
                                types,
                                policies,
                            )?;
                        }
                    }
                }
                NodeOperation::Create(_) => unreachable!(),
            }
        }
        let fallback = matches!(command.operation, NodeOperation::Delete).then(|| {
            graph
                .nodes
                .get(&command.target.node)
                .and_then(|node| node.parent.clone())
                .unwrap_or_else(|| graph.root.clone())
        });
        self.commit_edit(&mut edit, types, policies)?;
        if let Some(fallback) = fallback {
            let fallback = if self.opened[&id].graph.nodes.contains_key(&fallback) {
                fallback
            } else {
                self.opened[&id].graph.root.clone()
            };
            self.select(id, fallback);
        }
        Ok(())
    }
}

fn reparent_transform(
    compiled: &kedra_scene::instance::CompiledPrefab,
    node: &NodeAddress,
    parent: &NodeAddress,
) -> Result<Option<Transform>, String> {
    if compiled.graph().nodes[node]
        .components
        .contains_key("ui.layout")
    {
        return Ok(None);
    }
    let parent = global(compiled, parent);
    local_transform(parent, global(compiled, node)).map(Some)
}
pub(super) fn placement_transform(
    compiled: &kedra_scene::instance::CompiledPrefab,
    parent: &NodeAddress,
    position: Vec3,
) -> Result<Transform, String> {
    local_transform(global(compiled, parent), Mat4::from_translation(position))
}
fn global(compiled: &kedra_scene::instance::CompiledPrefab, address: &NodeAddress) -> Mat4 {
    let world = compiled.authoring_world();
    let entities = compiled.source_entities();
    let mut matrix = Mat4::IDENTITY;
    let mut current = Some(address);
    while let Some(address) = current {
        if let Some(transform) = entities
            .get(address)
            .and_then(|entity| world.get::<Transform>(*entity))
        {
            matrix = transform.to_matrix() * matrix;
        }
        current = compiled
            .graph()
            .nodes
            .get(address)
            .and_then(|node| node.parent.as_ref());
    }
    matrix
}
fn local_transform(parent: Mat4, global: Mat4) -> Result<Transform, String> {
    if parent.determinant().abs() < 1e-8 {
        return Err("Cannot preserve world transform under a zero-scale parent.".into());
    }
    let local = parent.inverse() * global;
    let transform = Transform::from_matrix(local);
    if !transform.to_matrix().abs_diff_eq(local, 0.001) {
        return Err(
            "Cannot preserve world transform: the new parent would introduce shear.".into(),
        );
    }
    Ok(transform)
}
