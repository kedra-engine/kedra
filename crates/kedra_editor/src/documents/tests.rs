use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn document_id(value: u128) -> DocumentId {
    DocumentId(uuid::Uuid::from_u128(value))
}

#[test]
fn admitted_description_distinguishes_in_memory_edits_from_rendering_and_save() -> TestResult {
    let directory = tempfile::tempdir()?;
    write(directory.path(), "main.prefab.ron", 42)?;
    let (scope, _) = source::admit(
        source_files::read(directory.path(), "game")?,
        &bevy::reflect::TypeRegistry::default(),
        &ComponentPolicies::default(),
    )?;
    let documents = Documents {
        scope: Some(scope),
        ..Default::default()
    };
    let (_, description, _) = documents
        .describe(document_id(42))
        .ok_or("Missing admitted description")?;
    assert!(description.contains("Inspector to inspect selected values"));
    assert!(description.contains("Scene View requires a matching verified delivery"));
    assert!(description.contains("Transform can be edited in memory with source Undo/Redo"));
    assert!(description.contains("Save active document writes accepted source changes"));
    assert!(!description.contains("Inspector, rendered preview, Save"));
    assert!(documents.describe(document_id(43)).is_none());
    Ok(())
}
#[test]
fn selection_is_scoped_validated_and_independent_of_document_activation() -> TestResult {
    use kedra_scene::{
        composition::Catalog,
        document::{Node, NodeId, PrefabDocument},
    };
    let mut first = PrefabDocument::new("First");
    let child = NodeId::new();
    first.nodes.push(Node {
        id: child,
        name: "Child".into(),
        parent: Some(first.root.into()),
        components: BTreeMap::new(),
    });
    let second = PrefabDocument::new("Second");
    let mut catalog = Catalog::default();
    catalog.insert(first.clone())?;
    catalog.insert(second.clone())?;
    let mut documents = Documents::default();
    let files = [&first, &second]
        .into_iter()
        .map(|doc| {
            let source = doc.serialize()?;
            Ok(kedra_authoring::persistence::SourceFile {
                path: format!("{}.prefab.ron", doc.title),
                hash: kedra_authoring::persistence::content_hash(&source),
                source,
            })
        })
        .collect::<Result<Vec<_>, kedra_scene::Error>>()?;
    documents.scope = Some(
        source::admit(
            kedra_authoring::persistence::SourceSnapshot {
                namespace: "game".into(),
                generation: 0,
                documents: files,
                assets: vec![],
                index: None,
            },
            &bevy::reflect::TypeRegistry::default(),
            &ComponentPolicies::default(),
        )?
        .0,
    );
    for doc in [&first, &second] {
        let graph = std::sync::Arc::new(catalog.resolve(doc.id)?);
        let ready = Ready {
            document: doc.id,
            path: format!("{}.prefab.ron", doc.title).into(),
            graph,
        };
        documents.published(ready, ViewId::new(), doc.root.into());
    }
    assert!(
        !documents.select(first.id, child.into()),
        "Inactive-document input was admitted"
    );
    let first_view = documents.opened[&first.id].view;
    documents.activate_view(first_view);
    assert!(documents.select(first.id, child.into()));
    assert!(!documents.select(first.id, NodeId::new().into()));
    assert!(!documents.select(
        first.id,
        NodeAddress {
            instances: vec![kedra_scene::document::InstanceId::new()],
            node: child
        }
    ));
    assert_eq!(documents.opened[&first.id].selected, Some(child.into()));
    assert!(documents.select_nodes(first.id, vec![first.root.into(), child.into()]));
    let multiple = documents.selection.clone();
    documents.activate_view(first_view);
    assert_eq!(documents.selection, multiple);
    documents.activate_view(ViewId::new());
    assert_eq!(documents.selection, multiple);
    assert!(documents.select_asset_path("game://First.prefab.ron"));
    let asset = documents.selection.clone();
    documents.activate_view(first_view);
    documents.activate_view(ViewId::new());
    assert_eq!(documents.selection, asset);
    assert!(!documents.select_nodes(first.id, vec![child.into(), NodeId::new().into()]));
    assert_eq!(documents.selection, asset);
    documents.activate_view(documents.opened[&second.id].view);
    assert_eq!(
        documents.opened[&second.id].selected,
        Some(second.root.into())
    );
    documents.activate_view(first_view);
    assert_eq!(documents.opened[&first.id].selected, Some(child.into()));
    documents
        .opened
        .get_mut(&second.id)
        .ok_or("second document")?
        .selected = Some(NodeId::new().into());
    documents.activate_view(documents.opened[&second.id].view);
    assert!(
        documents.selection.items().is_empty(),
        "Invalid remembered node retained another document's selection"
    );
    assert_eq!(catalog.documents[&first.id], first);
    Ok(())
}
fn write(root: &std::path::Path, name: &str, id: u128) -> TestResult {
    let mut doc = kedra_scene::document::PrefabDocument::new(name);
    doc.id = document_id(id);
    std::fs::write(root.join(name), doc.serialize()?)?;
    Ok(())
}
#[test]
fn asset_selection_admits_sources_without_open_and_failed_selection_preserves_context() -> TestResult
{
    bevy::tasks::IoTaskPool::get_or_init(bevy::tasks::TaskPool::new);
    let dir = tempfile::tempdir()?;
    write(dir.path(), "one.prefab.ron", 1)?;
    let before = std::fs::read(dir.path().join("one.prefab.ron"))?;
    let mut world = World::new();
    world.init_resource::<AppTypeRegistry>();
    world.init_resource::<ComponentPolicies>();
    let mut documents = Documents::default();
    documents.select_asset(&world, dir.path().into(), "game://one.prefab.ron".into());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while documents.selection.active().is_none() {
        assert!(
            documents.poll().is_none(),
            "Selection unexpectedly requested Open"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "Selection admission timed out"
        );
        std::thread::yield_now();
    }
    assert!(documents.opened.is_empty());
    assert!(documents.active.is_none());
    assert!(!documents.opening());
    assert!(documents.selection_target().is_some());
    let selected = documents.selection.clone();
    documents.select_asset(
        &world,
        dir.path().into(),
        "game://missing.prefab.ron".into(),
    );
    assert_eq!(documents.selection, selected);
    documents.selection.clear();
    documents.restore_selection(selected);
    assert!(documents.selection.revision() > 1);
    let scope = documents.scope.as_ref().ok_or("scope")?;
    assert!(!scope.working.can_undo());
    assert!(!scope.working.dirty(document_id(1)));
    assert_eq!(std::fs::read(dir.path().join("one.prefab.ron"))?, before);
    Ok(())
}
#[test]
fn source_admission_keeps_one_workspace_and_rejects_collisions_without_writes() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(dir.path(), "one.prefab.ron", 1)?;
    write(dir.path(), "two.prefab.ron", 2)?;
    let types = bevy::reflect::TypeRegistry::default();
    let policies = ComponentPolicies::default();
    let before = std::fs::read(dir.path().join("one.prefab.ron"))?;
    let source = source_files::read(dir.path(), "game")?;
    let (scope, _) = source::admit(source, &types, &policies)?;
    assert_eq!(scope.working.documents().len(), 2);
    assert_eq!(scope.working.documents().len(), 2);
    assert!(!scope.working.can_undo());
    assert!(!dir.path().join("catalog.prefabs.ron").exists());
    assert!(!dir.path().join("one.prefab.ron.meta").exists());
    write(dir.path(), "duplicate.prefab.ron", 1)?;
    assert!(
        source_files::read(dir.path(), "game")
            .and_then(|snapshot| source::admit(snapshot, &types, &policies))
            .is_err()
    );
    assert_eq!(std::fs::read(dir.path().join("one.prefab.ron"))?, before);
    assert_eq!(scope.working.documents().len(), 2);
    Ok(())
}
#[test]
fn malformed_missing_and_mismatched_sources_fail_before_publishing() -> TestResult {
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join("broken.scene.ron"), "invalid")?;
    assert!(source_files::read(dir.path(), "game").is_err());
    std::fs::remove_file(dir.path().join("broken.scene.ron"))?;
    write(dir.path(), "one.prefab.ron", 1)?;
    std::fs::write(
        dir.path().join("one.prefab.ron.meta"),
        "(id:\"00000000-0000-0000-0000-000000000002\",meta_format_version:\"1.0\",asset:Load(loader:\"kedra_scene::assets::DocumentLoader\",settings:()))",
    )?;
    assert!(source_files::read(dir.path(), "game").is_err());
    assert!(source_files::read(&dir.path().join("missing"), "game").is_err());
    Ok(())
}
#[test]
fn repeated_open_preserves_selection_and_latest_locator_wins() -> TestResult {
    bevy::tasks::IoTaskPool::get_or_init(bevy::tasks::TaskPool::new);
    let dir = tempfile::tempdir()?;
    write(dir.path(), "one.prefab.ron", 1)?;
    write(dir.path(), "two.prefab.ron", 2)?;
    let (scope, compiled) = source::admit(
        source_files::read(dir.path(), "game")?,
        &bevy::reflect::TypeRegistry::default(),
        &ComponentPolicies::default(),
    )?;
    let mut documents = Documents {
        paths: scope
            .working
            .documents()
            .keys()
            .filter_map(|id| scope.path(*id).map(|path| (PathBuf::from(path), *id)))
            .collect(),
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
    let mut world = World::new();
    world.init_resource::<AppTypeRegistry>();
    world.init_resource::<ComponentPolicies>();
    documents.request(&world, dir.path().into(), "one.prefab.ron".into())?;
    documents.request(&world, dir.path().into(), "two.prefab.ron".into())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let ready = loop {
        if let Some(ready) = documents.poll() {
            break ready;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    };
    assert_eq!(ready.document, document_id(2));
    let (_, _, root) = documents.describe(ready.document).ok_or("root")?;
    let view = ViewId::new();
    documents.published(ready, view, root.clone());
    std::fs::remove_file(dir.path().join("two.prefab.ron"))?;
    documents.request(&world, dir.path().into(), "two.prefab.ron".into())?;
    let ready = documents.poll().ok_or("ready")?;
    documents.published(ready, ViewId::new(), root.clone());
    assert_eq!(documents.opened.len(), 1);
    assert_eq!(documents.opened[&document_id(2)].view, view);
    assert_eq!(documents.opened[&document_id(2)].selected, Some(root));
    assert!(
        documents
            .request(&world, dir.path().into(), "new.prefab.ron".into())
            .is_err()
    );
    assert!(documents.pending.is_none());
    assert_eq!(documents.active, Some(document_id(2)));
    Ok(())
}
