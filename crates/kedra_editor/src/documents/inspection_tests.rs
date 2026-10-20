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
    assert!(state.follow(Some(target.clone()))?);
    let token = state.token;
    assert!(!state.follow(Some(target.clone()))?);
    state.toggle_lock();
    assert!(state.locked);
    let next = Target {
        document: DocumentId::new(),
        ..target.clone()
    };
    assert!(!state.follow(Some(next.clone()))?);
    assert_eq!(state.target.as_ref(), Some(&target));
    assert_eq!(state.token, token);
    state.toggle_lock();
    assert!(state.follow(Some(next))?);
    assert_ne!(state.token, token);
    assert!(state.follow(None)?);
    assert!(state.target.is_none());
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
    let metadata = snapshot.metadata("test.application")?;
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
        snapshot
            .metadata("test.reference")?
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
