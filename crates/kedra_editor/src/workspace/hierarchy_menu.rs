//! Hierarchy content commands pin the clicked source and its admitted revisions.
use super::*;
use crate::documents::{
    ObjectKind,
    structure::{NodeCommand, NodeOperation},
};
use kedra_scene::document::NodeAddress;

#[derive(Clone)]
pub(super) struct Command {
    pub view: ViewId,
    pub edit: NodeCommand,
}

pub(crate) fn context(
    world: &mut World,
    binding: crate::hierarchy::Binding,
    anchor: Entity,
    position: Vec2,
) {
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        let result = (|| {
            if !inspector_visible(&state, binding.view)
                || binding.document != state.documents.active
            {
                return Ok(());
            }
            let id = binding.document.ok_or("Open a scene or prefab first.")?;
            let node = match binding.action {
                crate::hierarchy::Action::Select(node) | crate::hierarchy::Action::Toggle(node) => {
                    Some(node)
                }
                crate::hierarchy::Action::FocusSearch => None,
                _ => return Ok(()),
            };
            let root = state
                .documents
                .opened
                .get(&id)
                .ok_or("Document unavailable")?
                .graph
                .root
                .clone();
            let target = state
                .documents
                .target(id, node.clone().unwrap_or(root.clone()))
                .ok_or("Node unavailable")?;
            if let Some(node) = &node
                && !state.documents.selected_nodes(id).contains(node)
            {
                state.documents.select(id, node.clone());
            }
            let nodes = if node.is_some() {
                state.documents.selected_nodes(id)
            } else {
                vec![]
            };
            let make = |operation| {
                input::Action::Node(Command {
                    view: binding.view,
                    edit: NodeCommand {
                        target: target.clone(),
                        expected: state.documents.revisions(),
                        nodes: nodes.clone(),
                        operation,
                    },
                })
            };
            let mut paths = vec![
                (
                    "Create/Empty Node",
                    make(NodeOperation::Create(ObjectKind::Empty)),
                ),
                (
                    "Create/3D Object/Cube",
                    make(NodeOperation::Create(ObjectKind::Cube)),
                ),
                (
                    "Create/3D Object/Sphere",
                    make(NodeOperation::Create(ObjectKind::Sphere)),
                ),
                (
                    "Create/3D Object/Plane",
                    make(NodeOperation::Create(ObjectKind::Plane)),
                ),
                (
                    "Create/Light/Directional Light",
                    make(NodeOperation::Create(ObjectKind::Sun)),
                ),
                (
                    "Create/Camera",
                    make(NodeOperation::Create(ObjectKind::Camera)),
                ),
            ];
            if nodes.len() == 1 {
                paths.push(("Rename", make(NodeOperation::Rename(String::new()))));
            }
            if !nodes.is_empty() && !nodes.contains(&root) {
                paths.push(("Delete", make(NodeOperation::Delete)));
            }
            if state.documents.can_history(false) {
                paths.push(("Undo", input::Action::SourceHistory(false)));
            }
            if state.documents.can_history(true) {
                paths.push(("Redo", input::Action::SourceHistory(true)));
            }
            let entries = kedra_ui::popup::from_paths(paths)?;
            input::dismiss(world, &mut state);
            let point = popup::attach_at(world, anchor, position)?;
            state.menu.push(point.root);
            document_menu::populate(world, point.menu, "hierarchy-menu", entries)
        })();
        if let Err(error) = result {
            notice(world, &state, &error);
        }
    });
}
pub(super) fn dispatch(
    world: &mut World,
    state: &mut EditorWorkspace,
    command: Command,
) -> Result<(), String> {
    validate(state, &command)?;
    if matches!(command.edit.operation, NodeOperation::Rename(_)) {
        return document_create::open_node_rename(world, state, command);
    }
    source_history::edit_nodes(world, state, &command.edit)
}
pub(super) fn validate(state: &EditorWorkspace, command: &Command) -> Result<(), String> {
    if !inspector_visible(state, command.view)
        || state.documents.active != Some(command.edit.target.document)
        || state.documents.revisions() != command.edit.expected
    {
        return Err("Hierarchy command target changed.".into());
    }
    Ok(())
}

pub(crate) fn shortcut(world: &mut World, binding: crate::hierarchy::Binding, rename: bool) {
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        let result = (|| {
            let crate::hierarchy::Action::Select(node) = binding.action else {
                return Ok(());
            };
            let id = binding.document.ok_or("No source document")?;
            let target = state
                .documents
                .target(id, node.clone())
                .ok_or("Node unavailable")?;
            let nodes = state.documents.selected_nodes(id);
            let command = Command {
                view: binding.view,
                edit: NodeCommand {
                    target,
                    expected: state.documents.revisions(),
                    nodes: if nodes.contains(&node) {
                        nodes
                    } else {
                        vec![node]
                    },
                    operation: if rename {
                        NodeOperation::Rename(String::new())
                    } else {
                        NodeOperation::Delete
                    },
                },
            };
            if rename && command.edit.nodes.len() != 1 {
                return Err("Select one node to rename.".into());
            }
            dispatch(world, &mut state, command)
        })();
        if let Err(error) = result {
            notice(world, &state, &error);
        }
    });
}

pub(crate) fn reparent(world: &mut World, binding: crate::hierarchy::Binding, owner: Entity) {
    let Some(crate::drag::DragPayload::Nodes(nodes)) = crate::drag::payload(world, owner) else {
        return;
    };
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        let result = (|| {
            if !inspector_visible(&state, binding.view)
                || binding.document != state.documents.active
            {
                return Err("Hierarchy drop target changed.".into());
            }
            let id = binding.document.ok_or("No active document")?;
            let parent: NodeAddress = match binding.action {
                crate::hierarchy::Action::Select(node) => node,
                crate::hierarchy::Action::FocusSearch => state
                    .documents
                    .opened
                    .get(&id)
                    .ok_or("Document unavailable")?
                    .graph
                    .root
                    .clone(),
                _ => return Ok(()),
            };
            let target = state
                .documents
                .target(id, parent)
                .ok_or("Parent unavailable")?;
            let addresses = nodes
                .iter()
                .map(|node| {
                    if node.document != id {
                        return Err("Move nodes within the same document.");
                    }
                    Ok(node.node.clone())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let command = NodeCommand {
                target,
                expected: state.documents.revisions(),
                nodes: addresses,
                operation: NodeOperation::Reparent,
            };
            source_history::edit_nodes(world, &mut state, &command)
        })();
        if let Err(error) = result {
            notice(world, &state, &error);
        }
    });
}

pub(crate) fn drop_valid(
    world: &World,
    binding: &crate::hierarchy::Binding,
    owner: Entity,
) -> bool {
    let Some(state) = world.get_resource::<EditorWorkspace>() else {
        return false;
    };
    let Some(id) = binding
        .document
        .filter(|id| state.documents.active == Some(*id))
    else {
        return false;
    };
    if !inspector_visible(state, binding.view) {
        return false;
    }
    let Some(opened) = state.documents.opened.get(&id) else {
        return false;
    };
    let parent = match &binding.action {
        crate::hierarchy::Action::Select(node) => node,
        crate::hierarchy::Action::FocusSearch => &opened.graph.root,
        _ => return false,
    };
    let Some(crate::drag::DragPayload::Nodes(nodes)) = crate::drag::payload(world, owner) else {
        return false;
    };
    !nodes.is_empty()
        && nodes.iter().all(|node| {
            if node.document != id || node.node == opened.graph.root {
                return false;
            }
            let mut ancestor = Some(parent);
            while let Some(address) = ancestor {
                if *address == node.node {
                    return false;
                }
                ancestor = opened
                    .graph
                    .nodes
                    .get(address)
                    .and_then(|node| node.parent.as_ref());
            }
            true
        })
}
