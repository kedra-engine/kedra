//! Save acknowledgment belongs to the document host; byte replacement belongs to fs.
use super::*;
use bevy::tasks::block_on;
use kedra_authoring::properties::PropertyEdit;
use kedra_scene::document::{ComponentData, PrefabDocument};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn save_all_covers_independent_documents_without_changing_selection_and_stops_on_refusal()
-> TestResult {
    for refuse in [false, true] {
        let Fixture {
            app,
            mut documents,
            root,
            directory: _directory,
            ..
        } = fixture_with_peer(true)?;
        let active = documents.active;
        let registry = app.world().resource::<AppTypeRegistry>().clone();
        let types = registry.read();
        let policies = app.world().resource::<ComponentPolicies>();
        for path in ["game://source.prefab.ron", "game://peer.prefab.ron"] {
            let mut edit = documents.begin_edit("Rename", &[path.into()])?;
            let scope = documents.scope.as_ref().ok_or("scope")?;
            let node = edit.nodes(scope, path)?.remove(0);
            edit.rename(scope, &node, "Accepted name", &types, policies)?;
            documents.commit_edit(&mut edit, &types, policies)?;
        }
        let before: Vec<_> = ["source.prefab.ron", "peer.prefab.ron"]
            .iter()
            .map(|p| std::fs::read(root.join(p)))
            .collect::<Result<_, _>>()?;
        if refuse {
            std::fs::write(root.join("source.prefab.ron"), "foreign bytes")?;
        }
        documents.begin_save_all(app.world(), root.clone())?;
        while documents.save_busy() {
            finish(&mut documents)?;
            documents.advance_save_all(app.world(), root.clone())?;
        }
        assert_eq!(documents.active, active);
        if refuse {
            assert!(documents.save_error().is_some());
            assert!(!documents.update_clean());
            assert_eq!(
                std::fs::read(root.join("source.prefab.ron"))?,
                b"foreign bytes"
            );
            assert_eq!(std::fs::read(root.join("peer.prefab.ron"))?, before[1]);
        } else {
            assert!(documents.update_clean());
            for path in ["source.prefab.ron", "peer.prefab.ron"] {
                let saved = PrefabDocument::parse(&std::fs::read_to_string(root.join(path))?)?;
                let mut catalog = kedra_scene::composition::Catalog::default();
                catalog.insert(saved.clone())?;
                assert_eq!(
                    catalog.resolve(saved.id)?.nodes[&saved.root.into()].name,
                    "Accepted name"
                );
            }
        }
    }
    Ok(())
}
#[test]
fn new_scene_save_reopen_and_history_removal_share_the_existing_document_coordinator() -> TestResult
{
    creation_save(
        kedra_scene::document::DocumentKind::Scene,
        "Arena.scene.ron",
    )
}
#[test]
fn new_prefab_save_reopen_and_history_removal_share_the_existing_document_coordinator() -> TestResult
{
    creation_save(
        kedra_scene::document::DocumentKind::Prefab,
        "Bomb.prefab.ron",
    )
}
fn creation_save(kind: kedra_scene::document::DocumentKind, path: &str) -> TestResult {
    let Fixture {
        app,
        mut documents,
        root,
        directory: _directory,
        ..
    } = fixture()?;
    let original = std::fs::read(root.join("source.prefab.ron"))?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let policies = app.world().resource::<ComponentPolicies>();
    let mut scene = PrefabDocument::new("Arena");
    scene.kind = kind;
    let id = scene.id;
    let compiled = source::create_document(
        documents.scope.as_mut().ok_or("scope")?,
        scene,
        path.into(),
        &types,
        policies,
    )?;
    documents.compiled = compiled;
    documents.compiled_revisions = documents.revisions();
    documents.active = Some(id);
    documents.begin_save(app.world(), root.clone())?;
    let message = finish(&mut documents)?;
    assert!(
        !documents.save_pending() && documents.update_clean(),
        "{message}"
    );
    let saved = std::fs::read_to_string(root.join(path))?;
    assert_eq!(PrefabDocument::parse(&saved)?.kind, kind);
    let (reopened, _) = source::admit(source_files::read(&root, "game")?, &types, policies)?;
    assert_eq!(reopened.path(id), Some(path));
    assert!(!reopened.working.is_dirty());
    let change = documents.prepare_history(false, &types, policies)?;
    documents.publish_change(change);
    assert!(!documents.contains(id));
    assert!(!documents.update_clean());
    assert!(documents.can_save());
    assert_eq!(
        std::fs::read_to_string(root.join(path))?,
        saved,
        "Undo must not write files"
    );
    documents.begin_save(app.world(), root.clone())?;
    let message = finish(&mut documents)?;
    assert!(documents.update_clean(), "{message}");
    assert!(!root.join(path).exists());
    let change = documents.prepare_history(true, &types, policies)?;
    documents.publish_change(change);
    assert!(documents.can_save());
    documents.begin_save(app.world(), root.clone())?;
    let message = finish(&mut documents)?;
    assert!(documents.update_clean(), "{message}");
    assert_eq!(std::fs::read_to_string(root.join(path))?, saved);
    assert_eq!(std::fs::read(root.join("source.prefab.ron"))?, original);
    Ok(())
}
struct Fixture {
    app: App,
    documents: Documents,
    target: inspection::Target,
    directory: tempfile::TempDir,
    root: PathBuf,
}
fn fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
    fixture_with_peer(false)
}
fn fixture_with_peer(peer: bool) -> Result<Fixture, Box<dyn std::error::Error>> {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<AppTypeRegistry>()
        .init_resource::<ComponentPolicies>();
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    kedra_scene::spatial::register(
        &mut registry.write(),
        &mut app.world_mut().resource_mut::<ComponentPolicies>(),
    )?;
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("assets");
    std::fs::create_dir(&root)?;
    let mut doc = PrefabDocument::new("Saved source");
    doc.nodes[0].components.insert(
        "scene.transform".into(),
        ComponentData::ron(
            1,
            "(translation:(0.0,0.0,0.0),rotation:(0.0,0.0,0.0,1.0),scale:(1.0,1.0,1.0))",
        )?,
    );
    std::fs::write(root.join("source.prefab.ron"), doc.serialize()?)?;
    if peer {
        let mut second = doc.clone();
        second.id = kedra_scene::document::DocumentId::new();
        second.title = "Peer source".into();
        std::fs::write(root.join("peer.prefab.ron"), second.serialize()?)?;
    }
    let (scope, compiled) = source::admit(
        source_files::read(&root, "game")?,
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
    )?;
    let mut documents = Documents {
        active: Some(doc.id),
        scope: Some(scope),
        compiled,
        ..default()
    };
    documents.compiled_revisions = documents.revisions();
    let target = documents.target(doc.id, doc.root.into()).ok_or("target")?;
    Ok(Fixture {
        app,
        documents,
        target,
        directory,
        root,
    })
}

#[test]
fn source_batch_save_publishes_every_member_and_acknowledges_only_the_pinned_group() -> TestResult {
    let Fixture {
        app,
        mut documents,
        target,
        directory: _directory,
        root,
    } = fixture_with_peer(true)?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let policies = app.world().resource::<ComponentPolicies>();
    let mut batch = documents.begin_edit(
        "Rename prefab pair",
        &[
            "game://source.prefab.ron".into(),
            "game://peer.prefab.ron".into(),
        ],
    )?;
    let before: Vec<_> = ["source.prefab.ron", "peer.prefab.ron"]
        .into_iter()
        .map(|path| std::fs::read(root.join(path)))
        .collect::<Result<_, _>>()?;
    for path in ["source.prefab.ron", "peer.prefab.ron"] {
        let node = batch
            .nodes(
                documents.scope.as_ref().ok_or("scope")?,
                &format!("game://{path}"),
            )?
            .remove(0);
        batch.rename(
            documents.scope.as_ref().ok_or("scope")?,
            &node,
            "Batch accepted",
            &types,
            policies,
        )?;
    }
    documents.commit_edit(&mut batch, &types, policies)?;
    for (path, before) in ["source.prefab.ron", "peer.prefab.ron"]
        .into_iter()
        .zip(&before)
    {
        assert_eq!(&std::fs::read(root.join(path))?, before);
    }
    documents.begin_save(app.world(), root.clone())?;
    assert_eq!(
        documents
            .scope
            .as_ref()
            .and_then(Workspace::pending)
            .ok_or("request")?
            .files
            .len(),
        2
    );
    edit(&app, &mut documents, &target, 9.)?;
    let message = finish(&mut documents)?;
    assert!(
        !message.contains("refused") && !message.contains("unresolved"),
        "{message}"
    );
    let scope = documents.scope.as_ref().ok_or("scope")?;
    assert!(scope.working.dirty(target.document));
    assert_eq!(
        scope
            .working
            .documents()
            .values()
            .filter(|doc| doc.revision != doc.saved_revision)
            .count(),
        1
    );
    for file in &scope.source.documents {
        assert_eq!(std::fs::read_to_string(root.join(&file.path))?, file.source);
    }
    let (_, saved) = source::admit(scope.source.clone(), &types, policies)?;
    assert!(saved.values().all(|compiled| {
        compiled
            .graph()
            .nodes
            .values()
            .any(|node| node.name == "Batch accepted")
    }));
    assert!(documents.can_history(false));
    assert!(!documents.save_pending());
    assert!(!root.join("catalog.prefabs.ron").exists());
    let peer = documents
        .scope
        .as_ref()
        .ok_or("scope")?
        .working
        .documents()
        .keys()
        .find(|id| **id != target.document)
        .copied()
        .ok_or("peer")?;
    documents.active = Some(peer);
    assert!(
        documents.can_save(),
        "a clean active member can still own a dirty coupled group"
    );
    documents.begin_save(app.world(), root.clone())?;
    let message = finish(&mut documents)?;
    assert!(
        !message.contains("refused") && !message.contains("unresolved"),
        "{message}"
    );
    assert!(documents.update_clean());
    Ok(())
}
fn edit(
    app: &App,
    documents: &mut Documents,
    target: &inspection::Target,
    x: f32,
) -> Result<(), String> {
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let value = Vec3::new(x, 0., 0.);
    let edit = PropertyEdit {
        path: bevy::reflect::ParsedPath::parse("translation").map_err(|error| error.to_string())?,
        value: &value,
    };
    let change = documents.prepare_property(
        target,
        &documents.revisions(),
        "scene.transform",
        &[edit],
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
    )?;
    documents.publish_change(change);
    Ok(())
}
fn finish(documents: &mut Documents) -> Result<String, Box<dyn std::error::Error>> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(result) = documents.poll_save() {
            return Ok(result);
        }
        if std::time::Instant::now() >= deadline {
            return Err("Save worker did not finish".into());
        }
        std::thread::yield_now();
    }
}
#[test]
fn async_save_acknowledges_only_submitted_revision_and_keeps_newer_source_and_history() -> TestResult
{
    let Fixture {
        app,
        mut documents,
        target,
        directory: _directory,
        root,
    } = fixture()?;
    edit(&app, &mut documents, &target, 3.)?;
    documents.begin_save(app.world(), root.clone())?;
    assert!(documents.save_pending());
    assert!(!documents.update_clean());
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    assert!(
        documents
            .prepare_discard(
                &registry.read(),
                app.world().resource::<ComponentPolicies>()
            )
            .is_err()
    );
    edit(&app, &mut documents, &target, 9.)?;
    finish(&mut documents)?;
    assert!(!documents.save_pending());
    assert!(!documents.update_clean());
    assert!(documents.can_history(false));
    let baseline = documents.scope.as_ref().ok_or("scope")?;
    assert_eq!(
        std::fs::read_to_string(root.join("source.prefab.ron"))?,
        baseline.source.documents[0].source
    );
    let disk = PrefabDocument::parse(&baseline.source.documents[0].source)?;
    assert_ne!(
        disk.serialize()?,
        baseline.working.documents()[&target.document]
            .document
            .serialize()?
    );
    let current = documents.inspect(&target)?;
    assert_eq!(
        current
            .value(
                "scene.transform",
                &registry.read(),
                app.world().resource::<ComponentPolicies>()
            )?
            .try_downcast_ref::<Transform>()
            .ok_or("Transform")?
            .translation
            .x,
        9.
    );
    let (_, saved) = source::admit(
        baseline.source.clone(),
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
    )?;
    let saved = &saved[&target.document];
    let entity = saved
        .source_entities()
        .get(&target.node)
        .ok_or("saved node")?;
    assert_eq!(
        saved
            .authoring_world()
            .get::<Transform>(*entity)
            .ok_or("saved Transform")?
            .translation
            .x,
        3.
    );
    documents.begin_save(app.world(), root.clone())?;
    finish(&mut documents)?;
    assert!(documents.update_clean());
    assert!(documents.can_history(false));
    assert!(!root.join("catalog.prefabs.ron").exists());
    Ok(())
}
#[test]
fn external_source_conflict_refuses_save_without_losing_working_changes_or_history() -> TestResult {
    let Fixture {
        app,
        mut documents,
        target,
        directory: _directory,
        root,
    } = fixture()?;
    edit(&app, &mut documents, &target, 5.)?;
    let before = documents.revisions();
    let path = root.join("source.prefab.ron");
    let mut external = PrefabDocument::parse(&std::fs::read_to_string(&path)?)?;
    external.title = "External editor".into();
    let external = external.serialize()?;
    std::fs::write(&path, &external)?;
    documents.begin_save(app.world(), root)?;
    assert!(finish(&mut documents)?.contains("Save refused"));
    assert_eq!(std::fs::read_to_string(&path)?, external);
    assert_eq!(documents.revisions(), before);
    assert!(!documents.update_clean());
    assert!(!documents.save_pending());
    assert!(documents.can_history(false));
    Ok(())
}

#[test]
fn unresolved_receipt_retains_exact_request_until_same_byte_retry_is_acknowledged() -> TestResult {
    let Fixture {
        app,
        mut documents,
        target,
        directory: _directory,
        root,
    } = fixture()?;
    edit(&app, &mut documents, &target, 7.)?;
    documents.begin_save(app.world(), root.clone())?;
    let request = documents
        .scope
        .as_ref()
        .and_then(Workspace::pending)
        .ok_or("pending")?
        .clone();
    // Consume the real durable writer result without delivering its receipt to
    // Documents, then deliver the transport/host's uncertain completion instead.
    let task = documents.save.task.take().ok_or("task")?;
    block_on(task)?;
    documents.save.task = Some(Wakeup::default().spawn(IoTaskPool::get(), async {
        Err(SaveError {
            uncertain: true,
            message: "Receipt delivery was interrupted".into(),
        })
    }));
    assert!(finish(&mut documents)?.contains("unresolved"));
    assert!(documents.save_pending());
    assert!(!documents.update_clean());
    let before = std::fs::read(root.join("source.prefab.ron"))?;
    documents.begin_save(app.world(), root.clone())?;
    assert_eq!(
        documents
            .scope
            .as_ref()
            .and_then(Workspace::pending)
            .ok_or("pending")?
            .operation,
        request.operation
    );
    finish(&mut documents)?;
    assert_eq!(std::fs::read(root.join("source.prefab.ron"))?, before);
    assert!(documents.update_clean());
    assert!(documents.can_history(false));
    Ok(())
}
