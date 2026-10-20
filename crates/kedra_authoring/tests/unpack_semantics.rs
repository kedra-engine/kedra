//! Unpack preserves authored reference intent and effective hierarchy order.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated composition fixture assertions"
)]
mod common;
use bevy::prelude::*;
use common::{Links, Result, setup};
use kedra_authoring::{overrides::ApplyField, unpack::Unpack, workspace::WorkingSet};
use kedra_scene::{assets::compile_catalog, document::*, instance::CompiledPrefab};
use std::collections::BTreeMap;

#[test]
fn apply_preserves_optional_symbolic_ports_for_field_and_component_operations() -> Result<()> {
    for whole_component in [true, false] {
        let (app, policies) = setup()?;
        let mut base = PrefabDocument::new("Base");
        base.ports.insert("optional".into(), false);
        base.nodes[0]
            .components
            .insert("links".into(), ComponentData::ron(1, "(optional:None)")?);
        let mut variant = base.variant("Variant");
        variant.overrides.push(if whole_component {
            Operation::SetComponent {
                node: base.root.into(),
                component: "links".into(),
                data: ComponentData::ron(1, "(optional:Some(Port(\"optional\")))")?,
            }
        } else {
            Operation::SetField {
                node: base.root.into(),
                component: "links".into(),
                version: 1,
                path: "optional".into(),
                value: ComponentData::ron(1, "Some(Port(\"optional\"))")?.value,
            }
        });
        let mut working = WorkingSet::default();
        working.open(base.clone())?;
        working.open(variant.clone())?;
        let types = app.world().resource::<AppTypeRegistry>();
        let mut loader = app.world().resource::<AssetServer>();
        let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
        let request = ApplyField {
            origin: variant.id,
            target: base.id,
            node: base.root.into(),
            component: "links".into(),
            path: if whole_component { "" } else { "optional" }.into(),
            expected: BTreeMap::from([(base.id, 1), (variant.id, 1)]),
        };
        if whole_component {
            working.apply_component(
                request,
                &snapshots[&variant.id],
                &policies,
                &types.read(),
                |_| Ok(()),
            )?;
        } else {
            working.apply_field(
                request,
                &snapshots[&variant.id],
                &policies,
                &types.read(),
                &mut loader,
                |_| Ok(()),
            )?;
        }
        assert!(
            working.documents()[&variant.id]
                .document
                .overrides
                .is_empty()
        );
        for id in [base.id, variant.id] {
            let compiled = CompiledPrefab::compile(
                working.catalog().resolve(id)?,
                &types.read(),
                &policies,
                &mut loader,
                &BTreeMap::from([("optional".into(), true)]),
            )?;
            assert!(
                compiled
                    .authoring_world()
                    .get::<Links>(compiled.source_entities()[&base.root.into()])
                    .unwrap()
                    .optional
                    .is_some()
            );
        }
    }
    Ok(())
}

#[test]
fn unpack_preserves_transitive_optional_ports_local_links_and_explicit_absence() -> Result<()> {
    let (app, policies) = setup()?;
    let mut child = PrefabDocument::new("Child");
    child.ports.insert("owner".into(), false);
    child.ports.insert("unbound".into(), false);
    let mut nodes = Vec::new();
    for value in [
        "Some(Port(\"owner\"))".to_owned(),
        "None".to_owned(),
        format!(
            "Some(Local((instances:[],node:{:?})))",
            child.root.to_string()
        ),
        "Some(Port(\"unbound\"))".to_owned(),
    ] {
        let id = NodeId::new();
        nodes.push(id);
        child.nodes.push(kedra_scene::document::Node {
            id,
            name: value.clone(),
            parent: Some(child.root.into()),
            components: BTreeMap::from([(
                "links".into(),
                ComponentData::ron(1, &format!("(optional:{value})"))?,
            )]),
        });
    }
    let mut middle = PrefabDocument::new("Middle");
    middle.ports.insert("forward".into(), false);
    let inner = InstanceId::new();
    middle.instances.push(NestedInstance {
        id: inner,
        asset: child.id,
        parent: middle.root.into(),
        bindings: BTreeMap::from([("owner".into(), Reference::Port("forward".into()))]),
        overrides: Vec::new(),
    });
    let mut parent = PrefabDocument::new("Parent");
    parent.ports.insert("external".into(), false);
    let outer = InstanceId::new();
    parent.instances.push(NestedInstance {
        id: outer,
        asset: middle.id,
        parent: parent.root.into(),
        bindings: BTreeMap::from([("forward".into(), Reference::Port("external".into()))]),
        overrides: Vec::new(),
    });
    let mut working = WorkingSet::default();
    for source in [&child, &middle, &parent] {
        working.open(source.clone())?;
    }
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
    let map = working.unpack_complete(
        Unpack {
            document: parent.id,
            revision: 1,
            scope: vec![outer],
        },
        &snapshots[&parent.id],
        &policies,
        &types.read(),
        |catalog| compile_catalog(catalog, &types.read(), &policies, &mut loader).map(|_| ()),
    )?;
    let graph = working.catalog().resolve(parent.id)?;
    let root = &map[&NodeAddress {
        instances: vec![outer, inner],
        node: child.root,
    }];
    for present in [false, true] {
        let compiled = CompiledPrefab::compile(
            graph.clone(),
            &types.read(),
            &policies,
            &mut loader,
            &BTreeMap::from([("external".into(), present)]),
        )?;
        let values: Vec<_> = nodes
            .iter()
            .map(|node| {
                let address = &map[&NodeAddress {
                    instances: vec![outer, inner],
                    node: *node,
                }];
                compiled
                    .authoring_world()
                    .get::<Links>(compiled.source_entities()[address])
                    .unwrap()
                    .optional
            })
            .collect();
        assert_eq!(
            values[0].is_some(),
            present,
            "forwarded symbolic port must survive absent-port Unpack"
        );
        assert_eq!(values[1], None);
        assert_eq!(values[2], Some(compiled.source_entities()[root]));
        assert_eq!(values[3], None);
    }
    let after = working.documents()[&parent.id].document.clone();
    working.undo_checked(|_| Ok(()))?;
    assert_eq!(working.documents()[&parent.id].document, parent);
    working.redo_checked(|_| Ok(()))?;
    assert_eq!(working.documents()[&parent.id].document, after);
    Ok(())
}

#[test]
fn unpack_materializes_inherited_orders_for_descendants_and_containing_siblings() -> Result<()> {
    let (app, policies) = setup()?;
    let mut child = PrefabDocument::new("Ordered base");
    let branch = NodeId::new();
    child.nodes.push(kedra_scene::document::Node {
        id: branch,
        name: "Branch".into(),
        parent: Some(child.root.into()),
        components: BTreeMap::new(),
    });
    for parent in [child.root, branch] {
        let mut order = Vec::new();
        for number in 0..8 {
            let id = NodeId::new();
            order.push(NodeAddress::from(id));
            child.nodes.push(kedra_scene::document::Node {
                id,
                name: format!("{number}"),
                parent: Some(parent.into()),
                components: BTreeMap::new(),
            });
        }
        if parent == child.root {
            order.insert(3, branch.into());
        }
        child.overrides.push(Operation::SetChildOrder {
            node: parent.into(),
            children: order,
        });
    }
    let variant = child.variant("Ordered inherited prefab");
    let mut parent = PrefabDocument::new("Parent");
    let site = InstanceId::new();
    parent.instances.push(NestedInstance {
        id: site,
        asset: variant.id,
        parent: parent.root.into(),
        bindings: BTreeMap::new(),
        overrides: Vec::new(),
    });
    let sibling = NodeId::new();
    parent.nodes.push(kedra_scene::document::Node {
        id: sibling,
        name: "Sibling".into(),
        parent: Some(parent.root.into()),
        components: BTreeMap::new(),
    });
    let nested_root = NodeAddress {
        instances: vec![site],
        node: child.root,
    };
    parent.overrides.push(Operation::SetChildOrder {
        node: parent.root.into(),
        children: vec![nested_root, sibling.into()],
    });
    let mut working = WorkingSet::default();
    for source in [&child, &variant, &parent] {
        working.open(source.clone())?;
    }
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
    let before = snapshots[&parent.id].graph();
    let map = working.unpack_complete(
        Unpack {
            document: parent.id,
            revision: 1,
            scope: vec![site],
        },
        &snapshots[&parent.id],
        &policies,
        &types.read(),
        |_| Ok(()),
    )?;
    let after = working.catalog().resolve(parent.id)?;
    for node in before
        .nodes
        .values()
        .filter(|node| !node.children.is_empty())
    {
        let address = map.get(&node.address).unwrap_or(&node.address);
        let expected: Vec<_> = node
            .children
            .iter()
            .map(|child| map.get(child).unwrap_or(child).clone())
            .collect();
        // Explicit source order, rather than a lucky random UUID sort, must own the result.
        assert!(working.documents()[&parent.id].document.overrides.iter().any(|op|
            matches!(op,Operation::SetChildOrder {node,children} if node==address&&*children==expected)));
        assert_eq!(after.nodes[address].children, expected);
    }
    let after_document = working.documents()[&parent.id].document.clone();
    working.undo_checked(|_| Ok(()))?;
    working.redo_checked(|_| Ok(()))?;
    assert_eq!(working.documents()[&parent.id].document, after_document);
    Ok(())
}
