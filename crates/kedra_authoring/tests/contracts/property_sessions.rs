//! Property candidates own source revision/transaction semantics, not widget input.
use crate::support::{Result, baseline};
use bevy::{asset::AssetPlugin, prelude::*, reflect::ParsedPath};
use kedra_authoring::{
    persistence::Workspace,
    properties::{PropertyEdit, PropertySession, PropertyTarget},
};
use kedra_scene::{assets::compile_catalog, document::*, reflection::ComponentPolicies};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
struct Configuration {
    enabled: bool,
    exact: u64,
    axes: Axes,
    minimum: f32,
    maximum: f32,
}

#[derive(Clone, Reflect, Debug, PartialEq)]
struct Axes {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Component, Clone, Reflect, Debug)]
#[reflect(Component)]
enum CollectionState {
    Off,
    Active { names: Vec<String>, threshold: f32 },
}

struct Fixture {
    app: App,
    policies: ComponentPolicies,
    workspace: Workspace,
    compiled: BTreeMap<DocumentId, Arc<kedra_scene::instance::CompiledPrefab>>,
    parent: PrefabDocument,
    child: PrefabDocument,
    node: NodeAddress,
}
impl Fixture {
    fn new() -> Result<Self> {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_resource::<AppTypeRegistry>();
        let mut policies = ComponentPolicies::default();
        let types = app.world().resource::<AppTypeRegistry>();
        policies.register::<Configuration>(&mut types.write(), "configuration", 1, |value| {
            let value = Configuration::from_reflect(value)
                .ok_or_else(|| kedra_scene::Error("configuration type".into()))?;
            if [value.axes.x, value.axes.y, value.axes.z]
                .iter()
                .any(|v| !v.is_finite())
                || value.axes.x < 0.
            {
                return Err(kedra_scene::Error(
                    "axes must be finite and x nonnegative".into(),
                ));
            }
            if value.minimum > value.maximum {
                return Err(kedra_scene::Error("interval must be ordered".into()));
            }
            Ok(())
        })?;
        policies.register::<CollectionState>(&mut types.write(), "collection", 1, |value| {
            if let Some(CollectionState::Active { names, threshold }) =
                CollectionState::from_reflect(value)
                && (names.iter().any(String::is_empty) || !threshold.is_finite() || threshold < 0.)
            {
                return Err(kedra_scene::Error("invalid collection state".into()));
            }
            Ok(())
        })?;
        let mut child = PrefabDocument::new("Configuration source");
        child.nodes[0].components.insert("configuration".into(), ComponentData::ron(
            1, "(enabled:true,exact:18446744073709551615,axes:(x:1.0,y:2.0,z:3.0),minimum:1.0,maximum:2.0)",
        )?);
        child.nodes[0].components.insert(
            "collection".into(),
            ComponentData::ron(1, "Active(names:[\"first\",\"second\"],threshold:2.0)")?,
        );
        let mut parent = PrefabDocument::new("Scene containing prefab");
        let instance = InstanceId::new();
        parent.instances.push(NestedInstance {
            id: instance,
            asset: child.id,
            parent: parent.root.into(),
            bindings: BTreeMap::new(),
            overrides: vec![],
        });
        let node = NodeAddress {
            instances: vec![instance],
            node: child.root,
        };
        let registry = types.read();
        let (workspace, compiled) = Workspace::open_compiled(
            baseline(&[child.clone(), parent.clone()])?,
            &registry,
            &policies,
            &mut app.world().resource::<AssetServer>().clone(),
        );
        if let Some(error) = &workspace.diagnostic {
            return Err(error.clone().into());
        }
        drop(registry);
        Ok(Self {
            app,
            policies,
            workspace,
            compiled,
            parent,
            child,
            node,
        })
    }
    fn begin(&self) -> Result<PropertySession> {
        Ok(PropertySession::begin(
            &self.workspace.working,
            self.compiled[&self.parent.id].clone(),
            PropertyTarget {
                document: self.parent.id,
                node: self.node.clone(),
                component: "configuration".into(),
            },
            &self.app.world().resource::<AppTypeRegistry>().read(),
            &self.policies,
        )?)
    }
    fn propose(
        &self,
        session: &mut PropertySession,
        path: &str,
        value: &dyn bevy::reflect::PartialReflect,
    ) -> Result<()> {
        let types = self.app.world().resource::<AppTypeRegistry>().read();
        let mut loader = self.app.world().resource::<AssetServer>().clone();
        session.propose(
            &self.workspace.working,
            ParsedPath::parse(path).map_err(|e| e.to_string())?,
            value,
            &types,
            &self.policies,
            |catalog| compile_catalog(catalog, &types, &self.policies, &mut loader).map(|_| ()),
        )?;
        Ok(())
    }
    fn accept(&mut self, session: &mut PropertySession) -> Result<()> {
        let types = self.app.world().resource::<AppTypeRegistry>().read();
        let mut loader = self.app.world().resource::<AssetServer>().clone();
        session.accept(&mut self.workspace.working, |catalog| {
            compile_catalog(catalog, &types, &self.policies, &mut loader).map(|_| ())
        })?;
        Ok(())
    }
    fn state(&self) -> Result<String> {
        Ok(ron::to_string(&self.workspace.working)?)
    }
    fn refresh(&mut self) -> Result<()> {
        self.compiled = compile_catalog(
            &self.workspace.working.catalog(),
            &self.app.world().resource::<AppTypeRegistry>().read(),
            &self.policies,
            &mut self.app.world().resource::<AssetServer>().clone(),
        )?;
        Ok(())
    }
}

#[test]
fn live_property_updates_publish_current_values_but_share_one_explicit_undo_boundary() -> Result<()>
{
    let mut f = Fixture::new()?;
    let mut group = f.workspace.working.edit_group();
    let mut previous = f.workspace.working.documents()[&f.parent.id].revision;
    for value in [3_f32, 12., 123.] {
        let mut session = f.begin()?;
        f.propose(&mut session, "axes.x", &value)?;
        let types = f.app.world().resource::<AppTypeRegistry>().read();
        let mut loader = f.app.world().resource::<AssetServer>().clone();
        session.accept_grouped(&mut f.workspace.working, &mut group, |catalog| {
            compile_catalog(catalog, &types, &f.policies, &mut loader).map(|_| ())
        })?;
        drop(types);
        let current = f.workspace.working.documents()[&f.parent.id].revision;
        assert!(
            current > previous,
            "live source values still have distinct revisions"
        );
        previous = current;
        f.refresh()?;
        let compiled = &f.compiled[&f.parent.id];
        let value_now = compiled
            .authoring_world()
            .get::<Configuration>(compiled.source_entities()[&f.node])
            .ok_or("live value")?;
        assert_eq!(value_now.axes.x, value);
        assert_eq!(
            f.workspace.working.documents()[&f.child.id].document,
            f.child
        );
    }
    let latest = f.workspace.working.documents()[&f.parent.id]
        .document
        .clone();
    f.workspace.working.undo_checked(|_| Ok(()))?;
    assert_eq!(
        f.workspace.working.documents()[&f.parent.id].document,
        f.parent
    );
    assert!(!f.workspace.working.can_undo());
    f.workspace.working.redo_checked(|_| Ok(()))?;
    assert_eq!(
        f.workspace.working.documents()[&f.parent.id].document,
        latest
    );
    f.refresh()?;
    let mut stale = f.begin()?;
    f.propose(&mut stale, "axes.x", &124_f32)?;
    let before = f.state()?;
    assert!(
        stale
            .accept_grouped(&mut f.workspace.working, &mut group, |_| Ok(()))
            .is_err()
    );
    assert_eq!(
        f.state()?,
        before,
        "Undo/Redo revokes an old gesture, not its source candidate"
    );
    // A newly focused gesture has its own boundary, even for the same field.
    let mut next_group = f.workspace.working.edit_group();
    stale.accept_grouped(&mut f.workspace.working, &mut next_group, |_| Ok(()))?;
    f.workspace.working.undo_checked(|_| Ok(()))?;
    assert_eq!(
        f.workspace.working.documents()[&f.parent.id].document,
        latest
    );
    assert!(f.workspace.working.can_undo());
    Ok(())
}

#[test]
fn rejected_live_updates_preserve_the_gesture_and_noop_preserves_redo_and_foreign_history()
-> Result<()> {
    let mut f = Fixture::new()?;
    let mut group = f.workspace.working.edit_group();
    let mut session = f.begin()?;
    f.propose(&mut session, "axes.x", &5_f32)?;
    let before = f.state()?;
    assert!(
        session
            .accept_grouped(&mut f.workspace.working, &mut group, |_| Err(
                kedra_scene::Error("host refused".into())
            ))
            .is_err()
    );
    assert_eq!(f.state()?, before);
    session.accept_grouped(&mut f.workspace.working, &mut group, |_| Ok(()))?;
    f.refresh()?;
    // Invalid text never reaches acceptance; rejected native values cannot change
    // the last accepted live source value or destroy its original Undo boundary.
    let mut invalid = f.begin()?;
    let before = f.state()?;
    assert!(f.propose(&mut invalid, "axes.x", &f32::NAN).is_err());
    assert_eq!(f.state()?, before);
    f.workspace.working.undo_checked(|_| Ok(()))?;
    f.refresh()?;
    let mut noop_group = f.workspace.working.edit_group();
    let mut noop = f.begin()?;
    f.propose(&mut noop, "axes.x", &1_f32)?;
    let before = f.state()?;
    noop.accept_grouped(&mut f.workspace.working, &mut noop_group, |_| Ok(()))?;
    assert_eq!(f.state()?, before);
    assert!(f.workspace.working.can_redo());
    // Equal serialized state does not grant a token from another workspace lineage.
    let mut copy: kedra_authoring::workspace::WorkingSet = ron::from_str(&before)?;
    let expected = copy
        .documents()
        .iter()
        .map(|(id, doc)| (*id, doc.revision))
        .collect();
    assert!(
        copy.transact_grouped(&mut noop_group, &expected, |_| Ok(()), |_| Ok(()))
            .is_err()
    );
    assert_eq!(ron::to_string(&copy)?, before);
    // A foreign, successful edit closes the former live gesture's history boundary.
    let mut outside = f.begin()?;
    f.propose(&mut outside, "enabled", &false)?;
    f.accept(&mut outside)?;
    f.refresh()?;
    let mut later = f.begin()?;
    f.propose(&mut later, "axes.x", &6_f32)?;
    let before = f.state()?;
    assert!(
        later
            .accept_grouped(&mut f.workspace.working, &mut noop_group, |_| Ok(()))
            .is_err()
    );
    assert_eq!(f.state()?, before);
    Ok(())
}

#[test]
fn grouped_fields_accept_once_into_the_opened_instance_document_and_undo_together() -> Result<()> {
    let mut f = Fixture::new()?;
    let before = f.state()?;
    let mut session = f.begin()?;
    f.propose(&mut session, "enabled", &false)?;
    f.propose(&mut session, "axes.x", &10_f32)?;
    f.propose(&mut session, "exact", &(u64::MAX - 1))?;
    assert!(session.has_changes());
    assert_eq!(f.state()?, before, "proposals are not source transactions");
    let candidate = session.candidate(&f.workspace.working, |_| Ok(()))?;
    assert_ne!(candidate.documents[&f.parent.id], f.parent);
    f.accept(&mut session)?;
    assert!(!session.has_changes());
    assert_eq!(
        f.workspace.working.documents()[&f.child.id].document,
        f.child
    );
    assert!(!f.workspace.working.dirty(f.child.id));
    assert!(f.workspace.working.dirty(f.parent.id));
    let types = f.app.world().resource::<AppTypeRegistry>().read();
    let mut loader = f.app.world().resource::<AssetServer>().clone();
    let compiled = compile_catalog(
        &f.workspace.working.catalog(),
        &types,
        &f.policies,
        &mut loader,
    )?;
    let source = &compiled[&f.parent.id];
    let entity = source.source_entities()[&f.node];
    let value = source
        .authoring_world()
        .get::<Configuration>(entity)
        .ok_or("value")?;
    assert!(!value.enabled);
    assert_eq!(value.exact, u64::MAX - 1);
    assert_eq!(
        value.axes,
        Axes {
            x: 10.,
            y: 2.,
            z: 3.
        }
    );
    f.workspace.working.undo_checked(|_| Ok(()))?;
    assert!(!f.workspace.working.can_undo(), "one atomic history entry");
    assert_eq!(
        f.workspace.working.documents()[&f.parent.id].document,
        f.parent
    );
    f.workspace.working.redo_checked(|_| Ok(()))?;
    assert!(!f.workspace.working.can_redo());
    let accepted = ron::to_string(&f.workspace.working)?;
    assert!(
        session
            .accept(&mut f.workspace.working, |_| Ok(()))
            .is_err()
    );
    assert_eq!(ron::to_string(&f.workspace.working)?, accepted);
    Ok(())
}

#[test]
fn invalid_proposals_and_final_rejection_preserve_the_last_candidate_and_sources() -> Result<()> {
    let mut f = Fixture::new()?;
    let before = f.state()?;
    let mut session = f.begin()?;
    f.propose(&mut session, "axes.x", &4_f32)?;
    let candidate = session
        .candidate(&f.workspace.working, |_| Ok(()))?
        .documents;
    for (path, value) in [
        ("axes.x", -1_f32),
        ("axes.x", f32::NAN),
        ("axes.missing", 1_f32),
    ] {
        assert!(f.propose(&mut session, path, &value).is_err());
        assert_eq!(
            session
                .candidate(&f.workspace.working, |_| Ok(()))?
                .documents,
            candidate
        );
    }
    assert!(f.propose(&mut session, "enabled", &1_u64).is_err());
    assert!(
        f.propose(
            &mut session,
            "axes",
            &Axes {
                x: 2.,
                y: 2.,
                z: 3.
            }
        )
        .is_err(),
        "overlapping paths"
    );
    assert!(
        session
            .accept(&mut f.workspace.working, |_| Err(kedra_scene::Error(
                "host permission revoked".into()
            )))
            .is_err()
    );
    assert_eq!(f.state()?, before);
    assert!(session.has_changes());
    session.cancel();
    assert!(f.accept(&mut session).is_err());
    assert_eq!(f.state()?, before);
    Ok(())
}

#[test]
fn changed_dependency_metadata_conflicts_without_rebasing_or_consuming_history() -> Result<()> {
    let mut f = Fixture::new()?;
    let mut session = f.begin()?;
    f.propose(&mut session, "enabled", &false)?;
    f.workspace.working.transact_checked(
        &BTreeMap::from([(f.child.id, 1)]),
        |documents| {
            documents
                .get_mut(&f.child.id)
                .ok_or_else(|| kedra_scene::Error("child".into()))?
                .title = "Changed metadata".into();
            Ok(())
        },
        |_| Ok(()),
    )?;
    let before = f.state()?;
    assert!(f.accept(&mut session).is_err());
    assert!(f.propose(&mut session, "enabled", &false).is_err());
    assert!(session.candidate(&f.workspace.working, |_| Ok(())).is_err());
    assert_eq!(f.state()?, before);
    assert!(
        f.begin().is_err(),
        "stale compilation cannot begin another edit"
    );
    Ok(())
}

#[test]
fn returning_to_baseline_and_cancel_preserve_redo_and_saved_revisions() -> Result<()> {
    let mut f = Fixture::new()?;
    let mut first = f.begin()?;
    f.propose(&mut first, "enabled", &false)?;
    f.accept(&mut first)?;
    f.workspace.working.undo_checked(|_| Ok(()))?;
    let before = f.state()?;
    let mut session = f.begin()?;
    f.propose(&mut session, ".enabled", &false)?;
    f.propose(&mut session, "enabled", &true)?;
    assert!(!session.has_changes());
    f.accept(&mut session)?;
    assert_eq!(f.state()?, before);
    assert!(f.workspace.working.can_redo());
    let mut cancelled = f.begin()?;
    f.propose(&mut cancelled, "exact", &7_u64)?;
    cancelled.cancel();
    assert_eq!(f.state()?, before);
    Ok(())
}

#[test]
fn native_path_aliases_replace_an_existing_field_override_without_duplicates() -> Result<()> {
    let mut f = Fixture::new()?;
    f.workspace.working.patch(
        f.parent.id,
        1,
        Operation::SetField {
            node: f.node.clone(),
            component: "configuration".into(),
            version: 1,
            path: "axes.x".into(),
            value: ComponentData::ron(1, "4.0")?.value,
        },
        |_| Ok(()),
    )?;
    f.refresh()?;
    let mut session = f.begin()?;
    f.propose(&mut session, ".axes.x", &7_f32)?;
    f.accept(&mut session)?;
    let overrides = &f.workspace.working.documents()[&f.parent.id]
        .document
        .overrides;
    assert_eq!(
        overrides.len(),
        1,
        "one override owns the canonical property"
    );
    f.refresh()?;
    let mut repeated = f.begin()?;
    f.propose(&mut repeated, "axes.x", &8_f32)?;
    f.accept(&mut repeated)?;
    f.refresh()?;
    let source = &f.compiled[&f.parent.id];
    assert_eq!(
        source
            .authoring_world()
            .get::<Configuration>(source.source_entities()[&f.node])
            .ok_or("value")?
            .axes
            .x,
        8.
    );
    let revision = f.workspace.working.documents()[&f.parent.id].revision;
    f.workspace.working.revert_field(
        f.parent.id,
        revision,
        &f.node,
        "configuration",
        ".axes.x",
        |_| Ok(()),
    )?;
    f.refresh()?;
    let source = &f.compiled[&f.parent.id];
    assert_eq!(
        source
            .authoring_world()
            .get::<Configuration>(source.source_entities()[&f.node])
            .ok_or("value")?
            .axes
            .x,
        1.
    );
    Ok(())
}

#[test]
fn coupled_fields_validate_the_complete_batch_and_rejected_batches_retain_the_candidate()
-> Result<()> {
    let mut f = Fixture::new()?;
    let before = f.state()?;
    let mut session = f.begin()?;
    assert!(f.propose(&mut session, "minimum", &3_f32).is_err());
    assert!(!session.has_changes());
    let registry = f.app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let mut loader = f.app.world().resource::<AssetServer>().clone();
    let edits = [
        PropertyEdit {
            path: ParsedPath::parse("minimum")?,
            value: &3_f32,
        },
        PropertyEdit {
            path: ParsedPath::parse("maximum")?,
            value: &4_f32,
        },
    ];
    session.propose_batch(
        &f.workspace.working,
        &edits,
        &types,
        &f.policies,
        |catalog| compile_catalog(catalog, &types, &f.policies, &mut loader).map(|_| ()),
    )?;
    let candidate = session
        .candidate(&f.workspace.working, |_| Ok(()))?
        .documents;
    let duplicates = [
        PropertyEdit {
            path: ParsedPath::parse("minimum")?,
            value: &3_f32,
        },
        PropertyEdit {
            path: ParsedPath::parse(".minimum")?,
            value: &4_f32,
        },
    ];
    assert!(
        session
            .propose_batch(
                &f.workspace.working,
                &duplicates,
                &types,
                &f.policies,
                |_| Ok(())
            )
            .is_err()
    );
    assert_eq!(
        session
            .candidate(&f.workspace.working, |_| Ok(()))?
            .documents,
        candidate
    );
    assert_eq!(f.state()?, before);
    f.accept(&mut session)?;
    f.workspace.working.undo_checked(|_| Ok(()))?;
    assert_eq!(
        f.workspace.working.documents()[&f.parent.id].document,
        f.parent
    );
    assert!(!f.workspace.working.can_undo());
    Ok(())
}

#[test]
fn child_of_an_existing_parent_override_preserves_siblings_and_rejects_invalid_candidates()
-> Result<()> {
    let mut f = Fixture::new()?;
    f.workspace.working.patch(
        f.parent.id,
        1,
        Operation::SetField {
            node: f.node.clone(),
            component: "configuration".into(),
            version: 1,
            path: ".axes".into(),
            value: ComponentData::ron(1, "(x:4.0,y:5.0,z:6.0)")?.value,
        },
        |_| Ok(()),
    )?;
    f.refresh()?;
    let before = f.state()?;
    let mut session = f.begin()?;
    assert!(f.propose(&mut session, "axes.x", &(-1_f32)).is_err());
    assert!(!session.has_changes());
    assert_eq!(f.state()?, before);
    f.propose(&mut session, "axes.x", &7_f32)?;
    f.accept(&mut session)?;
    assert_eq!(
        f.workspace.working.documents()[&f.parent.id]
            .document
            .overrides
            .len(),
        1
    );
    assert_eq!(
        f.workspace.working.documents()[&f.child.id].document,
        f.child
    );
    f.refresh()?;
    let source = &f.compiled[&f.parent.id];
    let value = source
        .authoring_world()
        .get::<Configuration>(source.source_entities()[&f.node])
        .ok_or("configuration")?;
    assert_eq!(
        value.axes,
        Axes {
            x: 7.,
            y: 5.,
            z: 6.
        }
    );
    let mut next = f.begin()?;
    f.propose(&mut next, "enabled", &false)?;
    f.accept(&mut next)?;
    f.refresh()?;
    let source = &f.compiled[&f.parent.id];
    let value = source
        .authoring_world()
        .get::<Configuration>(source.source_entities()[&f.node])
        .ok_or("configuration")?;
    assert!(!value.enabled);
    assert_eq!(
        value.axes,
        Axes {
            x: 7.,
            y: 5.,
            z: 6.
        }
    );
    Ok(())
}

#[test]
fn compound_and_indexed_changes_replace_exact_values_without_growing_lists_or_editing_prefabs()
-> Result<()> {
    let mut f = Fixture::new()?;
    let begin = |f: &Fixture| -> Result<PropertySession> {
        Ok(PropertySession::begin(
            &f.workspace.working,
            f.compiled[&f.parent.id].clone(),
            PropertyTarget {
                document: f.parent.id,
                node: f.node.clone(),
                component: "collection".into(),
            },
            &f.app.world().resource::<AppTypeRegistry>().read(),
            &f.policies,
        )?)
    };
    let mut session = begin(&f)?;
    f.propose(&mut session, "names", &vec!["only".to_owned()])?;
    f.accept(&mut session)?;
    f.refresh()?;
    let source = &f.compiled[&f.parent.id];
    let actual = source
        .authoring_world()
        .get::<CollectionState>(source.source_entities()[&f.node]);
    assert!(
        matches!(actual, Some(CollectionState::Active { names, threshold }) if names == &["only"] && *threshold == 2.),
        "{actual:?}; {}",
        f.state()?
    );
    let before = f.state()?;
    let mut session = begin(&f)?;
    assert!(
        f.propose(&mut session, "names[1]", &"outside".to_owned())
            .is_err()
    );
    assert!(f.propose(&mut session, "names[0]", &String::new()).is_err());
    assert_eq!(f.state()?, before);
    f.propose(&mut session, "names[0]", &"replacement".to_owned())?;
    f.accept(&mut session)?;
    f.refresh()?;
    let source = &f.compiled[&f.parent.id];
    assert!(
        matches!(source.authoring_world().get::<CollectionState>(source.source_entities()[&f.node]), Some(CollectionState::Active { names, threshold }) if names == &["replacement"] && *threshold == 2.)
    );
    let mut stale = begin(&f)?;
    let mut root = begin(&f)?;
    f.propose(&mut root, "", &CollectionState::Off)?;
    f.accept(&mut root)?;
    assert!(
        f.propose(&mut stale, "names[0]", &"late".to_owned())
            .is_err()
    );
    f.refresh()?;
    let source = &f.compiled[&f.parent.id];
    assert!(matches!(
        source
            .authoring_world()
            .get::<CollectionState>(source.source_entities()[&f.node]),
        Some(CollectionState::Off)
    ));
    assert_eq!(
        f.workspace.working.documents()[&f.child.id].document,
        f.child
    );
    f.workspace.working.undo_checked(|_| Ok(()))?;
    f.refresh()?;
    let source = &f.compiled[&f.parent.id];
    assert!(
        matches!(source.authoring_world().get::<CollectionState>(source.source_entities()[&f.node]), Some(CollectionState::Active { names, .. }) if names == &["replacement"])
    );
    Ok(())
}
