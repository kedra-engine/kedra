//! Compiled source values must belong to the guarded source/dependency snapshot.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated source/history fixture assertions"
)]
mod common;
use bevy::prelude::*;
use common::{Result, document, setup};
use kedra_authoring::{overrides::ApplyField, unpack::Unpack, workspace::WorkingSet};
use kedra_scene::{assets::compile_catalog, document::*};
use std::collections::BTreeMap;

#[test]
fn apply_rejects_stale_component_and_field_snapshots_without_consuming_history() -> Result<()> {
    for whole_component in [true, false] {
        let (app, policies) = setup()?;
        let base = document()?;
        let mut variant = base.variant("Variant");
        let operation = |speed| -> kedra_scene::Result<Operation> {
            Ok(if whole_component {
                Operation::SetComponent {
                    node: base.root.into(),
                    component: "settings".into(),
                    data: ComponentData::ron(1, &format!("(speed:{speed}.0)"))?,
                }
            } else {
                Operation::SetField {
                    node: base.root.into(),
                    component: "settings".into(),
                    version: 1,
                    path: "speed".into(),
                    value: ComponentData::ron(1, &format!("{speed}.0"))?.value,
                }
            })
        };
        variant.overrides.push(operation(3)?);
        let mut working = WorkingSet::default();
        working.open(base.clone())?;
        working.open(variant.clone())?;
        let types = app.world().resource::<AppTypeRegistry>();
        let mut loader = app.world().resource::<AssetServer>();
        let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
        working.patch(variant.id, 1, operation(9)?, |_| Ok(()))?;
        let before = ron::to_string(&working)?;
        let request = ApplyField {
            origin: variant.id,
            target: base.id,
            node: base.root.into(),
            component: "settings".into(),
            path: if whole_component { "" } else { "speed" }.into(),
            expected: working
                .documents()
                .iter()
                .map(|(id, d)| (*id, d.revision))
                .collect(),
        };
        let result = if whole_component {
            working.apply_component(
                request,
                &snapshots[&variant.id],
                &policies,
                &types.read(),
                |_| Ok(()),
            )
        } else {
            working.apply_field(
                request,
                &snapshots[&variant.id],
                &policies,
                &types.read(),
                &mut loader,
                |_| Ok(()),
            )
        };
        assert!(
            result.is_err(),
            "stale Apply must not replace speed 9 with speed 3"
        );
        assert_eq!(ron::to_string(&working)?, before);
        working.undo_checked(|_| Ok(()))?;
        assert_eq!(working.documents()[&variant.id].document, variant);
        let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
        let request = ApplyField {
            origin: variant.id,
            target: base.id,
            node: base.root.into(),
            component: "settings".into(),
            path: if whole_component { "" } else { "speed" }.into(),
            expected: working
                .documents()
                .iter()
                .map(|(id, d)| (*id, d.revision))
                .collect(),
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
        let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
        let compiled = &snapshots[&variant.id];
        assert_eq!(
            compiled
                .authoring_world()
                .get::<common::Settings>(compiled.source_entities()[&base.root.into()])
                .unwrap()
                .speed,
            3.0
        );
    }
    Ok(())
}

#[test]
fn unpack_rejects_transitive_source_changes_even_when_effective_values_are_shadowed() -> Result<()>
{
    let (app, policies) = setup()?;
    let base = document()?;
    let mut variant = base.variant("Variant");
    variant.overrides.push(Operation::SetComponent {
        node: base.root.into(),
        component: "settings".into(),
        data: ComponentData::ron(1, "(speed:3.0)")?,
    });
    let mut parent = PrefabDocument::new("Parent");
    let site = InstanceId::new();
    parent.instances.push(NestedInstance {
        id: site,
        asset: variant.id,
        parent: parent.root.into(),
        bindings: BTreeMap::new(),
        overrides: Vec::new(),
    });
    let mut working = WorkingSet::default();
    for source in [&base, &variant, &parent] {
        working.open(source.clone())?;
    }
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
    working.transact_checked(
        &BTreeMap::from([(base.id, 1)]),
        |documents| {
            // Titles are intentionally absent from effective node/component graphs.
            documents.get_mut(&base.id).unwrap().title = "Externally refreshed base".into();
            Ok(())
        },
        |_| Ok(()),
    )?;
    let before = ron::to_string(&working)?;
    assert!(
        working
            .unpack_complete(
                Unpack {
                    document: parent.id,
                    revision: 1,
                    scope: vec![site]
                },
                &snapshots[&parent.id],
                &policies,
                &types.read(),
                |_| Ok(())
            )
            .is_err()
    );
    assert_eq!(ron::to_string(&working)?, before);
    // A fresh compilation is admitted, including after an unrelated source is opened.
    let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
    working.open(PrefabDocument::new("Unrelated"))?;
    working.unpack_complete(
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
    Ok(())
}
