use kedra_authoring::persistence::*;
use kedra_authoring_fs::commit_existing;
use kedra_scene::document::PrefabDocument;
use std::{collections::BTreeMap, path::Path};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn fixture() -> Result<(tempfile::TempDir, CommitRequest, String), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("assets");
    std::fs::create_dir(&root)?;
    let mut doc = PrefabDocument::new("Original");
    let original = doc.serialize()?;
    std::fs::write(root.join("日本語.prefab.ron"), &original)?;
    doc.title = "Saved".into();
    let source = doc.serialize()?;
    let request = CommitRequest {
        operation: uuid::Uuid::new_v4(),
        expected_generation: 1,
        expected_sources: vec!["日本語.prefab.ron".into()],
        checks: vec![
            FileCheck {
                path: "日本語.prefab.ron".into(),
                expected_hash: Some(content_hash(&original)),
            },
            FileCheck {
                path: "catalog.prefabs.ron".into(),
                expected_hash: None,
            },
        ],
        files: vec![CommitFile {
            document: Some(doc.id),
            path: "日本語.prefab.ron".into(),
            revision: 2,
            expected_hash: Some(content_hash(&original)),
            source: Some(source.clone()),
        }],
    };
    Ok((directory, request, source))
}
fn inventory(root: &Path) -> Result<BTreeMap<std::ffi::OsString, Vec<u8>>, std::io::Error> {
    std::fs::read_dir(root)?
        .map(|entry| {
            let entry = entry?;
            Ok((entry.file_name(), std::fs::read(entry.path())?))
        })
        .collect()
}
#[test]
fn existing_source_replacement_preserves_identity_permissions_and_reconciles_exact_retries()
-> TestResult {
    let (directory, request, source) = fixture()?;
    let root = directory.path().join("assets");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            root.join("日本語.prefab.ron"),
            std::fs::Permissions::from_mode(0o640),
        )?;
    }
    let receipt = commit_existing(&root, &request)?;
    assert_eq!(receipt.operation, request.operation);
    assert_eq!(receipt.generation, 2);
    assert_eq!(
        std::fs::read_to_string(root.join("日本語.prefab.ron"))?,
        source
    );
    assert!(!root.join("catalog.prefabs.ron").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(root.join("日本語.prefab.ron"))?
                .permissions()
                .mode()
                & 0o777,
            0o640
        );
    }
    let before = inventory(&root)?;
    let repeated = commit_existing(&root, &request)?;
    assert_eq!(repeated.files[0].hash, receipt.files[0].hash);
    assert_eq!(repeated.files[0].revision, receipt.files[0].revision);
    assert_eq!(inventory(&root)?, before);
    Ok(())
}
#[test]
fn source_conflicts_inventory_changes_and_invalid_requests_never_replace_sources() -> TestResult {
    for case in [
        "external",
        "new_source",
        "new_asset",
        "duplicate_target",
        "creation",
        "deletion",
        "traversal",
        "missing_check",
        "kind_mismatch",
        "identity_change",
    ] {
        let (directory, mut request, _) = fixture()?;
        let root = directory.path().join("assets");
        match case {
            "external" => std::fs::write(
                root.join("日本語.prefab.ron"),
                PrefabDocument::new("External").serialize()?,
            )?,
            "new_source" => std::fs::write(
                root.join("new.prefab.ron"),
                PrefabDocument::new("New").serialize()?,
            )?,
            "new_asset" => std::fs::write(root.join("new.bin"), [0u8, 255])?,
            "duplicate_target" => request.files.push(request.files[0].clone()),
            "creation" => request.files[0].expected_hash = None,
            "deletion" => request.files[0].source = None,
            "traversal" => request.files[0].path = "../foreign.prefab.ron".into(),
            "kind_mismatch" => {
                let mut document =
                    PrefabDocument::parse(request.files[0].source.as_deref().ok_or("source")?)?;
                document.kind = kedra_scene::document::DocumentKind::Scene;
                request.files[0].source = Some(document.serialize()?);
            }
            "identity_change" => {
                let document = PrefabDocument::new("Foreign identity");
                request.files[0].document = Some(document.id);
                request.files[0].source = Some(document.serialize()?);
            }
            _ => request.checks.clear(),
        }
        let before = inventory(&root)?;
        assert!(commit_existing(&root, &request).is_err(), "{case}");
        assert_eq!(inventory(&root)?, before, "{case}");
    }
    Ok(())
}
#[test]
fn cooperative_writer_lease_conflict_preserves_the_source_and_can_be_retried() -> TestResult {
    let (directory, request, _) = fixture()?;
    let root = directory.path().join("assets");
    let lease = std::fs::File::create(directory.path().join("assets.kedra-save.lock"))?;
    lease.try_lock()?;
    let before = inventory(&root)?;
    assert!(commit_existing(&root, &request).is_err());
    assert_eq!(inventory(&root)?, before);
    drop(lease);
    commit_existing(&root, &request)?;
    Ok(())
}
#[cfg(unix)]
#[test]
fn source_and_lease_symlinks_cannot_redirect_save_to_another_file() -> TestResult {
    use std::os::unix::fs::symlink;
    for lease in [false, true] {
        let (directory, request, _) = fixture()?;
        let root = directory.path().join("assets");
        let foreign = directory.path().join("foreign");
        std::fs::write(&foreign, "Keep foreign bytes")?;
        if lease {
            symlink(&foreign, directory.path().join("assets.kedra-save.lock"))?;
        } else {
            std::fs::remove_file(root.join("日本語.prefab.ron"))?;
            symlink(&foreign, root.join("日本語.prefab.ron"))?;
        }
        assert!(commit_existing(&root, &request).is_err());
        assert_eq!(std::fs::read_to_string(foreign)?, "Keep foreign bytes");
    }
    Ok(())
}
