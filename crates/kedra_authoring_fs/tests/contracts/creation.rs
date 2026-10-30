//! Real publication and history removal use the shared writer, lease and request protocol.
use kedra_authoring::persistence::*;
use kedra_authoring_fs::{commit_documents, source};
use kedra_scene::document::{DocumentKind, PrefabDocument};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn request() -> Result<CommitRequest, Box<dyn std::error::Error>> {
    let mut doc = PrefabDocument::new("日本語");
    doc.kind = DocumentKind::Scene;
    Ok(CommitRequest {
        operation: uuid::Uuid::new_v4(),
        expected_generation: 1,
        expected_sources: vec![],
        checks: vec![
            FileCheck {
                path: "日本語.scene.ron".into(),
                expected_hash: None,
            },
            FileCheck {
                path: "catalog.prefabs.ron".into(),
                expected_hash: None,
            },
        ],
        files: vec![CommitFile {
            document: Some(doc.id),
            path: "日本語.scene.ron".into(),
            revision: 1,
            expected_hash: None,
            source: Some(doc.serialize()?),
        }],
    })
}
#[test]
fn new_source_round_trips_through_admission_retry_removal_and_restoration() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("assets");
    std::fs::create_dir(&root)?;
    let create = request()?;
    let receipt = commit_documents(&root, &create)?;
    assert_eq!(
        commit_documents(&root, &create)?.operation,
        receipt.operation
    );
    let observed = source::read(&root, "game")?;
    assert_eq!(
        observed.documents[0].source,
        create.files[0].source.as_deref().ok_or("source")?
    );
    assert_eq!(observed.documents.len(), 1);
    let mut remove = create.clone();
    remove.operation = uuid::Uuid::new_v4();
    remove.expected_generation = receipt.generation;
    remove.expected_sources.push(create.files[0].path.clone());
    remove.checks[0].expected_hash = receipt.files[0].hash.clone();
    remove.files[0].expected_hash = receipt.files[0].hash.clone();
    remove.files[0].source = None;
    assert!(commit_documents(&root, &remove)?.files[0].hash.is_none());
    assert!(source::read(&root, "game")?.documents.is_empty());
    assert!(commit_documents(&root, &remove)?.files[0].hash.is_none());
    let mut restore = create.clone();
    restore.operation = uuid::Uuid::new_v4();
    commit_documents(&root, &restore)?;
    assert_eq!(
        source::read(&root, "game")?.documents[0].source,
        observed.documents[0].source
    );
    Ok(())
}
#[test]
fn foreign_targets_and_orphan_sidecars_refuse_without_overwriting() -> TestResult {
    for sidecar in [false, true] {
        let directory = tempfile::tempdir()?;
        let root = directory.path().join("assets");
        std::fs::create_dir(&root)?;
        let request = request()?;
        let foreign = root.join(if sidecar {
            "日本語.scene.ron.meta"
        } else {
            "日本語.scene.ron"
        });
        std::fs::write(&foreign, "foreign bytes")?;
        assert!(commit_documents(&root, &request).is_err());
        assert_eq!(std::fs::read_to_string(&foreign)?, "foreign bytes");
        if sidecar {
            assert!(!root.join("日本語.scene.ron").exists());
        }
    }
    Ok(())
}
