use super::model::*;
use kedra_scene::{composition::Catalog, document::*};
use std::collections::BTreeMap;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn compact_viewport_keeps_rows_visible_at_twenty_two_pixel_pitch() -> TestResult {
    use bevy::prelude::*;
    let mut app = crate::workspace::browser_test_app()?;
    let host = app.world_mut().spawn(Node::default()).id();
    let host = crate::views::mount_asset(
        app.world_mut(),
        host,
        "editor://prefabs/window-frame.prefab.ron",
        "hierarchy-test",
    )?
    .root;
    let mut panel =
        super::Panel::mount(app.world_mut(), host, kedra_docking::model::ViewId::new())?;
    panel.visible = (0..30).map(|i| (i, false)).collect();
    let area = panel.element("entries")?;
    app.world_mut().entity_mut(area).insert((
        ComputedNode {
            size: Vec2::new(640., 88.),
            inverse_scale_factor: 0.5,
            ..default()
        },
        UiGlobalTransform::default(),
        ScrollPosition(Vec2::new(0., 220.)),
    ));
    assert_eq!(panel.viewport(app.world())?, (9, 14, 320));
    Ok(())
}
#[test]
fn maximum_graph_is_projected_without_a_visible_row_count_truncation() -> TestResult {
    let mut doc = PrefabDocument::new("Large tree");
    for _ in 1..4096 {
        doc.nodes.push(Node {
            id: NodeId::new(),
            name: "Node".into(),
            parent: Some(doc.root.into()),
            components: BTreeMap::new(),
        });
    }
    let id = doc.id;
    let mut catalog = Catalog::default();
    catalog.insert(doc)?;
    let graph = catalog.resolve(id)?;
    let tree = Tree::new(&graph);
    assert_eq!(tree.visible(&Navigation::default()).len(), 4096);
    assert_eq!(tree.indices.len(), 4096);
    Ok(())
}

#[test]
fn variants_and_explicit_child_order_use_effective_not_raw_source_nodes() -> TestResult {
    let mut doc = PrefabDocument::new("Base");
    let root = doc.root;
    let children = [NodeId::new(), NodeId::new()];
    for (index, id) in children.iter().enumerate() {
        doc.nodes.push(Node {
            id: *id,
            name: format!("Node {index}"),
            parent: Some(root.into()),
            components: BTreeMap::new(),
        });
    }
    let mut variant = doc.variant("Variant");
    variant.overrides.push(Operation::SetChildOrder {
        node: root.into(),
        children: children.iter().rev().map(|id| (*id).into()).collect(),
    });
    let id = variant.id;
    let mut catalog = Catalog::default();
    catalog.insert(doc)?;
    catalog.insert(variant)?;
    let tree = Tree::new(&catalog.resolve(id)?);
    assert_eq!(tree.rows[1].address.node, children[1]);
    assert_eq!(tree.rows[2].address.node, children[0]);
    Ok(())
}
fn fixture() -> Result<(Catalog, DocumentId), Box<dyn std::error::Error>> {
    let mut child = PrefabDocument::new("Child");
    child.nodes[0].name = "Shared root".into();
    child.nodes.push(Node {
        id: NodeId::new(),
        name: "Straße leaf".into(),
        parent: Some(child.root.into()),
        components: BTreeMap::new(),
    });
    let mut doc = PrefabDocument::new("Main");
    doc.nodes[0].name = "Root".into();
    for _ in 0..2 {
        doc.instances.push(NestedInstance {
            id: InstanceId::new(),
            asset: child.id,
            parent: doc.root.into(),
            bindings: BTreeMap::new(),
            overrides: Vec::new(),
        });
    }
    let root = doc.id;
    let mut catalog = Catalog::default();
    catalog.insert(child)?;
    catalog.insert(doc)?;
    Ok((catalog, root))
}
#[test]
fn composition_order_and_nested_instance_addresses_are_preserved() -> TestResult {
    let (catalog, id) = fixture()?;
    let graph = catalog.resolve(id)?;
    let before = graph.clone();
    let tree = Tree::new(&graph);
    assert_eq!(tree.rows.len(), 5);
    assert_eq!(tree.indices.len(), 5);
    assert_eq!(tree.rows[1].address.node, tree.rows[3].address.node);
    assert_ne!(
        tree.rows[1].address.instances,
        tree.rows[3].address.instances
    );
    assert!(tree.rows[1].boundary && tree.rows[3].boundary);
    assert!(!tree.rows[2].boundary);
    assert_eq!(tree.rows[0].address, graph.root);
    assert_eq!(
        vec![tree.rows[1].address.clone(), tree.rows[3].address.clone()],
        graph.nodes[&graph.root].children
    );
    assert_eq!(before, graph);
    Ok(())
}
#[test]
fn collapse_search_and_reveal_do_not_alias_hidden_selection() -> TestResult {
    let (catalog, id) = fixture()?;
    let graph = catalog.resolve(id)?;
    let tree = Tree::new(&graph);
    let selected = tree.rows[4].address.clone();
    let mut navigation = Navigation::default();
    navigation.collapsed.insert(graph.root.clone());
    assert_eq!(tree.visible(&navigation), vec![(0, false)]);
    navigation.filter = "STRASSE".into();
    assert_eq!(
        tree.visible(&navigation),
        vec![(0, true), (1, true), (2, false), (3, true), (4, false)]
    );
    navigation.filter = "no matches".into();
    assert!(tree.visible(&navigation).is_empty());
    tree.reveal(&mut navigation, &selected);
    assert!(navigation.filter.is_empty());
    assert_eq!(tree.visible(&navigation).len(), 5);
    assert_eq!(selected, tree.rows[4].address);
    Ok(())
}
#[test]
fn root_only_document_is_a_real_node_not_empty_content() -> TestResult {
    let doc = PrefabDocument::new("Empty scene");
    let id = doc.id;
    let mut catalog = Catalog::default();
    catalog.insert(doc)?;
    let tree = Tree::new(&catalog.resolve(id)?);
    assert_eq!(tree.visible(&Navigation::default()), vec![(0, false)]);
    assert!(!tree.rows[0].children);
    Ok(())
}
