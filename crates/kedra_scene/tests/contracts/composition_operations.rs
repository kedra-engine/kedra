//! Composition operations reject conflicting edits without changing source authority.
use kedra_scene::{composition::Catalog, document::*};
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn fixture() -> Result<(PrefabDocument, NodeId, NodeId)> {
    let mut document = PrefabDocument::new("Operations");
    document.nodes[0]
        .components
        .insert("data".into(), ComponentData::ron(1, "(value:1)")?);
    let a = NodeId::new();
    let b = NodeId::new();
    for id in [a, b] {
        document.nodes.push(kedra_scene::document::Node {
            id,
            name: id.to_string(),
            parent: Some(document.root.into()),
            components: BTreeMap::new(),
        });
    }
    Ok((document, a, b))
}

#[test]
fn invalid_operation_matrix_preserves_the_exact_source() -> Result<()> {
    let (source, a, b) = fixture()?;
    let root = NodeAddress::from(source.root);
    let alias = NodeId::new();
    let field = |path: &str| -> Result<Operation> {
        Ok(Operation::SetField {
            node: root.clone(),
            component: "data".into(),
            version: 1,
            path: path.into(),
            value: ComponentData::ron(1, "2")?.value,
        })
    };
    let whole = Operation::SetComponent {
        node: root.clone(),
        component: "data".into(),
        data: ComponentData::ron(1, "(value:2)")?,
    };
    let cases = vec![
        vec![Operation::RemoveNode(root.clone())],
        vec![Operation::RemoveNode(NodeId::new().into())],
        vec![Operation::ReparentNode {
            node: root.clone(),
            parent: a.into(),
        }],
        vec![Operation::ReparentNode {
            node: a.into(),
            parent: a.into(),
        }],
        vec![Operation::ReparentNode {
            node: a.into(),
            parent: NodeId::new().into(),
        }],
        vec![Operation::ReparentNode {
            node: a.into(),
            parent: NodeAddress {
                instances: vec![InstanceId::new()],
                node: b,
            },
        }],
        vec![Operation::RemoveComponent {
            node: root.clone(),
            component: "missing".into(),
        }],
        vec![Operation::SetField {
            node: a.into(),
            component: "data".into(),
            version: 1,
            path: "value".into(),
            value: ComponentData::ron(1, "2")?.value,
        }],
        vec![field("")?],
        vec![field("value[0]")?],
        vec![field("#0")?],
        vec![field("value")?, field("value")?],
        vec![field("value")?, field("value.child")?],
        vec![field("value.child")?, field("value")?],
        vec![whole.clone(), whole.clone()],
        vec![field("value")?, whole.clone()],
        vec![whole, field("value")?],
        vec![Operation::SetChildOrder {
            node: root.clone(),
            children: vec![a.into(), a.into()],
        }],
        vec![Operation::SetChildOrder {
            node: root.clone(),
            children: vec![a.into()],
        }],
        vec![Operation::SetChildOrder {
            node: a.into(),
            children: vec![b.into()],
        }],
        vec![Operation::RemoveNestedInstanceAt(vec![])],
        vec![Operation::RemoveNestedInstance(InstanceId::new())],
        vec![Operation::SetBinding {
            instance: InstanceId::new(),
            port: "unknown".into(),
            target: Reference::Local(root.clone()),
        }],
        vec![Operation::RemapReferences(BTreeMap::from([(
            root.clone(),
            a.into(),
        )]))],
        vec![Operation::RemapReferences(BTreeMap::from([(
            alias.into(),
            NodeId::new().into(),
        )]))],
        vec![Operation::RemapReferences(BTreeMap::from([(
            alias.into(),
            alias.into(),
        )]))],
        vec![
            Operation::RemapReferences(BTreeMap::from([(alias.into(), a.into())])),
            Operation::RemapReferences(BTreeMap::from([(alias.into(), b.into())])),
        ],
        vec![
            Operation::RemapReferences(BTreeMap::from([(alias.into(), a.into())])),
            Operation::ReparentNode {
                node: b.into(),
                parent: alias.into(),
            },
        ],
    ];
    for (index, operations) in cases.into_iter().enumerate() {
        let mut candidate = source.clone();
        candidate.overrides = operations;
        let mut catalog = Catalog::default();
        catalog.insert(candidate.clone())?;
        assert!(
            catalog.resolve(candidate.id).is_err(),
            "invalid operation case {index} admitted"
        );
        assert_eq!(catalog.documents[&candidate.id], candidate);
    }
    Ok(())
}

#[test]
fn ordered_reparent_rename_and_component_removal_are_source_only_edits() -> Result<()> {
    let (mut source, a, b) = fixture()?;
    let alias = NodeId::new();
    source.overrides = vec![
        Operation::RemapReferences(BTreeMap::from([(alias.into(), a.into())])),
        Operation::RenameNode {
            node: alias.into(),
            name: "Renamed".into(),
        },
        Operation::ReparentNode {
            node: b.into(),
            parent: a.into(),
        },
        Operation::SetChildOrder {
            node: source.root.into(),
            children: vec![a.into()],
        },
        Operation::SetChildOrder {
            node: a.into(),
            children: vec![b.into()],
        },
        Operation::RemoveComponent {
            node: source.root.into(),
            component: "data".into(),
        },
    ];
    let mut catalog = Catalog::default();
    catalog.insert(source.clone())?;
    let graph = catalog.resolve(source.id)?;
    assert_eq!(graph.nodes[&a.into()].name, "Renamed");
    assert_eq!(graph.nodes[&a.into()].children, vec![b.into()]);
    assert!(graph.nodes[&source.root.into()].components.is_empty());
    assert_eq!(catalog.documents[&source.id], source);
    Ok(())
}

#[test]
fn inheritance_and_nesting_contracts_reject_invalid_sources() -> Result<()> {
    let (base, _, _) = fixture()?;
    let mut catalog = Catalog::default();
    catalog.insert(base.clone())?;
    let mut variants = vec![];
    let mut wrong_root = base.variant("Changed root");
    wrong_root.root = NodeId::new();
    variants.push(wrong_root);
    let mut wrong_kind = base.variant("Changed kind");
    wrong_kind.kind = DocumentKind::Scene;
    variants.push(wrong_kind);
    let mut missing = base.variant("Missing base");
    missing.base = Some(DocumentId::new());
    variants.push(missing);
    let mut unsupported = PrefabDocument::new("Version");
    unsupported.version = 2;
    variants.push(unsupported);
    for document in variants {
        let mut candidate = catalog.clone();
        candidate.insert(document.clone())?;
        assert!(candidate.resolve(document.id).is_err());
    }
    let mut scene = PrefabDocument::new("Scene");
    scene.kind = DocumentKind::Scene;
    catalog.insert(scene.clone())?;
    for (asset, parent, duplicate) in [
        (scene.id, base.root, false),
        (DocumentId::new(), base.root, false),
        (base.id, NodeId::new(), false),
        (base.id, base.root, true),
    ] {
        let mut host = base.clone();
        let site = NestedInstance {
            id: InstanceId::new(),
            asset,
            parent: parent.into(),
            bindings: BTreeMap::new(),
            overrides: vec![],
        };
        host.instances.push(site.clone());
        if duplicate {
            host.instances.push(site);
        }
        let mut candidate = catalog.clone();
        candidate.documents.insert(host.id, host.clone());
        assert!(candidate.resolve(host.id).is_err());
    }
    assert!(!catalog.inherits(DocumentId::new(), base.id));
    Ok(())
}
