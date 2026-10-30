//! Source-owned hierarchy edits: atomic failures, world pose and shared history.
use super::*;
use kedra_scene::document::{Node, NodeId, PrefabDocument};

#[test]
fn node_commands_preserve_pose_and_history_and_reject_stale_cycles_and_root_deletion()
-> Result<(), Box<dyn std::error::Error>> {
    let mut types = bevy::reflect::TypeRegistry::default();
    let mut policies = ComponentPolicies::default();
    kedra_scene::spatial::register(&mut types, &mut policies)?;
    let mut source = PrefabDocument::new("Hierarchy");
    let a = NodeId::new();
    let b = NodeId::new();
    let zero = NodeId::new();
    let skew = NodeId::new();
    let group = NodeId::new();
    source.nodes.push(Node {
        id: group,
        name: "Transformless group".into(),
        parent: Some(source.root.into()),
        components: BTreeMap::new(),
    });
    for (id, name, transform) in [
        (a, "First", Transform::from_xyz(2., 0., 0.)),
        (b, "Second", Transform::from_xyz(10., 0., 0.)),
        (zero, "Zero scale", Transform::from_scale(Vec3::ZERO)),
        (
            skew,
            "Nonuniform rotated parent",
            Transform::from_rotation(Quat::from_rotation_z(0.6)).with_scale(Vec3::new(2., 1., 1.)),
        ),
    ] {
        source.nodes.push(Node {
            id,
            name: name.into(),
            parent: Some(source.root.into()),
            components: BTreeMap::from([(
                "scene.transform".into(),
                policies.encode("scene.transform", &transform, &types, &BTreeMap::new())?,
            )]),
        });
    }
    let bytes = source.serialize()?;
    let snapshot = kedra_authoring::persistence::SourceSnapshot {
        environments: Default::default(),
        namespace: "game".into(),
        generation: 1,
        documents: vec![kedra_authoring::persistence::SourceFile {
            path: "Tree.prefab.ron".into(),
            hash: kedra_assets::delivery::digest(bytes.as_bytes()),
            source: bytes.clone(),
        }],
        assets: vec![],
        index: None,
    };
    let (scope, compiled) = kedra_authoring::source::admit(snapshot, &types, &policies)?;
    let graph = std::sync::Arc::new(compiled[&source.id].graph().clone());
    let mut documents = Documents {
        scope: Some(scope),
        compiled,
        ..default()
    };
    documents.refresh_inspection(&types, &policies)?;
    documents.published(
        Ready {
            document: source.id,
            path: "Tree.prefab.ron".into(),
            graph,
        },
        ViewId::new(),
        source.root.into(),
    );
    let command = |documents: &Documents,
                   target: NodeAddress,
                   nodes: Vec<NodeAddress>,
                   operation|
     -> Result<NodeCommand, String> {
        Ok(NodeCommand {
            target: documents
                .target(source.id, target)
                .ok_or("Target unavailable")?,
            expected: documents.revisions(),
            nodes,
            operation,
        })
    };
    let move_group = command(
        &documents,
        b.into(),
        vec![group.into()],
        NodeOperation::Reparent,
    )?;
    documents.edit_nodes(&move_group, &types, &policies)?;
    let compiled = &documents.compiled[&source.id];
    assert_eq!(
        compiled
            .authoring_world()
            .get::<Transform>(compiled.source_entities()[&group.into()])
            .ok_or("Group transform unavailable")?
            .translation,
        Vec3::new(-10., 0., 0.)
    );
    let move_node = command(
        &documents,
        b.into(),
        vec![a.into()],
        NodeOperation::Reparent,
    )?;
    documents.edit_nodes(&move_node, &types, &policies)?;
    let compiled = &documents.compiled[&source.id];
    let transform = compiled
        .authoring_world()
        .get::<Transform>(compiled.source_entities()[&a.into()])
        .ok_or("Transform unavailable")?;
    assert!(
        transform
            .translation
            .abs_diff_eq(Vec3::new(-8., 0., 0.), 0.001)
    );
    assert_eq!(compiled.graph().nodes[&a.into()].parent, Some(b.into()));
    let before = documents.revisions();
    assert!(documents.edit_nodes(&move_node, &types, &policies).is_err());
    let cycle = command(
        &documents,
        a.into(),
        vec![b.into()],
        NodeOperation::Reparent,
    )?;
    assert!(documents.edit_nodes(&cycle, &types, &policies).is_err());
    for parent in [zero, skew] {
        let rejected = command(
            &documents,
            parent.into(),
            vec![a.into()],
            NodeOperation::Reparent,
        )?;
        assert!(documents.edit_nodes(&rejected, &types, &policies).is_err());
    }
    let root = command(
        &documents,
        source.root.into(),
        vec![source.root.into()],
        NodeOperation::Delete,
    )?;
    assert!(documents.edit_nodes(&root, &types, &policies).is_err());
    let invalid = command(
        &documents,
        a.into(),
        vec![a.into()],
        NodeOperation::Rename("\n".into()),
    )?;
    assert!(documents.edit_nodes(&invalid, &types, &policies).is_err());
    assert_eq!(documents.revisions(), before);
    let rename = command(
        &documents,
        a.into(),
        vec![a.into()],
        NodeOperation::Rename("Камера 日本語".into()),
    )?;
    documents.edit_nodes(&rename, &types, &policies)?;
    assert_eq!(
        documents.opened[&source.id].graph.nodes[&a.into()].name,
        "Камера 日本語"
    );
    let remove = command(
        &documents,
        b.into(),
        vec![b.into(), a.into()],
        NodeOperation::Delete,
    )?;
    documents.edit_nodes(&remove, &types, &policies)?;
    assert!(
        !documents.opened[&source.id]
            .graph
            .nodes
            .contains_key(&a.into())
    );
    let undo = documents.prepare_history(false, &types, &policies)?;
    documents.publish_change(undo);
    assert_eq!(
        documents.opened[&source.id].graph.nodes[&a.into()].name,
        "Камера 日本語"
    );
    let redo = documents.prepare_history(true, &types, &policies)?;
    documents.publish_change(redo);
    assert!(
        !documents.opened[&source.id]
            .graph
            .nodes
            .contains_key(&b.into())
    );
    assert_eq!(
        documents
            .scope
            .as_ref()
            .ok_or("Scope missing")?
            .source
            .documents[0]
            .source,
        bytes,
        "Edits cannot publish source bytes before Save"
    );
    Ok(())
}
