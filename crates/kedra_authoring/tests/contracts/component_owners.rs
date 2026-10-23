//! Whole-component edits follow the actual declared/operation-added override owner.
use crate::support::{Result, document, setup};
use bevy::prelude::*;
use kedra_authoring::{overrides::ApplyField, workspace::WorkingSet};
use kedra_scene::{assets::compile_catalog, document::*};
use std::collections::BTreeMap;

#[test]
fn component_apply_and_revert_reach_nested_override_owners_atomically() -> Result<()> {
    for added in [false, true] {
        for deep in [false, true] {
            for apply in [false, true] {
                let (app, policies) = setup()?;
                let child = document()?;
                let mut parent = PrefabDocument::new("Parent");
                let site = InstanceId::new();
                let mut instance = NestedInstance {
                    id: site,
                    asset: child.id,
                    parent: parent.root.into(),
                    bindings: BTreeMap::new(),
                    overrides: vec![Operation::SetComponent {
                        node: child.root.into(),
                        component: "settings".into(),
                        data: ComponentData::ron(1, "(speed:3.0)")?,
                    }],
                };
                let container = PrefabDocument::new("Container");
                let address = if deep {
                    let inner = instance.id;
                    instance.parent = container.root.into();
                    instance = NestedInstance {
                        id: InstanceId::new(),
                        asset: container.id,
                        parent: parent.root.into(),
                        bindings: BTreeMap::new(),
                        overrides: vec![Operation::AddNestedInstance(instance)],
                    };
                    NodeAddress {
                        instances: vec![instance.id, inner],
                        node: child.root,
                    }
                } else {
                    NodeAddress {
                        instances: vec![site],
                        node: child.root,
                    }
                };
                if added {
                    parent
                        .overrides
                        .push(Operation::AddNestedInstance(instance));
                } else {
                    parent.instances.push(instance);
                }
                let sibling = InstanceId::new();
                parent.instances.push(NestedInstance {
                    id: sibling,
                    asset: child.id,
                    parent: parent.root.into(),
                    bindings: BTreeMap::new(),
                    overrides: vec![Operation::SetComponent {
                        node: child.root.into(),
                        component: "settings".into(),
                        data: ComponentData::ron(1, "(speed:9.0)")?,
                    }],
                });
                let mut working = WorkingSet::default();
                working.open(child.clone())?;
                if deep {
                    working.open(container)?;
                }
                working.open(parent.clone())?;
                let types = app.world().resource::<AppTypeRegistry>();
                let mut loader = app.world().resource::<AssetServer>();
                let snapshots =
                    compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
                if apply {
                    working.apply_component(
                        ApplyField {
                            origin: parent.id,
                            target: child.id,
                            node: address.clone(),
                            component: "settings".into(),
                            path: String::new(),
                            expected: BTreeMap::from([(parent.id, 1), (child.id, 1)]),
                        },
                        &snapshots[&parent.id],
                        &policies,
                        &types.read(),
                        |_| Ok(()),
                    )?;
                } else {
                    working.revert_component(parent.id, 1, &address, "settings", |_| Ok(()))?;
                }
                let compiled =
                    compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
                let snapshot = &compiled[&parent.id];
                let sibling_address = NodeAddress {
                    instances: vec![sibling],
                    node: child.root,
                };
                assert_eq!(
                    snapshot
                        .authoring_world()
                        .get::<crate::support::Settings>(
                            snapshot.source_entities()[&sibling_address]
                        )
                        .ok_or("sibling settings missing")?
                        .speed,
                    9.0
                );
                assert_eq!(snapshot.graph().frames[&vec![sibling]], child.id);
                assert_eq!(
                    snapshot
                        .authoring_world()
                        .get::<crate::support::Settings>(snapshot.source_entities()[&address])
                        .ok_or("settings missing")?
                        .speed,
                    if apply { 3.0 } else { 1.0 }
                );
                working.undo_checked(|_| Ok(()))?;
                assert_eq!(working.documents()[&parent.id].document, parent);
                assert_eq!(working.documents()[&child.id].document, child);
                working.redo_checked(|_| Ok(()))?;
                let before = ron::to_string(&working)?;
                assert!(
                    working
                        .revert_component(parent.id, 1, &address, "settings", |_| Ok(()))
                        .is_err()
                );
                assert_eq!(ron::to_string(&working)?, before);
            }
        }
    }
    Ok(())
}

#[test]
fn component_owner_failures_preserve_sources_and_history() -> Result<()> {
    let (app, policies) = setup()?;
    let child = document()?;
    let mut parent = PrefabDocument::new("Parent");
    let site = InstanceId::new();
    parent.instances.push(NestedInstance {
        id: site,
        asset: child.id,
        parent: parent.root.into(),
        bindings: BTreeMap::new(),
        overrides: vec![Operation::SetComponent {
            node: child.root.into(),
            component: "settings".into(),
            data: ComponentData::ron(1, "(speed:7.0)")?,
        }],
    });
    let address = NodeAddress {
        instances: vec![site],
        node: child.root,
    };
    let mut working = WorkingSet::default();
    working.open(child.clone())?;
    working.open(parent.clone())?;
    let before = ron::to_string(&working)?;
    assert!(
        working
            .revert_component(parent.id, 1, &address, "absent", |_| Ok(()))
            .is_err()
    );
    assert!(
        working
            .revert_component(parent.id, 1, &address, "settings", |_| Err(
                kedra_scene::Error("validation rejected".into())
            ))
            .is_err()
    );
    assert_eq!(ron::to_string(&working)?, before);
    let types = app.world().resource::<AppTypeRegistry>();
    let snapshots = compile_catalog(
        &working.catalog(),
        &types.read(),
        &policies,
        &mut app.world().resource::<AssetServer>(),
    )?;
    for target in [parent.id, DocumentId::new(), child.id] {
        let expected = if target == child.id {
            BTreeMap::from([(parent.id, 1)])
        } else {
            BTreeMap::from([(parent.id, 1), (child.id, 1)])
        };
        assert!(
            working
                .apply_component(
                    ApplyField {
                        origin: parent.id,
                        target,
                        node: address.clone(),
                        component: "settings".into(),
                        path: String::new(),
                        expected,
                    },
                    &snapshots[&parent.id],
                    &policies,
                    &types.read(),
                    |_| Ok(())
                )
                .is_err()
        );
        assert_eq!(ron::to_string(&working)?, before);
    }
    // A nested RemoveComponent is also a Revert-owned operation, not a missing component.
    working.transact_checked(
        &BTreeMap::from([(parent.id, 1)]),
        |documents| {
            documents
                .get_mut(&parent.id)
                .ok_or_else(|| kedra_scene::Error("parent missing".into()))?
                .instances[0]
                .overrides = vec![Operation::RemoveComponent {
                node: child.root.into(),
                component: "settings".into(),
            }];
            Ok(())
        },
        |_| Ok(()),
    )?;
    let revision = working.documents()[&parent.id].revision;
    working.revert_component(parent.id, revision, &address, "settings", |_| Ok(()))?;
    let graph = working.catalog().resolve(parent.id)?;
    assert!(graph.nodes[&address].components.contains_key("settings"));
    Ok(())
}
