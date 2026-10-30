//! Authoring is a source owner, not a gameplay snapshot or an implicit file writer.
use bevy::{asset::AssetPlugin, prelude::*};
use kedra_authoring::{
    overrides::ApplyField, persistence::*, unpack::Unpack, workspace::WorkingSet,
};
use kedra_scene::{
    assets::compile_catalog, composition::Catalog, document::*, reflection::ComponentPolicies,
};
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
struct Settings {
    speed: f32,
}

fn setup() -> Result<(App, ComponentPolicies)> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_resource::<AppTypeRegistry>();
    let mut policies = ComponentPolicies::default();
    policies.register::<Settings>(
        &mut app.world().resource::<AppTypeRegistry>().write(),
        "game.settings",
        1,
        |_| Ok(()),
    )?;
    Ok((app, policies))
}

fn source() -> Result<PrefabDocument> {
    let mut source = PrefabDocument::new("Actor");
    source.nodes[0].components.insert(
        "game.settings".into(),
        ComponentData::ron(1, "(speed:1.0)")?,
    );
    Ok(source)
}

fn baseline(documents: &[PrefabDocument]) -> Result<SourceSnapshot> {
    let documents = documents
        .iter()
        .map(|document| {
            let source = document.serialize()?;
            Ok(SourceFile {
                path: format!("{}.prefab.ron", document.id),
                hash: content_hash(&source),
                source,
            })
        })
        .collect::<kedra_scene::Result<Vec<_>>>()?;
    Ok(SourceSnapshot {
        environments: Default::default(),
        namespace: "game".into(),
        generation: 4,
        documents,
        assets: Vec::new(),
        index: None,
    })
}

#[test]
fn failed_and_stale_multi_document_edits_preserve_sources_and_history() -> Result<()> {
    let a = source()?;
    let b = source()?;
    let mut working = WorkingSet::default();
    working.open(a.clone())?;
    working.open(b.clone())?;
    let expected = BTreeMap::from([(a.id, 1), (b.id, 1)]);
    let reject = working.transact_checked(
        &expected,
        |documents| {
            documents
                .get_mut(&a.id)
                .ok_or_else(|| kedra_scene::Error("missing source".into()))?
                .title = "Candidate".into();
            Ok(())
        },
        |_| Err(kedra_scene::Error("rejected by schema".into())),
    );
    assert!(reject.is_err());
    assert!(!working.can_undo());
    assert_eq!(working.documents()[&a.id].document, a);
    assert!(
        working
            .transact_checked(
                &BTreeMap::from([(a.id, 1)]),
                |documents| {
                    documents
                        .get_mut(&b.id)
                        .ok_or_else(|| kedra_scene::Error("missing source".into()))?
                        .title = "Unprotected edit".into();
                    Ok(())
                },
                |_| Ok(())
            )
            .is_err()
    );
    working.transact_checked(
        &expected,
        |documents| {
            for document in documents.values_mut() {
                document.title = "Edited".into();
            }
            Ok(())
        },
        |_| Ok(()),
    )?;
    assert_eq!(working.save_group(a.id).len(), 2);
    assert!(
        working
            .transact_checked(&expected, |_| Ok(()), |_| Ok(()))
            .is_err()
    );
    assert!(
        working
            .undo_checked(|_| Err(kedra_scene::Error("undo rejected".into())))
            .is_err()
    );
    assert_eq!(working.documents()[&a.id].document.title, "Edited");
    working.undo_checked(|_| Ok(()))?;
    assert_eq!(working.documents()[&a.id].document, a);
    working.redo_checked(|_| Ok(()))?;
    assert_eq!(working.documents()[&a.id].document.title, "Edited");
    assert!(working.documents()[&a.id].revision > 2);
    Ok(())
}

#[test]
fn save_receipt_is_exact_and_does_not_acknowledge_newer_edits() -> Result<()> {
    let (app, policies) = setup()?;
    let source = source()?;
    let id = source.id;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let mut workspace =
        Workspace::open(baseline(&[source])?, &types.read(), &policies, &mut loader);
    assert!(workspace.diagnostic.is_none());
    workspace.working.transact_checked(
        &BTreeMap::from([(id, 1)]),
        |documents| {
            documents
                .get_mut(&id)
                .ok_or_else(|| kedra_scene::Error("missing source".into()))?
                .title = "Submitted".into();
            Ok(())
        },
        |_| Ok(()),
    )?;
    let request = workspace
        .prepare_save(Some(id), &types.read(), &policies, &mut loader)?
        .clone();
    assert!(
        workspace
            .prepare_save(Some(id), &types.read(), &policies, &mut loader)
            .is_err()
    );
    let mut receipt = Receipt {
        operation: request.operation,
        generation: 5,
        files: request
            .files
            .iter()
            .map(|file| SavedFile {
                document: file.document,
                path: file.path.clone(),
                revision: file.revision,
                hash: file.source.as_deref().map(content_hash),
            })
            .collect(),
    };
    receipt.files[0].hash = Some("not the committed bytes".into());
    assert!(workspace.acknowledge(&receipt).is_err());
    assert!(workspace.pending().is_some());
    receipt.files[0].hash = request.files[0].source.as_deref().map(content_hash);
    let revision = workspace.working.documents()[&id].revision;
    workspace.working.transact_checked(
        &BTreeMap::from([(id, revision)]),
        |documents| {
            documents
                .get_mut(&id)
                .ok_or_else(|| kedra_scene::Error("missing source".into()))?
                .title = "Newer edit".into();
            Ok(())
        },
        |_| Ok(()),
    )?;
    workspace.acknowledge(&receipt)?;
    workspace.acknowledge(&receipt)?;
    assert!(workspace.working.dirty(id));
    assert!(workspace.pending().is_none());
    assert!(workspace.source.documents[0].source.contains("Submitted"));
    assert!(!workspace.source.documents[0].source.contains("Newer edit"));
    assert_eq!(workspace.source.generation, 5);
    let index = workspace
        .source
        .index
        .as_ref()
        .ok_or("catalog was not saved")?;
    let paths: Vec<String> = ron::from_str(&index.source)?;
    assert_eq!(paths, vec![format!("game://{id}.prefab.ron")]);
    assert!(!workspace.index_dirty());
    let reopened = Workspace::open(
        workspace.source.clone(),
        &types.read(),
        &policies,
        &mut loader,
    );
    assert!(reopened.diagnostic.is_none());
    Ok(())
}

#[test]
fn catalog_baseline_rejects_unscoped_references_and_namespace_changes() -> Result<()> {
    let (app, policies) = setup()?;
    let document = source()?;
    let mut original = baseline(&[document])?;
    let index = ron::to_string(&vec![original.documents[0].path.clone()])?;
    original.index = Some(SourceFile {
        path: "catalog.prefabs.ron".into(),
        hash: content_hash(&index),
        source: index,
    });
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let rejected = Workspace::open(original.clone(), &types.read(), &policies, &mut loader);
    assert!(rejected.diagnostic.is_some());
    original.index = None;
    let mut workspace = Workspace::open(original.clone(), &types.read(), &policies, &mut loader);
    original.namespace = "different-source".into();
    assert!(
        workspace
            .keep_working(original, &types.read(), &policies, &mut loader)
            .is_err()
    );
    assert_eq!(workspace.source.namespace, "game");
    Ok(())
}

#[test]
fn undecodable_baseline_is_retained_and_cannot_be_saved_as_a_replacement() -> Result<()> {
    let (app, policies) = setup()?;
    let mut baseline = baseline(&[source()?])?;
    baseline.documents[0].source = "invalid source".into();
    baseline.documents[0].hash = content_hash("invalid source");
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let mut workspace = Workspace::open(baseline, &types.read(), &policies, &mut loader);
    assert!(workspace.diagnostic.is_some());
    assert!(workspace.working.documents().is_empty());
    assert_eq!(workspace.source.documents[0].source, "invalid source");
    assert!(
        workspace
            .prepare_save(None, &types.read(), &policies, &mut loader)
            .is_err()
    );
    Ok(())
}

#[test]
fn apply_component_changes_sources_atomically_and_revert_restores_inheritance() -> Result<()> {
    let (app, policies) = setup()?;
    let base = source()?;
    let mut variant = base.variant("Variant");
    variant.overrides.push(Operation::SetComponent {
        node: base.root.into(),
        component: "game.settings".into(),
        data: ComponentData::ron(1, "(speed:3.0)")?,
    });
    let mut working = WorkingSet::default();
    working.open(base.clone())?;
    working.open(variant.clone())?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
    working.apply_component(
        ApplyField {
            origin: variant.id,
            target: base.id,
            node: base.root.into(),
            component: "game.settings".into(),
            path: String::new(),
            expected: BTreeMap::from([(base.id, 1), (variant.id, 1)]),
        },
        &snapshots[&variant.id],
        &policies,
        &types.read(),
        |catalog| compile_catalog(catalog, &types.read(), &policies, &mut loader).map(|_| ()),
    )?;
    assert!(
        working.documents()[&variant.id]
            .document
            .overrides
            .is_empty()
    );
    assert_eq!(working.save_group(base.id).len(), 2);
    working.undo_checked(|_| Ok(()))?;
    assert_eq!(working.documents()[&base.id].document, base);
    let revision = working.documents()[&variant.id].revision;
    working.revert_component(
        variant.id,
        revision,
        &base.root.into(),
        "game.settings",
        |_| Ok(()),
    )?;
    let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
    let node = snapshots[&variant.id].source_entities()[&base.root.into()];
    assert_eq!(
        snapshots[&variant.id]
            .authoring_world()
            .get::<Settings>(node)
            .ok_or("settings missing")?
            .speed,
        1.
    );
    Ok(())
}

#[test]
fn complete_unpack_preserves_other_instances_and_has_reversible_selection_mapping() -> Result<()> {
    let (app, policies) = setup()?;
    let prefab = source()?;
    let mut scene = PrefabDocument::new("Scene");
    scene.kind = DocumentKind::Scene;
    let first = InstanceId::new();
    let second = InstanceId::new();
    for id in [first, second] {
        scene.instances.push(NestedInstance {
            id,
            asset: prefab.id,
            parent: scene.root.into(),
            bindings: BTreeMap::new(),
            overrides: Vec::new(),
        });
    }
    let mut working = WorkingSet::default();
    working.open(prefab.clone())?;
    working.open(scene.clone())?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let snapshots = compile_catalog(&working.catalog(), &types.read(), &policies, &mut loader)?;
    let mapping = working.unpack_complete(
        Unpack {
            document: scene.id,
            revision: 1,
            scope: vec![first],
        },
        &snapshots[&scene.id],
        &policies,
        &types.read(),
        |catalog| compile_catalog(catalog, &types.read(), &policies, &mut loader).map(|_| ()),
    )?;
    assert_eq!(mapping.len(), 1);
    let graph = working.catalog().resolve(scene.id)?;
    assert!(graph.frames.contains_key(&vec![second]));
    assert!(!graph.frames.contains_key(&vec![first]));
    assert!(graph.nodes.contains_key(
        &mapping[&NodeAddress {
            instances: vec![first],
            node: prefab.root
        }]
    ));
    working.undo_checked(|_| Ok(()))?;
    assert_eq!(working.documents()[&scene.id].document, scene);
    assert_eq!(
        working
            .last_mapping
            .as_ref()
            .ok_or("undo mapping missing")?
            .1
            .len(),
        1
    );
    working.redo_checked(|_| Ok(()))?;
    assert_eq!(
        working
            .last_mapping
            .as_ref()
            .ok_or("redo mapping missing")?
            .1,
        mapping
    );
    Ok(())
}

#[test]
fn catalog_keys_must_not_be_changed_independently_from_document_identity() -> Result<()> {
    let source = source()?;
    let mut catalog = Catalog::default();
    catalog.insert(source.clone())?;
    catalog.documents.insert(DocumentId::new(), source);
    assert!(
        catalog
            .resolve(*catalog.documents.keys().next().ok_or("catalog empty")?)
            .is_err()
    );
    Ok(())
}
