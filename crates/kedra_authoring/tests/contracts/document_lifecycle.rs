//! Inventory authoring uses the same bounded source history and correlated Save protocol.
use bevy::reflect::TypeRegistry;
use kedra_authoring::{persistence::*, source};
use kedra_scene::{document::*, reflection::ComponentPolicies};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn empty() -> SourceSnapshot {
    SourceSnapshot {
        environments: Default::default(),
        namespace: "game".into(),
        generation: 1,
        documents: vec![],
        assets: vec![],
        index: None,
    }
}
fn receipt(request: &CommitRequest) -> Receipt {
    Receipt {
        operation: request.operation,
        generation: request.expected_generation + 1,
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
    }
}
#[test]
fn creation_undo_redo_and_save_keep_identity_path_and_saved_content() -> TestResult {
    let types = TypeRegistry::default();
    let policies = ComponentPolicies::default();
    let (mut workspace, _) = source::admit(empty(), &types, &policies)?;
    let mut doc = PrefabDocument::new("日本語");
    doc.kind = DocumentKind::Scene;
    let id = doc.id;
    source::create_document(
        &mut workspace,
        doc.clone(),
        "日本語.scene.ron".into(),
        &types,
        &policies,
    )?;
    let operation = workspace.working.undo_entry().ok_or("history")?.id;
    assert!(workspace.working.dirty(id));
    assert!(workspace.source.documents.is_empty());
    workspace.working =
        kedra_authoring::changes::history(&workspace, false, &types, &policies)?.working;
    assert!(!workspace.working.documents().contains_key(&id));
    assert!(!workspace.working.dirty(id));
    workspace.working =
        kedra_authoring::changes::history(&workspace, true, &types, &policies)?.working;
    assert_eq!(
        workspace.working.undo_entry().ok_or("history")?.id,
        operation
    );
    assert_eq!(workspace.working.documents()[&id].document, doc);
    assert_eq!(workspace.path(id), Some("日本語.scene.ron"));
    let request = source::prepare_document_save(&mut workspace, id, &types, &policies)?;
    assert!(request.files[0].expected_hash.is_none());
    assert!(
        request
            .checks
            .iter()
            .any(|check| check.path == "日本語.scene.ron" && check.expected_hash.is_none())
    );
    workspace.acknowledge(&receipt(&request))?;
    assert!(!workspace.working.dirty(id));
    assert!(workspace.working.can_undo());
    workspace.working =
        kedra_authoring::changes::history(&workspace, false, &types, &policies)?.working;
    assert!(
        workspace.working.dirty(id),
        "undoing saved creation is a pending source removal"
    );
    let removal = source::prepare_document_save(&mut workspace, id, &types, &policies)?;
    assert!(removal.files[0].source.is_none());
    workspace.acknowledge(&receipt(&removal))?;
    assert!(!workspace.working.dirty(id));
    workspace.working =
        kedra_authoring::changes::history(&workspace, true, &types, &policies)?.working;
    assert!(workspace.working.dirty(id));
    assert_eq!(workspace.working.documents()[&id].document, doc);
    Ok(())
}
#[test]
fn invalid_path_or_identity_cannot_replace_working_sources_or_consume_redo() -> TestResult {
    let types = TypeRegistry::default();
    let policies = ComponentPolicies::default();
    let (mut workspace, _) = source::admit(empty(), &types, &policies)?;
    let doc = PrefabDocument::new("Source");
    source::create_document(
        &mut workspace,
        doc.clone(),
        "source.prefab.ron".into(),
        &types,
        &policies,
    )?;
    let before = ron::to_string(&workspace)?;
    assert!(
        source::create_document(
            &mut workspace,
            doc,
            "other.prefab.ron".into(),
            &types,
            &policies
        )
        .is_err()
    );
    assert_eq!(ron::to_string(&workspace)?, before);
    workspace.working =
        kedra_authoring::changes::history(&workspace, false, &types, &policies)?.working;
    let before = ron::to_string(&workspace)?;
    assert!(
        source::create_document(
            &mut workspace,
            PrefabDocument::new("Invalid"),
            "../escape.prefab.ron".into(),
            &types,
            &policies
        )
        .is_err()
    );
    assert_eq!(ron::to_string(&workspace)?, before);
    assert!(workspace.working.can_redo());
    Ok(())
}
