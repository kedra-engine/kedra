//! Coupled inventory changes retain rollback and crash-replay ownership through the real port.
use super::*;
use kedra_authoring::persistence::{CommitFile, FileCheck};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn fixture() -> Result<(tempfile::TempDir, CommitRequest, String), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("assets");
    std::fs::create_dir(&root)?;
    let old = PrefabDocument::new("Old");
    let original = old.serialize()?;
    std::fs::write(root.join("old.prefab.ron"), &original)?;
    let new = PrefabDocument::new("New");
    let request = CommitRequest {
        operation: uuid::Uuid::new_v4(),
        expected_generation: 1,
        expected_sources: vec!["old.prefab.ron".into()],
        checks: vec![
            FileCheck {
                path: "old.prefab.ron".into(),
                expected_hash: Some(content_hash(&original)),
            },
            FileCheck {
                path: "new.prefab.ron".into(),
                expected_hash: None,
            },
        ],
        files: vec![
            CommitFile {
                document: Some(old.id),
                path: "old.prefab.ron".into(),
                revision: 2,
                expected_hash: Some(content_hash(&original)),
                source: None,
            },
            CommitFile {
                document: Some(new.id),
                path: "new.prefab.ron".into(),
                revision: 2,
                expected_hash: None,
                source: Some(new.serialize()?),
            },
        ],
    };
    Ok((directory, request, original))
}
#[test]
fn rejected_later_write_rolls_back_removal_and_creation_without_foreign_changes() -> TestResult {
    let (directory, request, original) = fixture()?;
    let root = directory.path().join("assets");
    let session = Session::new(&root)?;
    let backend = storage::Guarded {
        session: &session,
        request: &request,
    };
    block_on(backend.create_dir_all(&session.staging))?;
    let mut changes = ChangeSet::new();
    changes.remove("old.prefab.ron");
    changes.write(
        "new.prefab.ron",
        request.files[1]
            .source
            .as_deref()
            .ok_or("new source")?
            .as_bytes(),
    );
    changes.write("new.prefab.ron", b"ungranted bytes");
    assert!(block_on(session.journal.apply(&changes, &backend, &root)).is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("old.prefab.ron"))?,
        original
    );
    assert!(!root.join("new.prefab.ron").exists());
    assert_eq!(std::fs::read_dir(&root)?.count(), 1);
    assert!(!session.journal_present()?);
    Ok(())
}
#[test]
fn interrupted_removal_recovers_private_backup_and_completes_coupled_creation() -> TestResult {
    let (directory, request, _) = fixture()?;
    let root = directory.path().join("assets");
    let session = Session::new(&root)?;
    let backend = storage::Guarded {
        session: &session,
        request: &request,
    };
    block_on(backend.create_dir_all(&session.staging))?;
    std::fs::write(&session.intent, ron::to_string(&request)?)?;
    let ops = vec![
        FileOp::Remove {
            path: "old.prefab.ron".into(),
        },
        FileOp::Write {
            path: "new.prefab.ron".into(),
            bytes: request.files[1]
                .source
                .as_deref()
                .ok_or("new source")?
                .as_bytes()
                .to_vec(),
        },
    ];
    std::fs::write(
        session.journal.path_in(&root),
        fs_transaction::journal::encode(&ops)?,
    )?;
    block_on(backend.rename(
        &root.join("old.prefab.ron"),
        &root.join(".old.prefab.ron.fstx-aside-0"),
    ))?;
    assert!(
        std::fs::read_dir(&root)?.next().is_none(),
        "rollback backup must stay outside authoring inventory"
    );
    assert!(read_guard(&root, None).is_err());
    let receipt = recover_documents(&root)?.ok_or("missing recovery receipt")?;
    assert_eq!(receipt.operation, request.operation);
    assert!(receipt.files[0].hash.is_none());
    assert_eq!(
        std::fs::read_to_string(root.join("new.prefab.ron"))?,
        request.files[1].source.as_deref().ok_or("source")?
    );
    assert!(!root.join("old.prefab.ron").exists());
    assert!(std::fs::read_dir(&session.staging)?.next().is_none());
    assert!(recover_documents(&root)?.is_none());
    Ok(())
}
