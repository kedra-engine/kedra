use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn document_id(value: u128) -> DocumentId {
    DocumentId(uuid::Uuid::from_u128(value))
}

#[test]
fn admitted_description_distinguishes_read_only_inspection_from_rendering_and_save() -> TestResult {
    let directory = tempfile::tempdir()?;
    write(directory.path(), "main.prefab.ron", 42)?;
    let (scope, _) = source::admit(
        source::read(directory.path())?,
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
    assert!(description.contains("Save and source edits are unavailable"));
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
    documents.activate_view(documents.opened[&second.id].view);
    assert_eq!(
        documents.opened[&second.id].selected,
        Some(second.root.into())
    );
    documents.activate_view(first_view);
    assert_eq!(documents.opened[&first.id].selected, Some(child.into()));
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
fn source_admission_keeps_one_workspace_and_rejects_collisions_without_writes() -> TestResult {
    let dir = tempfile::tempdir()?;
    write(dir.path(), "one.prefab.ron", 1)?;
    write(dir.path(), "two.prefab.ron", 2)?;
    let types = bevy::reflect::TypeRegistry::default();
    let policies = ComponentPolicies::default();
    let before = std::fs::read(dir.path().join("one.prefab.ron"))?;
    let source = source::read(dir.path())?;
    let (scope, _) = source::admit(source, &types, &policies)?;
    assert_eq!(scope.working.documents().len(), 2);
    assert_eq!(source::paths(&scope).len(), 2);
    assert!(!scope.working.can_undo());
    assert!(!dir.path().join("catalog.prefabs.ron").exists());
    assert!(!dir.path().join("one.prefab.ron.meta").exists());
    write(dir.path(), "duplicate.prefab.ron", 1)?;
    assert!(
        source::read(dir.path())
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
    assert!(source::read(dir.path()).is_err());
    std::fs::remove_file(dir.path().join("broken.scene.ron"))?;
    write(dir.path(), "one.prefab.ron", 1)?;
    std::fs::write(
        dir.path().join("one.prefab.ron.meta"),
        "(id:\"00000000-0000-0000-0000-000000000002\",meta_format_version:\"1.0\",asset:Load(loader:\"kedra_scene::assets::DocumentLoader\",settings:()))",
    )?;
    assert!(source::read(dir.path()).is_err());
    assert!(source::read(&dir.path().join("missing")).is_err());
    Ok(())
}
#[test]
fn repeated_open_preserves_selection_and_latest_locator_wins() -> TestResult {
    bevy::tasks::IoTaskPool::get_or_init(bevy::tasks::TaskPool::new);
    let dir = tempfile::tempdir()?;
    write(dir.path(), "one.prefab.ron", 1)?;
    write(dir.path(), "two.prefab.ron", 2)?;
    let (scope, compiled) = source::admit(
        source::read(dir.path())?,
        &bevy::reflect::TypeRegistry::default(),
        &ComponentPolicies::default(),
    )?;
    let mut documents = Documents {
        paths: source::paths(&scope),
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
