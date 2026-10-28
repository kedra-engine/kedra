//! Kedra recovery admission/correlation over real files and the upstream journal codec.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn fixture() -> Result<(tempfile::TempDir, CommitRequest), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("assets");
    std::fs::create_dir(&root)?;
    std::fs::create_dir(root.join("Prefabs"))?;
    let mut files = Vec::new();
    let mut checks = Vec::new();
    for path in ["入口.prefab.ron", "Prefabs/Exit.prefab.ron"] {
        let mut document = PrefabDocument::new(path);
        let before = document.serialize()?;
        std::fs::write(root.join(path), &before)?;
        document.title = "Saved together".into();
        files.push(kedra_authoring::persistence::CommitFile {
            document: Some(document.id),
            path: path.into(),
            revision: 2,
            expected_hash: Some(content_hash(&before)),
            source: Some(document.serialize()?),
        });
        checks.push(kedra_authoring::persistence::FileCheck {
            path: path.into(),
            expected_hash: Some(content_hash(&before)),
        });
    }
    checks.push(kedra_authoring::persistence::FileCheck {
        path: "catalog.prefabs.ron".into(),
        expected_hash: None,
    });
    let request = CommitRequest {
        operation: uuid::Uuid::new_v4(),
        expected_generation: 1,
        expected_sources: files.iter().map(|file| file.path.clone()).collect(),
        files,
        checks,
    };
    Ok((directory, request))
}
fn interrupted(
    root: &Path,
    request: &CommitRequest,
) -> Result<Session, Box<dyn std::error::Error>> {
    let session = Session::new(root)?;
    let pinned = pin_sidecars(&session.root, request)?;
    std::fs::write(&session.intent, ron::to_string(&pinned)?)?;
    std::fs::create_dir(&session.staging)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&session.staging, std::fs::Permissions::from_mode(0o700))?;
    }
    let ops: Vec<_> = request
        .files
        .iter()
        .map(|file| FileOp::Write {
            path: (&file.path).into(),
            bytes: file
                .source
                .as_deref()
                .unwrap_or_default()
                .as_bytes()
                .to_vec(),
        })
        .collect();
    std::fs::write(
        session.journal.path_in(root),
        fs_transaction::journal::encode(&ops)?,
    )?;
    std::fs::write(
        root.join(&request.files[0].path),
        request.files[0].source.as_deref().ok_or("source")?,
    )?;
    Ok(session)
}
#[test]
fn coupled_sources_publish_with_one_receipt_and_shared_admission_excludes_writers() -> TestResult {
    let (directory, request) = fixture()?;
    let root = directory.path().join("assets");
    let guard = read_guard(&root, None)?;
    assert!(commit_existing(&root, &request).is_err());
    assert!(
        request
            .files
            .iter()
            .all(|file| hash(&root.join(&file.path)).ok().as_ref() == file.expected_hash.as_ref())
    );
    drop(guard);
    let receipt = commit_existing(&root, &request)?;
    assert_eq!(receipt.files.len(), 2);
    assert!(request.files.iter().all(|file| {
        std::fs::read_to_string(root.join(&file.path)).ok().as_ref() == file.source.as_ref()
    }));
    commit_existing(&root, &request)?;
    assert!(recover_existing(&root)?.is_none());
    assert!(!root.join("catalog.prefabs.ron").exists());
    Ok(())
}
#[test]
fn interrupted_group_blocks_admission_and_recovers_exact_intent_without_rewriting_foreign_files()
-> TestResult {
    let (directory, request) = fixture()?;
    let root = directory.path().join("assets");
    std::fs::write(root.join("入口.prefab.ron.meta"), "Preserve sidecar bytes")?;
    let session = interrupted(&root, &request)?;
    assert!(read_guard(&root, None).is_err());
    drop(read_guard(&root, Some(&request))?);
    let mut foreign = request.clone();
    foreign.operation = uuid::Uuid::new_v4();
    assert!(read_guard(&root, Some(&foreign)).is_err());
    assert!(commit_existing(&root, &foreign).is_err());
    let receipt = recover_existing(&root)?.ok_or("recovery")?;
    assert_eq!(receipt.operation, request.operation);
    assert_eq!(receipt.files.len(), 2);
    assert!(request.files.iter().all(|file| {
        std::fs::read_to_string(root.join(&file.path)).ok().as_ref() == file.source.as_ref()
    }));
    assert!(!session.intent.exists() && !session.journal_present()?);
    assert_eq!(
        std::fs::read_to_string(root.join("入口.prefab.ron.meta"))?,
        "Preserve sidecar bytes"
    );
    assert!(recover_existing(&root)?.is_none());
    read_guard(&root, None)?;
    Ok(())
}
#[test]
fn recovery_conflicts_or_foreign_journal_operations_preserve_all_observed_bytes_and_records()
-> TestResult {
    for case in ["external", "foreign_op", "sidecar", "oversized_ops"] {
        let (directory, request) = fixture()?;
        let root = directory.path().join("assets");
        if case == "sidecar" {
            std::fs::write(root.join("入口.prefab.ron.meta"), "Original sidecar")?;
        }
        let session = interrupted(&root, &request)?;
        let foreign = directory.path().join("foreign.txt");
        std::fs::write(&foreign, "Keep foreign data")?;
        if case == "oversized_ops" {
            let file = &request.files[0];
            let ops: Vec<_> = (0..129)
                .map(|_| FileOp::Write {
                    path: (&file.path).into(),
                    bytes: file
                        .source
                        .as_deref()
                        .unwrap_or_default()
                        .as_bytes()
                        .to_vec(),
                })
                .collect();
            std::fs::write(
                session.journal.path_in(&root),
                fs_transaction::journal::encode(&ops)?,
            )?;
        } else if case == "foreign_op" {
            let ops = vec![FileOp::Write {
                path: "../foreign.txt".into(),
                bytes: b"Must reject".to_vec(),
            }];
            std::fs::write(
                session.journal.path_in(&root),
                fs_transaction::journal::encode(&ops)?,
            )?;
        } else if case == "sidecar" {
            std::fs::write(root.join("入口.prefab.ron.meta"), "External sidecar")?;
        } else {
            std::fs::write(
                root.join(&request.files[1].path),
                PrefabDocument::new("External").serialize()?,
            )?;
        }
        let before: Vec<_> = request
            .files
            .iter()
            .map(|file| std::fs::read(root.join(&file.path)))
            .collect::<Result<_, _>>()?;
        let error = recover_existing(&root)
            .err()
            .ok_or("recovery must refuse")?;
        if case == "oversized_ops" {
            assert!(
                error.message.contains("outside its admitted request"),
                "{error}"
            );
        }
        for (file, before) in request.files.iter().zip(before) {
            assert_eq!(std::fs::read(root.join(&file.path))?, before);
        }
        assert_eq!(std::fs::read_to_string(&foreign)?, "Keep foreign data");
        assert!(session.intent.exists() && session.journal_present()?);
    }
    Ok(())
}

#[test]
fn group_retry_accepts_already_published_bytes_when_another_member_was_content_unchanged()
-> TestResult {
    let (directory, mut request) = fixture()?;
    let root = directory.path().join("assets");
    request.files[1].source = Some(std::fs::read_to_string(root.join(&request.files[1].path))?);
    let receipt = commit_existing(&root, &request)?;
    assert_eq!(receipt.files.len(), 2);
    let before = request
        .files
        .iter()
        .map(|file| std::fs::read(root.join(&file.path)))
        .collect::<Result<Vec<_>, _>>()?;
    commit_existing(&root, &request)?;
    for (file, bytes) in request.files.iter().zip(before) {
        assert_eq!(std::fs::read(root.join(&file.path))?, bytes);
    }
    Ok(())
}
#[test]
fn preflight_refusal_of_one_group_member_leaves_every_other_source_unchanged() -> TestResult {
    let (directory, request) = fixture()?;
    let root = directory.path().join("assets");
    let last = root.join(&request.files[1].path);
    let mut permissions = std::fs::metadata(&last)?.permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&last, permissions)?;
    assert!(commit_existing(&root, &request).is_err());
    assert!(
        request
            .files
            .iter()
            .all(|file| hash(&root.join(&file.path)).ok().as_ref() == file.expected_hash.as_ref())
    );
    assert!(!Session::new(&root)?.intent.exists());
    Ok(())
}

#[test]
fn guarded_port_refusal_rolls_back_an_earlier_replacement_without_changing_permissions()
-> TestResult {
    let (directory, request) = fixture()?;
    let root = directory.path().join("assets");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            root.join(&request.files[0].path),
            std::fs::Permissions::from_mode(0o640),
        )?;
    }
    let session = Session::new(&root)?;
    std::fs::write(&session.intent, ron::to_string(&request)?)?;
    let mut changes = ChangeSet::new();
    changes.write(
        &request.files[0].path,
        request.files[0]
            .source
            .as_deref()
            .ok_or("source")?
            .as_bytes(),
    );
    // The upstream engine may only publish bytes authorized by Kedra's request.
    // Its rollback must also remain usable through our restricted storage port.
    changes.write(&request.files[1].path, b"not admitted source bytes");
    let backend = storage::Guarded {
        session: &session,
        request: &request,
    };
    block_on(backend.create_dir_all(&session.staging))?;
    let error = block_on(session.journal.apply(&changes, &backend, &session.root))
        .err()
        .ok_or("guard should refuse foreign bytes")?;
    assert!(
        error.to_string().contains("outside its admitted request"),
        "{error}"
    );
    assert!(
        request
            .files
            .iter()
            .all(|file| hash(&root.join(&file.path)).ok().as_ref() == file.expected_hash.as_ref())
    );
    assert!(!session.journal_present()?);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(root.join(&request.files[0].path))?
                .permissions()
                .mode()
                & 0o777,
            0o640
        );
    }
    assert!(session.intent.exists());
    recover_existing(&root)?.ok_or("recover retained intent")?;
    assert!(request.files.iter().all(|file| {
        std::fs::read_to_string(root.join(&file.path)).ok().as_ref() == file.source.as_ref()
    }));
    Ok(())
}

#[test]
fn guarded_flush_preserves_admitted_bytes_and_refuses_foreign_paths() -> TestResult {
    let (directory, request) = fixture()?;
    let root = directory.path().join("assets");
    let session = Session::new(&root)?;
    let backend = storage::Guarded {
        session: &session,
        request: &request,
    };
    let path = session.root.join(&request.files[0].path);
    let before = std::fs::read(&path)?;
    block_on(backend.sync(&path, fs_transaction::fs::Durability::Durable))?;
    assert_eq!(std::fs::read(&path)?, before);
    let foreign = directory.path().join("foreign.txt");
    std::fs::write(&foreign, b"not granted")?;
    assert!(block_on(backend.sync(&foreign, fs_transaction::fs::Durability::Durable)).is_err());
    assert_eq!(std::fs::read(&foreign)?, b"not granted");
    assert!(!session.journal_present()?);
    assert!(!session.intent.exists());
    Ok(())
}
