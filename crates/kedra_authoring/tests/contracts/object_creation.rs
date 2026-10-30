//! Node/component creation validates one isolated candidate and publishes one history entry.
use bevy::reflect::TypeRegistry;
use kedra_authoring::{editing::SourceEdit, persistence::SourceSnapshot, source};
use kedra_scene::{document::*, reflection::ComponentPolicies};

#[test]
fn child_and_components_commit_together_and_invalid_requests_preserve_the_candidate()
-> Result<(), Box<dyn std::error::Error>> {
    let mut types = TypeRegistry::default();
    let mut policies = ComponentPolicies::default();
    kedra_scene::spatial::register(&mut types, &mut policies)?;
    let (mut scope, _) = source::admit(
        SourceSnapshot {
            environments: Default::default(),
            namespace: "game".into(),
            generation: 1,
            documents: vec![],
            assets: vec![],
            index: None,
        },
        &types,
        &policies,
    )?;
    let document = PrefabDocument::new("Arena");
    let id = document.id;
    source::create_document(
        &mut scope,
        document,
        "Arena.prefab.ron".into(),
        &types,
        &policies,
    )?;
    let before = scope.working.catalog().documents;
    let history = scope.working.undo_entry().ok_or("history")?.id;
    let path = "game://Arena.prefab.ron";
    let mut edit = SourceEdit::begin(&scope, "Create cube", &[path.into()])?;
    let parent = edit.nodes(&scope, path)?.remove(0);
    assert!(
        edit.add_node(&scope, &parent, "", &types, &policies)
            .is_err()
    );
    let child = edit.add_node(&scope, &parent, "Куб", &types, &policies)?;
    assert!(
        edit.add_component(
            &scope,
            &child,
            "scene.primitive",
            ComponentData::ron(1, "Cuboid(size:(-1.0,1.0,1.0))")?,
            &types,
            &policies
        )
        .is_err()
    );
    edit.add_component(
        &scope,
        &child,
        "scene.primitive",
        ComponentData::ron(1, "Cuboid(size:(1.0,1.0,1.0))")?,
        &types,
        &policies,
    )?;
    assert!(
        edit.add_component(
            &scope,
            &child,
            "scene.primitive",
            ComponentData::ron(1, "Sphere(radius:2.0)")?,
            &types,
            &policies
        )
        .is_err()
    );
    let mut foreign = parent.clone();
    foreign.document = DocumentId::new();
    assert!(
        edit.add_node(&scope, &foreign, "Foreign", &types, &policies)
            .is_err()
    );
    assert_eq!(scope.working.catalog().documents, before);
    assert_eq!(scope.working.undo_entry().ok_or("history")?.id, history);
    edit.commit(&mut scope, &types, &policies)?;
    let after = scope.working.catalog().documents;
    let graph = scope.working.catalog().resolve(id)?;
    assert_eq!(
        graph.nodes[&child.address].parent.as_ref(),
        Some(&parent.address)
    );
    assert!(
        graph.nodes[&child.address].components["scene.primitive"]
            .data
            .value
            .get_ron()
            .contains("Cuboid")
    );
    scope.working = kedra_authoring::changes::history(&scope, false, &types, &policies)?.working;
    assert_eq!(scope.working.catalog().documents, before);
    scope.working = kedra_authoring::changes::history(&scope, true, &types, &policies)?.working;
    assert_eq!(scope.working.catalog().documents, after);
    let mut remove = SourceEdit::begin(&scope, "Remove newly authored component", &[path.into()])?;
    remove.remove_component(&scope, &child, "scene.primitive", &types, &policies)?;
    remove.commit(&mut scope, &types, &policies)?;
    assert!(
        !scope.working.catalog().resolve(id)?.nodes[&child.address]
            .components
            .contains_key("scene.primitive")
    );
    let mut restore = SourceEdit::begin(&scope, "Add component after removal", &[path.into()])?;
    restore.add_component(
        &scope,
        &child,
        "scene.primitive",
        ComponentData::ron(1, "Sphere(radius:2.0)")?,
        &types,
        &policies,
    )?;
    restore.commit(&mut scope, &types, &policies)?;
    assert!(
        scope.working.catalog().resolve(id)?.nodes[&child.address].components["scene.primitive"]
            .data
            .value
            .get_ron()
            .contains("Sphere")
    );
    Ok(())
}
