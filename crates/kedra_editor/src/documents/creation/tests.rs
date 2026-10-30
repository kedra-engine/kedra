//! Editor admission and shared history preserve unsaved scenes and rejected candidates.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn creation_publishes_scene_and_prefab_before_open_and_collision_preserves_file() -> TestResult {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<AppTypeRegistry>()
        .init_resource::<ComponentPolicies>();
    let directory = tempfile::tempdir()?;
    for (kind, name) in [
        (DocumentKind::Scene, "Arena.scene.ron"),
        (DocumentKind::Prefab, "Bomb.prefab.ron"),
    ] {
        let mut documents = Documents::default();
        documents.begin_document(app.world(), directory.path().into(), name.into(), kind)?;
        assert!(documents.poll().is_none());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while documents.creating() {
            documents.poll_save();
            documents.poll_creation(app.world());
            if documents.creating() {
                assert!(documents.poll().is_none());
            }
            if std::time::Instant::now() >= deadline {
                return Err("Creation deadline exceeded".into());
            }
            std::thread::yield_now();
        }
        assert!(documents.error.is_none(), "{:?}", documents.error);
        let bytes = std::fs::read_to_string(directory.path().join(name))?;
        assert_eq!(PrefabDocument::parse(&bytes)?.kind, kind);
        let ready = documents.poll().ok_or("Saved document did not open")?;
        assert!(
            !documents
                .scope
                .as_ref()
                .ok_or("No scope")?
                .working
                .dirty(ready.document)
        );
        let mut rejected = Documents::default();
        rejected.begin_document(app.world(), directory.path().into(), name.into(), kind)?;
        while rejected.creating() {
            rejected.poll_creation(app.world());
            if std::time::Instant::now() >= deadline {
                return Err("Collision deadline exceeded".into());
            }
            std::thread::yield_now();
        }
        assert!(rejected.error.is_some());
        assert!(rejected.poll().is_none());
        assert_eq!(std::fs::read_to_string(directory.path().join(name))?, bytes);
    }
    Ok(())
}
#[derive(bevy::prelude::Component, bevy::prelude::Reflect, Default)]
#[reflect(Component, Default)]
struct GameHealth {
    hit_points: u32,
}
#[derive(bevy::prelude::Component, bevy::prelude::Reflect)]
#[reflect(Component)]
struct RequiredGameValue {
    value: String,
}
use bevy::{ecs::reflect::ReflectComponent, reflect::std_traits::ReflectDefault};

#[test]
fn registered_game_component_is_discovered_without_editor_list_and_stale_commands_refuse()
-> TestResult {
    let mut types = bevy::reflect::TypeRegistry::default();
    let mut policies = ComponentPolicies::default();
    policies.register::<GameHealth>(&mut types, "game.health", 1, |_| Ok(()))?;
    policies.register::<RequiredGameValue>(&mut types, "game.required", 1, |_| Ok(()))?;
    let directory = tempfile::tempdir()?;
    let admission = source::admit(
        source_files::read(directory.path(), "game")?,
        &types,
        &policies,
    )?;
    let mut documents = Documents::default();
    documents.create_document(
        DocumentKind::Scene,
        "Arena.scene.ron".into(),
        Some(admission),
        &types,
        &policies,
    )?;
    let ready = documents.poll().ok_or("Created source missing")?;
    let id = ready.document;
    let root = ready.graph.root.clone();
    documents.published(ready, ViewId::new(), root.clone());
    let target = documents.target(id, root.clone()).ok_or("Target missing")?;
    let required = documents
        .component_commands(&target, &policies)?
        .into_iter()
        .find(|c| c.key == "game.required")
        .ok_or("Schema without default must remain discoverable")?;
    let before = documents.revisions();
    assert!(
        documents
            .change_component(&required, &types, &policies)
            .is_err()
    );
    assert_eq!(documents.revisions(), before);
    let add = documents
        .component_commands(&target, &policies)?
        .into_iter()
        .find(|c| c.key == "game.health" && c.add)
        .ok_or("Registered game component not offered")?;
    documents.change_component(&add, &types, &policies)?;
    let after = documents.revisions();
    assert!(documents.change_component(&add, &types, &policies).is_err());
    assert_eq!(documents.revisions(), after);
    let remove = documents
        .component_commands(&target, &policies)?
        .into_iter()
        .find(|c| c.key == "game.health" && !c.add)
        .ok_or("Removal missing")?;
    documents.change_component(&remove, &types, &policies)?;
    assert!(
        !documents.opened[&id].graph.nodes[&root]
            .components
            .contains_key("game.health")
    );
    let change = documents.prepare_history(false, &types, &policies)?;
    documents.publish_change(change);
    assert!(
        documents.opened[&id].graph.nodes[&root]
            .components
            .contains_key("game.health")
    );
    assert!(!directory.path().join("Arena.scene.ron").exists());
    Ok(())
}
#[test]
fn presets_publish_one_selected_child_and_history_restores_the_same_identity() -> TestResult {
    use crate::documents::ObjectKind;
    let mut types = bevy::reflect::TypeRegistry::default();
    let mut policies = ComponentPolicies::default();
    kedra_scene::spatial::register(&mut types, &mut policies)?;
    for kind in [
        ObjectKind::Empty,
        ObjectKind::Cube,
        ObjectKind::Sphere,
        ObjectKind::Plane,
        ObjectKind::Sun,
        ObjectKind::Camera,
    ] {
        let mut documents = Documents::default();
        assert!(documents.update_clean());
        let snapshot = kedra_authoring::persistence::SourceSnapshot {
            environments: Default::default(),
            namespace: "game".into(),
            generation: 1,
            documents: vec![],
            assets: vec![],
            index: None,
        };
        let admission = source::admit(snapshot, &types, &policies)?;
        documents.create_document(
            DocumentKind::Scene,
            "Arena.scene.ron".into(),
            Some(admission),
            &types,
            &policies,
        )?;
        let ready = documents.poll().ok_or("Created source missing")?;
        let id = ready.document;
        let root = ready.graph.root.clone();
        documents.published(ready, ViewId::new(), root.clone());
        let target = documents.root_target(id).ok_or("Root unavailable")?;
        assert!(
            Documents::default()
                .create_child(kind, &target, &types, &policies)
                .is_err()
        );
        documents.create_child(kind, &target, &types, &policies)?;
        let child = documents.opened[&id]
            .selected
            .clone()
            .ok_or("Created child not selected")?;
        assert_ne!(child, root);
        assert_eq!(
            documents.opened[&id].graph.nodes[&child].parent.as_ref(),
            Some(&root)
        );
        let after = documents.opened[&id].graph.clone();
        let change = documents.prepare_history(false, &types, &policies)?;
        documents.publish_change(change);
        assert!(!documents.opened[&id].graph.nodes.contains_key(&child));
        let change = documents.prepare_history(true, &types, &policies)?;
        documents.publish_change(change);
        assert_eq!(documents.opened[&id].graph, after);
    }
    Ok(())
}
#[test]
fn scene_creation_opens_one_dirty_source_with_common_history_and_no_disk_writes() -> TestResult {
    creation(DocumentKind::Scene, "Arena.scene.ron")
}
#[test]
fn prefab_creation_opens_one_dirty_source_with_common_history_and_no_disk_writes() -> TestResult {
    creation(DocumentKind::Prefab, "Bomb.prefab.ron")
}
fn creation(kind: DocumentKind, path: &str) -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("assets");
    std::fs::create_dir(&root)?;
    let types = bevy::reflect::TypeRegistry::default();
    let policies = ComponentPolicies::default();
    let admission = source::admit(source_files::read(&root, "game")?, &types, &policies)?;
    let mut documents = Documents::default();
    documents.create_document(kind, path.into(), Some(admission), &types, &policies)?;
    let ready = documents.poll().ok_or("new scene did not become ready")?;
    let id = ready.document;
    let node = ready.graph.root.clone();
    documents.published(ready, ViewId::new(), node);
    assert!(documents.can_save());
    assert!(documents.can_history(false));
    assert!(!documents.update_clean());
    assert!(!root.join(path).exists());
    let change = documents.prepare_history(false, &types, &policies)?;
    documents.publish_change(change);
    assert!(!documents.contains(id));
    assert!(documents.update_clean());
    let change = documents.prepare_history(true, &types, &policies)?;
    documents.publish_change(change);
    assert!(documents.contains(id));
    assert!(documents.can_save());
    let before = documents.revisions();
    let wrong_kind = match kind {
        DocumentKind::Scene => "Wrong.prefab.ron",
        DocumentKind::Prefab => "Wrong.scene.ron",
    };
    assert!(
        documents
            .create_document(kind, wrong_kind.into(), None, &types, &policies)
            .is_err()
    );
    assert_eq!(documents.revisions(), before);
    assert!(!root.join(wrong_kind).exists());
    assert!(
        documents
            .create_document(kind, path.into(), None, &types, &policies)
            .is_err()
    );
    assert_eq!(documents.revisions(), before);
    assert_eq!(documents.opened.len(), 1);
    assert_eq!(documents.active, Some(id));
    let change = documents.prepare_discard(&types, &policies)?;
    documents.publish_change(change);
    assert!(documents.update_clean());
    assert!(!documents.can_history(false));
    documents.create_document(kind, path.into(), None, &types, &policies)?;
    let ready = documents.poll().ok_or("recreated scene")?;
    assert_ne!(
        ready.document, id,
        "Discard must release the old unsaved identity/path reservation"
    );
    Ok(())
}
