//! Apply shadowing, destination representations and failure atomicity.
mod common;
use bevy::prelude::*;
use common::{Result, document, setup};
use kedra_authoring::{overrides::ApplyField, workspace::WorkingSet};
use kedra_scene::{assets::compile_catalog, document::*};
use std::collections::BTreeMap;

fn field(node: NodeId, speed: &str) -> Result<Operation> {
    Ok(Operation::SetField {
        node: node.into(),
        component: "settings".into(),
        version: 1,
        path: "speed".into(),
        value: ComponentData::ron(1, speed)?.value,
    })
}

#[test]
fn applying_a_field_updates_an_existing_whole_component_and_is_reversible() -> Result<()> {
    let (app, policies) = setup()?;
    let mut base = document()?;
    base.overrides.push(Operation::SetComponent {
        node: base.root.into(),
        component: "settings".into(),
        data: ComponentData::ron(1, "(speed:2.0)")?,
    });
    let mut variant = base.variant("Variant");
    variant.overrides.push(field(base.root, "4.0")?);
    let mut working = WorkingSet::default();
    working.open(base.clone())?;
    working.open(variant.clone())?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
    working.apply_field(
        ApplyField {
            origin: variant.id,
            target: base.id,
            node: base.root.into(),
            component: "settings".into(),
            path: "speed".into(),
            expected: BTreeMap::from([(base.id, 1), (variant.id, 1)]),
        },
        &snapshots[&variant.id],
        &policies,
        &types.read(),
        &mut loader,
        |_| Ok(()),
    )?;
    assert!(
        working.documents()[&variant.id]
            .document
            .overrides
            .is_empty()
    );
    assert!(matches!(
        working.documents()[&base.id].document.overrides.as_slice(),
        [Operation::SetComponent { .. }]
    ));
    let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
    let compiled = &snapshots[&base.id];
    assert_eq!(
        compiled
            .authoring_world()
            .get::<common::Settings>(compiled.source_entities()[&base.root.into()])
            .ok_or("settings missing")?
            .speed,
        4.0
    );
    working.undo_checked(|_| Ok(()))?;
    assert_eq!(working.documents()[&base.id].document, base);
    assert_eq!(working.documents()[&variant.id].document, variant);
    working.redo_checked(|_| Ok(()))?;
    Ok(())
}

#[test]
fn intervening_field_or_whole_component_shadowing_rejects_apply_without_history() -> Result<()> {
    for whole in [false, true] {
        let (app, policies) = setup()?;
        let base = document()?;
        let mut middle = base.variant("Middle");
        let mut leaf = middle.variant("Leaf");
        if whole {
            for (source, speed) in [(&mut middle, "3.0"), (&mut leaf, "7.0")] {
                source.overrides.push(Operation::SetComponent {
                    node: base.root.into(),
                    component: "settings".into(),
                    data: ComponentData::ron(1, &format!("(speed:{speed})"))?,
                });
            }
        } else {
            middle.overrides.push(field(base.root, "3.0")?);
            leaf.overrides.push(field(base.root, "7.0")?);
        }
        let mut working = WorkingSet::default();
        for source in [&base, &middle, &leaf] {
            working.open(source.clone())?;
        }
        let types = app.world().resource::<AppTypeRegistry>();
        let mut loader = app.world().resource::<AssetServer>();
        let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
        let before = ron::to_string(&working)?;
        let request = ApplyField {
            origin: leaf.id,
            target: base.id,
            node: base.root.into(),
            component: "settings".into(),
            path: if whole { "" } else { "speed" }.into(),
            expected: BTreeMap::from([(leaf.id, 1), (base.id, 1)]),
        };
        let result = if whole {
            working.apply_component(
                request,
                &snapshots[&leaf.id],
                &policies,
                &types.read(),
                |_| Ok(()),
            )
        } else {
            working.apply_field(
                request,
                &snapshots[&leaf.id],
                &policies,
                &types.read(),
                &mut loader,
                |_| Ok(()),
            )
        };
        assert!(result.is_err());
        assert_eq!(ron::to_string(&working)?, before);
        assert!(!working.can_undo());
    }
    Ok(())
}

#[test]
fn missing_origin_and_field_revert_errors_do_not_consume_sources_or_history() -> Result<()> {
    let (app, policies) = setup()?;
    let base = document()?;
    let variant = base.variant("Unmodified variant");
    let mut working = WorkingSet::default();
    working.open(base.clone())?;
    working.open(variant.clone())?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
    let before = ron::to_string(&working)?;
    assert!(
        working
            .revert_field(
                variant.id,
                1,
                &base.root.into(),
                "settings",
                "speed",
                |_| Ok(())
            )
            .is_err()
    );
    for whole in [false, true] {
        let request = ApplyField {
            origin: variant.id,
            target: base.id,
            node: base.root.into(),
            component: "settings".into(),
            path: if whole { "" } else { "speed" }.into(),
            expected: BTreeMap::from([(variant.id, 1), (base.id, 1)]),
        };
        let result = if whole {
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
        assert!(result.is_err());
        assert_eq!(ron::to_string(&working)?, before);
    }
    working.patch(variant.id, 1, field(base.root, "5.0")?, |_| Ok(()))?;
    let revision = working.documents()[&variant.id].revision;
    working.revert_field(
        variant.id,
        revision,
        &base.root.into(),
        "settings",
        "speed",
        |_| Ok(()),
    )?;
    assert!(
        working.documents()[&variant.id]
            .document
            .overrides
            .is_empty()
    );
    Ok(())
}
