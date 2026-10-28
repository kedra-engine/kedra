use super::*;
use crate::documents::{Documents, source};
use bevy::{prelude::*, reflect::TypeRegistry};
use kedra_authoring::persistence::{SourceFile, SourceSnapshot, content_hash};
use kedra_scene::{document::*, reflection::ComponentPolicies};
use std::collections::BTreeMap;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Component, Reflect)]
#[reflect(Component)]
struct ApplicationValue {
    enabled: bool,
    exact: u64,
}

#[test]
fn working_source_refresh_updates_locked_panel_controls_and_retires_removed_components()
-> TestResult {
    use kedra_authoring::properties::{PropertySession, PropertyTarget};
    use kedra_docking::model::ViewId;
    let mut app = crate::workspace::browser_test_app()?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let policies = app.world().resource::<ComponentPolicies>().clone();
    let mut document = PrefabDocument::new("Retained working source");
    document.nodes[0].components.insert(
        "ui.editable".into(),
        ComponentData::ron(1, "(autofocus:false,select_all:true)")?,
    );
    let text = document.serialize()?;
    let (scope, compiled) = source::admit(
        SourceSnapshot {
            namespace: "game".into(),
            generation: 1,
            documents: vec![SourceFile {
                path: "retained.prefab.ron".into(),
                hash: content_hash(&text),
                source: text.clone(),
            }],
            assets: vec![],
            index: None,
        },
        &registry.read(),
        &policies,
    )?;
    let graph = std::sync::Arc::new(compiled[&document.id].graph().clone());
    let mut documents = Documents {
        compiled_revisions: scope
            .working
            .documents()
            .iter()
            .map(|(id, d)| (*id, d.revision))
            .collect(),
        scope: Some(scope),
        compiled,
        ..Default::default()
    };
    documents.published(
        crate::documents::Ready {
            path: "retained.prefab.ron".into(),
            document: document.id,
            graph,
        },
        ViewId::new(),
        document.root.into(),
    );
    let parent = app
        .world()
        .resource::<kedra_desktop::DesktopSurface>()
        .root
        .ok_or("root")?;
    let id = ViewId::new();
    let mut panel = crate::inspector::Panel::mount(app.world_mut(), parent, id)?;
    panel.tick(app.world_mut(), id, &mut documents)?;
    panel.action(app.world_mut(), crate::inspector::Action::Lock)?;
    let field = app
        .world_mut()
        .query_filtered::<Entity, (With<bevy::ui_widgets::Checkbox>, With<bevy::ui::Checked>)>()
        .iter(app.world())
        .next()
        .ok_or("field")?;
    let token = panel.state.token;
    let snapshot = documents.compiled[&document.id].clone();
    let scope = documents.scope.as_mut().ok_or("scope")?;
    let mut session = PropertySession::begin(
        &scope.working,
        snapshot,
        PropertyTarget {
            document: document.id,
            node: document.root.into(),
            component: "ui.editable".into(),
        },
        &registry.read(),
        &policies,
    )?;
    session.propose(
        &scope.working,
        bevy::reflect::ParsedPath::parse("select_all")?,
        &false,
        &registry.read(),
        &policies,
        |_| Ok(()),
    )?;
    session.accept(&mut scope.working, |_| Ok(()))?;
    documents.active = None;
    panel.tick(app.world_mut(), id, &mut documents)?;
    assert!(
        app.world()
            .get::<bevy::ui_widgets::Checkbox>(field)
            .is_some()
    );
    assert!(app.world().get::<bevy::ui::Checked>(field).is_none());
    assert_eq!(
        panel.state.token, token,
        "Refreshing values must not replace the binding identity"
    );
    let scope = documents.scope.as_mut().ok_or("scope")?;
    let revision = scope.working.documents()[&document.id].revision;
    scope.working.transact_checked(
        &BTreeMap::from([(document.id, revision)]),
        |docs| {
            let document = docs
                .get_mut(&document.id)
                .ok_or_else(|| kedra_scene::Error("document".into()))?;
            document.nodes[0].components.clear();
            document.overrides.clear();
            Ok(())
        },
        |_| Ok(()),
    )?;
    panel.tick(app.world_mut(), id, &mut documents)?;
    assert!(
        app.world().get_entity(field).is_err(),
        "Removed source components must not remain as stale controls"
    );
    assert_eq!(
        documents.scope.as_ref().ok_or("scope")?.source.documents[0].source,
        text
    );
    Ok(())
}

#[test]
fn current_inspection_tracks_accepted_property_history_without_reopening_or_saving() -> TestResult {
    use kedra_authoring::properties::{PropertySession, PropertyTarget};
    use std::sync::Arc;
    let mut types = TypeRegistry::default();
    let mut policies = ComponentPolicies::default();
    policies.register::<ApplicationValue>(&mut types, "test.application", 1, |_| Ok(()))?;
    let mut document = PrefabDocument::new("Working source");
    document.nodes[0].components.insert(
        "test.application".into(),
        ComponentData::ron(1, "(enabled:true,exact:7)")?,
    );
    let text = document.serialize()?;
    let (scope, compiled) = source::admit(
        SourceSnapshot {
            namespace: "game".into(),
            generation: 1,
            documents: vec![SourceFile {
                path: "working.prefab.ron".into(),
                hash: content_hash(&text),
                source: text.clone(),
            }],
            assets: vec![],
            index: None,
        },
        &types,
        &policies,
    )?;
    let mut documents = Documents {
        compiled_revisions: scope
            .working
            .documents()
            .iter()
            .map(|(id, d)| (*id, d.revision))
            .collect(),
        scope: Some(scope),
        compiled,
        ..Default::default()
    };
    let target = documents
        .target(document.id, document.root.into())
        .ok_or("target")?;
    let baseline = documents.compiled[&document.id].clone();
    let scope = documents.scope.as_mut().ok_or("scope")?;
    let mut session = PropertySession::begin(
        &scope.working,
        baseline.clone(),
        PropertyTarget {
            document: document.id,
            node: document.root.into(),
            component: "test.application".into(),
        },
        &types,
        &policies,
    )?;
    session.propose(
        &scope.working,
        bevy::reflect::ParsedPath::parse("exact")?,
        &u64::MAX,
        &types,
        &policies,
        |_| Ok(()),
    )?;
    session.accept(&mut scope.working, |_| Ok(()))?;
    assert!(
        documents.inspect(&target).is_err(),
        "Never expose an admission value under a new revision"
    );
    documents.refresh_inspection(&types, &policies)?;
    let current = documents.compiled[&document.id].clone();
    assert!(!Arc::ptr_eq(&baseline, &current));
    let exact = |documents: &Documents| -> Result<u64, String> {
        documents
            .inspect(&target)?
            .value("test.application", &types, &policies)?
            .try_downcast_ref::<ApplicationValue>()
            .map(|v| v.exact)
            .ok_or("value".into())
    };
    assert_eq!(exact(&documents)?, u64::MAX);
    documents.refresh_inspection(&types, &policies)?;
    assert!(
        Arc::ptr_eq(&current, &documents.compiled[&document.id]),
        "Idle refresh retains the compilation"
    );
    documents
        .scope
        .as_mut()
        .ok_or("scope")?
        .working
        .undo_checked(|_| Ok(()))?;
    documents.refresh_inspection(&types, &policies)?;
    assert_eq!(exact(&documents)?, 7);
    documents
        .scope
        .as_mut()
        .ok_or("scope")?
        .working
        .redo_checked(|_| Ok(()))?;
    documents.refresh_inspection(&types, &policies)?;
    assert_eq!(exact(&documents)?, u64::MAX);
    let scope = documents.scope.as_ref().ok_or("scope")?;
    assert_eq!(scope.source.documents[0].source, text);
    assert_eq!(scope.working.documents()[&document.id].saved_revision, 1);
    assert!(scope.working.dirty(document.id));
    Ok(())
}

#[test]
fn rejected_inspection_refresh_preserves_cache_sources_and_history_without_stale_values()
-> TestResult {
    use std::sync::Arc;
    let mut types = TypeRegistry::default();
    let mut policies = ComponentPolicies::default();
    policies.register::<ApplicationValue>(&mut types, "test.application", 1, |value| {
        let value = value
            .try_downcast_ref::<ApplicationValue>()
            .ok_or_else(|| kedra_scene::Error("Application value type is unavailable".into()))?;
        if value.exact > 10 {
            Err(kedra_scene::Error("Value exceeds domain limit".into()))
        } else {
            Ok(())
        }
    })?;
    let mut document = PrefabDocument::new("Validated source");
    document.nodes[0].components.insert(
        "test.application".into(),
        ComponentData::ron(1, "(enabled:true,exact:7)")?,
    );
    let text = document.serialize()?;
    let (scope, compiled) = source::admit(
        SourceSnapshot {
            namespace: "game".into(),
            generation: 1,
            documents: vec![SourceFile {
                path: "validated.prefab.ron".into(),
                hash: content_hash(&text),
                source: text,
            }],
            assets: vec![],
            index: None,
        },
        &types,
        &policies,
    )?;
    let mut documents = Documents {
        compiled_revisions: scope
            .working
            .documents()
            .iter()
            .map(|(id, d)| (*id, d.revision))
            .collect(),
        scope: Some(scope),
        compiled,
        ..Default::default()
    };
    let target = documents
        .target(document.id, document.root.into())
        .ok_or("target")?;
    let baseline = documents.compiled[&document.id].clone();
    // A different source consumer published a graph-valid but policy-invalid value.
    documents
        .scope
        .as_mut()
        .ok_or("scope")?
        .working
        .transact_checked(
            &BTreeMap::from([(document.id, 1)]),
            |docs| {
                docs.get_mut(&document.id)
                    .ok_or_else(|| kedra_scene::Error("document".into()))?
                    .nodes[0]
                    .components
                    .insert(
                        "test.application".into(),
                        ComponentData::ron(1, "(enabled:true,exact:11)")?,
                    );
                Ok(())
            },
            |_| Ok(()),
        )?;
    let before = ron::to_string(documents.scope.as_ref().ok_or("scope")?)?;
    assert!(documents.refresh_inspection(&types, &policies).is_err());
    assert!(documents.inspect(&target).is_err());
    assert!(Arc::ptr_eq(&baseline, &documents.compiled[&document.id]));
    assert_eq!(documents.compiled_revisions[&document.id], 1);
    assert_eq!(
        ron::to_string(documents.scope.as_ref().ok_or("scope")?)?,
        before
    );
    documents
        .scope
        .as_mut()
        .ok_or("scope")?
        .working
        .undo_checked(|_| Ok(()))?;
    documents.refresh_inspection(&types, &policies)?;
    assert_eq!(
        documents
            .inspect(&target)?
            .value("test.application", &types, &policies)?
            .try_downcast_ref::<ApplicationValue>()
            .map(|value| value.exact),
        Some(7)
    );
    Ok(())
}

#[test]
fn source_values_are_inspectable_without_rendering_and_missing_or_foreign_targets_reject()
-> TestResult {
    let mut types = TypeRegistry::default();
    let mut policies = ComponentPolicies::default();
    policies.register::<ApplicationValue>(&mut types, "test.application", 1, |_| Ok(()))?;
    let mut document = PrefabDocument::new("Independent source");
    document.nodes[0].components.insert(
        "test.application".into(),
        ComponentData::ron(1, "(enabled:true,exact:18446744073709551615)")?,
    );
    let text = document.serialize()?;
    let source = SourceSnapshot {
        namespace: "game".into(),
        generation: 1,
        documents: vec![SourceFile {
            path: "source.prefab.ron".into(),
            hash: content_hash(&text),
            source: text.clone(),
        }],
        assets: vec![],
        index: None,
    };
    let (scope, compiled) = source::admit(source, &types, &policies)?;
    let documents = Documents {
        compiled_revisions: scope
            .working
            .documents()
            .iter()
            .map(|(id, d)| (*id, d.revision))
            .collect(),
        scope: Some(scope),
        compiled,
        ..Default::default()
    };
    let target = documents
        .target(document.id, document.root.into())
        .ok_or("target")?;
    let snapshot = documents.inspect(&target)?;
    assert_eq!(
        snapshot.components.len(),
        1,
        "Do not include compiler Name/ChildOf components"
    );
    let value = snapshot.value("test.application", &types, &policies)?;
    assert_eq!(
        value
            .try_downcast_ref::<ApplicationValue>()
            .map(|v| v.exact),
        Some(u64::MAX)
    );
    assert_eq!(snapshot.source_path, "source.prefab.ron");
    assert!(
        documents
            .inspect(&Target {
                node: NodeId::new().into(),
                ..target.clone()
            })
            .is_err()
    );
    assert!(Documents::default().inspect(&target).is_err());
    let mut state = crate::inspector::model::State::default();
    assert!(state.follow(Some(target.clone()), None)?);
    let token = state.token;
    assert!(!state.follow(Some(target.clone()), None)?);
    state.toggle_lock();
    assert!(state.locked);
    let next = Target {
        document: DocumentId::new(),
        ..target.clone()
    };
    assert!(!state.follow(Some(next.clone()), None)?);
    assert_eq!(state.target.as_ref(), Some(&target));
    assert_eq!(state.token, token);
    state.toggle_lock();
    assert!(state.follow(Some(next), None)?);
    assert_ne!(state.token, token);
    assert!(state.follow(None, None)?);
    assert!(state.target.is_none());
    let asset = documents
        .library()
        .ok_or("library")?
        .asset("game://source.prefab.ron")
        .ok_or("asset")?;
    assert!(state.follow(None, Some(asset.clone()))?);
    state.toggle_lock();
    let token = state.token;
    assert!(!state.follow(Some(target.clone()), None)?);
    assert_eq!(state.asset, Some(asset));
    assert!(state.target.is_none());
    assert_eq!(state.token, token);
    state.toggle_lock();
    assert!(state.follow(Some(target), None)?);
    assert!(state.asset.is_none());
    let scope = documents.scope.as_ref().ok_or("scope")?;
    assert_eq!(scope.source.documents[0].source, text);
    assert!(!scope.working.can_undo());
    assert!(!scope.working.can_redo());
    assert_eq!(scope.working.documents()[&document.id].revision, 1);
    assert_eq!(scope.working.documents()[&document.id].saved_revision, 1);
    let _: BTreeMap<_, _> = documents.compiled;
    Ok(())
}

#[test]
fn nested_field_overrides_keep_effective_values_and_field_level_source_provenance() -> TestResult {
    use kedra_inspector::binding::ValueMetadata;
    let mut types = TypeRegistry::default();
    let mut policies = ComponentPolicies::default();
    policies.register::<ApplicationValue>(&mut types, "test.application", 1, |_| Ok(()))?;
    let mut child = PrefabDocument::new("Child");
    child.nodes[0].components.insert(
        "test.application".into(),
        ComponentData::ron(1, "(enabled:true,exact:7)")?,
    );
    let mut parent = PrefabDocument::new("Parent");
    let instance = InstanceId::new();
    parent.instances.push(NestedInstance {
        id: instance,
        asset: child.id,
        parent: parent.root.into(),
        bindings: BTreeMap::new(),
        overrides: vec![Operation::SetField {
            node: child.root.into(),
            component: "test.application".into(),
            version: 1,
            path: "enabled".into(),
            value: ComponentData::ron(1, "false")?.value,
        }],
    });
    let source = SourceSnapshot {
        namespace: "game".into(),
        generation: 1,
        documents: [("child.prefab.ron", &child), ("parent.prefab.ron", &parent)]
            .into_iter()
            .map(|(path, doc)| {
                let text = doc.serialize().unwrap_or_else(|e| panic!("{e}"));
                SourceFile {
                    path: path.into(),
                    hash: content_hash(&text),
                    source: text,
                }
            })
            .collect(),
        assets: vec![],
        index: None,
    };
    let (scope, compiled) = source::admit(source, &types, &policies)?;
    let mut documents = Documents {
        compiled_revisions: scope
            .working
            .documents()
            .iter()
            .map(|(id, d)| (*id, d.revision))
            .collect(),
        scope: Some(scope),
        compiled,
        ..Default::default()
    };
    let target = documents
        .target(
            parent.id,
            NodeAddress {
                instances: vec![instance],
                node: child.root,
            },
        )
        .ok_or("target")?;
    let snapshot = documents.inspect(&target)?;
    assert_eq!(
        snapshot
            .value("test.application", &types, &policies)?
            .try_downcast_ref::<ApplicationValue>()
            .map(|v| (v.enabled, v.exact)),
        Some((false, 7))
    );
    let metadata =
        crate::inspector::metadata::Metadata::new(snapshot.metadata("test.application")?);
    let overridden = metadata
        .describe(Some(&bevy::reflect::ParsedPath::parse("enabled")?))
        .ok_or("provenance")?;
    assert!(
        overridden.contains("Instance override") && overridden.contains("parent.prefab.ron"),
        "{overridden}"
    );
    let inherited = metadata
        .describe(Some(&bevy::reflect::ParsedPath::parse("exact")?))
        .ok_or("provenance")?;
    assert!(
        inherited.contains("Inherited") && inherited.contains("child.prefab.ron"),
        "{inherited}"
    );
    documents
        .scope
        .as_mut()
        .ok_or("scope")?
        .working
        .transact_checked(
            &BTreeMap::from([(child.id, 1)]),
            |docs| {
                docs.get_mut(&child.id)
                    .ok_or_else(|| kedra_scene::Error("child missing".into()))?
                    .title = "Changed dependency metadata".into();
                Ok(())
            },
            |_| Ok(()),
        )?;
    let before = ron::to_string(documents.scope.as_ref().ok_or("scope")?)?;
    assert!(
        documents.inspect(&target).is_err(),
        "Even visually unchanged dependency edits invalidate admission snapshots"
    );
    assert_eq!(
        ron::to_string(documents.scope.as_ref().ok_or("scope")?)?,
        before
    );
    documents.refresh_inspection(&types, &policies)?;
    assert_eq!(
        documents
            .inspect(&target)?
            .value("test.application", &types, &policies)?
            .try_downcast_ref::<ApplicationValue>()
            .map(|value| (value.enabled, value.exact)),
        Some((false, 7)),
        "A dependency revision refresh must preserve effective overrides"
    );
    assert_eq!(
        ron::to_string(documents.scope.as_ref().ok_or("scope")?)?,
        before
    );
    Ok(())
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct OptionalReference {
    #[entities]
    target: Option<Entity>,
}

#[test]
fn optional_source_ports_are_not_presented_as_authored_none_or_runtime_entity_ids() -> TestResult {
    use bevy::reflect::ReflectPath;
    use kedra_inspector::binding::ValueMetadata;
    let mut types = TypeRegistry::default();
    let mut policies = ComponentPolicies::default();
    policies.register::<OptionalReference>(&mut types, "test.reference", 1, |_| Ok(()))?;
    let mut document = PrefabDocument::new("Optional source reference");
    document.ports.insert("optional".into(), false);
    document.nodes[0].components.insert(
        "test.reference".into(),
        ComponentData::ron(1, "(target:Some(Port(\"optional\")))")?,
    );
    let text = document.serialize()?;
    let source = SourceSnapshot {
        namespace: "game".into(),
        generation: 1,
        documents: vec![SourceFile {
            path: "reference.prefab.ron".into(),
            hash: content_hash(&text),
            source: text,
        }],
        assets: vec![],
        index: None,
    };
    let (scope, compiled) = source::admit(source, &types, &policies)?;
    let documents = Documents {
        compiled_revisions: scope
            .working
            .documents()
            .iter()
            .map(|(id, d)| (*id, d.revision))
            .collect(),
        scope: Some(scope),
        compiled,
        ..Default::default()
    };
    let target = documents
        .target(document.id, document.root.into())
        .ok_or("target")?;
    let snapshot = documents.inspect(&target)?;
    let field = "target".reflect_element(snapshot.value("test.reference", &types, &policies)?)?;
    assert_eq!(field.try_downcast_ref::<Option<Entity>>(), Some(&None));
    assert!(
        crate::inspector::metadata::Metadata::new(snapshot.metadata("test.reference")?)
            .limitation(field, None)
            .is_some()
    );
    assert!(
        snapshot
            .value("test.reference", &types, &ComponentPolicies::default())
            .is_err()
    );
    Ok(())
}
