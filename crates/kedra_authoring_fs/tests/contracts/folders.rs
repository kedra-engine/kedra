//! Folder publication preserves exact content/identities and refuses rejected candidates.
use kedra_authoring_fs::{
    entries::{Change, apply},
    source,
};
use std::fs;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn file_moves_preserve_identity_and_pair_metadata_through_delete() -> TestResult {
    let project = tempfile::tempdir()?;
    let root = project.path().join("assets");
    fs::create_dir_all(root.join("nested"))?;
    fs::write(root.join("original.txt"), "retained bytes")?;
    let meta = format!(
        "(id:\"{}\",meta_format_version:\"1.0\",asset:Load(loader:\"kedra_assets::data::DataLoader\",settings:()))",
        uuid::Uuid::new_v4()
    );
    fs::write(root.join("original.txt.meta"), &meta)?;
    let original = source::read(&root, "game")?;
    let change = Change::Rename {
        from: "original.txt".into(),
        to: "nested/日本語.txt".into(),
    };
    assert!(
        apply(&root, "game", &change, |_, _| Err::<(), _>(
            "Referenced asset".into()
        ))
        .is_err()
    );
    assert_eq!(fs::read_to_string(root.join("original.txt.meta"))?, meta);
    assert_eq!(
        fs::read_to_string(root.join("original.txt"))?,
        "retained bytes"
    );
    assert!(!root.join("nested/日本語.txt").exists());
    let moved = apply(&root, "game", &change, |_, after| Ok(after))?;
    assert_eq!(moved.assets[0].id, original.assets[0].id);
    assert_eq!(moved.assets, source::read(&root, "game")?.assets);
    assert!(!root.join("original.txt").exists());
    assert!(!root.join("original.txt.meta").exists());
    assert_eq!(
        fs::read_to_string(root.join("nested/日本語.txt.meta"))?,
        meta
    );
    fs::write(root.join("collision.txt.meta"), "reserved")?;
    assert!(
        apply(
            &root,
            "game",
            &Change::Rename {
                from: "nested/日本語.txt".into(),
                to: "collision.txt".into()
            },
            |_, _| Ok(())
        )
        .is_err()
    );
    assert_eq!(
        fs::read_to_string(root.join("collision.txt.meta"))?,
        "reserved"
    );
    assert_eq!(
        fs::read_to_string(root.join("nested/日本語.txt.meta"))?,
        meta
    );
    assert_eq!(
        fs::read_to_string(root.join("nested/日本語.txt"))?,
        "retained bytes"
    );
    fs::remove_file(root.join("collision.txt.meta"))?;
    apply(
        &root,
        "game",
        &Change::Delete("nested/日本語.txt".into()),
        |_, after| {
            assert!(after.assets.is_empty());
            Ok(())
        },
    )?;
    assert!(!root.join("nested/日本語.txt.meta").exists());
    let recovery = fs::read_dir(project.path())?
        .filter_map(Result::ok)
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".kedra-deleted-")
        })
        .ok_or("recovery missing")?
        .path();
    assert_eq!(
        fs::read_to_string(recovery.join("日本語.txt"))?,
        "retained bytes"
    );
    assert_eq!(fs::read_to_string(recovery.join("日本語.txt.meta"))?, meta);
    Ok(())
}

#[test]
fn folder_move_refuses_descendants_and_keeps_contents() -> TestResult {
    let project = tempfile::tempdir()?;
    let root = project.path().join("assets");
    fs::create_dir_all(root.join("original/child"))?;
    fs::create_dir(root.join("destination"))?;
    fs::write(root.join("original/file.txt"), "content")?;
    assert!(
        apply(
            &root,
            "game",
            &Change::Rename {
                from: "original".into(),
                to: "original/child/moved".into()
            },
            |_, _| Ok(())
        )
        .is_err()
    );
    assert_eq!(
        fs::read_to_string(root.join("original/file.txt"))?,
        "content"
    );
    apply(
        &root,
        "game",
        &Change::Rename {
            from: "original".into(),
            to: "destination/moved".into(),
        },
        |_, _| Ok(()),
    )?;
    assert!(!root.join("original").exists());
    assert_eq!(
        fs::read_to_string(root.join("destination/moved/file.txt"))?,
        "content"
    );
    Ok(())
}

#[test]
fn folder_lifecycle_preserves_unicode_bytes_sidecars_and_recoverable_deleted_contents() -> TestResult
{
    let project = tempfile::tempdir()?;
    let root = project.path().join("assets");
    fs::create_dir(&root)?;
    apply(
        &root,
        "game",
        &Change::Create("日本語".into()),
        |_, after| Ok(after),
    )?;
    fs::write(root.join("日本語/license.txt"), b"original bytes")?;
    let meta = format!(
        "(id:\"{}\",meta_format_version:\"1.0\",asset:Load(loader:\"kedra_assets::data::DataLoader\",settings:()))",
        uuid::Uuid::new_v4()
    );
    fs::write(root.join("日本語/license.txt.meta"), &meta)?;
    let original = source::read(&root, "game")?;
    let renamed = apply(
        &root,
        "game",
        &Change::Rename {
            from: "日本語".into(),
            to: "Модели".into(),
        },
        |before, after| {
            assert_eq!(before.assets, original.assets);
            assert_eq!(after.assets[0].id, before.assets[0].id);
            Ok(after)
        },
    )?;
    assert_eq!(renamed.assets, source::read(&root, "game")?.assets);
    assert!(!root.join("日本語").exists());
    assert_eq!(
        fs::read(root.join("Модели/license.txt"))?,
        b"original bytes"
    );
    assert_eq!(
        fs::read_to_string(root.join("Модели/license.txt.meta"))?,
        meta
    );
    apply(
        &root,
        "game",
        &Change::Delete("Модели".into()),
        |_, after| {
            assert!(after.assets.is_empty());
            Ok(())
        },
    )?;
    assert!(!root.join("Модели").exists());
    let recovery = fs::read_dir(project.path())?
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".kedra-deleted-")
        })
        .ok_or("recovery")?
        .path();
    assert_eq!(
        fs::read(recovery.join("Модели/license.txt"))?,
        b"original bytes"
    );
    assert_eq!(
        fs::read_to_string(recovery.join("Модели/license.txt.meta"))?,
        meta
    );
    Ok(())
}

#[test]
fn conflicts_traversal_rejected_validation_and_active_read_lease_leave_folders_unchanged()
-> TestResult {
    let project = tempfile::tempdir()?;
    let root = project.path().join("assets");
    fs::create_dir_all(root.join("Original"))?;
    fs::create_dir(root.join("Existing"))?;
    fs::write(root.join("Original/file.txt"), "retained")?;
    for change in [
        Change::Create("Original".into()),
        Change::Create("../escape".into()),
        Change::Delete("".into()),
        Change::Rename {
            from: "Original".into(),
            to: "Existing".into(),
        },
    ] {
        assert!(apply(&root, "game", &change, |_, _| Ok(())).is_err());
    }
    assert!(
        apply(&root, "game", &Change::Delete("Original".into()), |_, _| {
            Err::<(), _>("Referenced source".into())
        })
        .is_err()
    );
    let guard = kedra_authoring_fs::read_guard(&root, None)?;
    assert!(
        apply(&root, "game", &Change::Create("Blocked".into()), |_, _| Ok(
            ()
        ))
        .is_err()
    );
    drop(guard);
    assert_eq!(
        fs::read_to_string(root.join("Original/file.txt"))?,
        "retained"
    );
    assert!(root.join("Existing").is_dir());
    assert!(!root.join("Blocked").exists());
    assert!(!project.path().join("escape").exists());
    for name in [
        "", ".", "..", "a/b", "a\\b", " x", "x ", "x.", "CON", "NUL.txt", "COM1", "x.meta",
    ] {
        assert!(kedra_authoring_fs::entries::name(name).is_err(), "{name}");
    }
    assert!(kedra_authoring_fs::entries::name("Текстуры 日本語").is_ok());
    Ok(())
}

#[cfg(unix)]
#[test]
fn linked_folder_and_linked_contents_never_move_external_targets() -> TestResult {
    let project = tempfile::tempdir()?;
    let root = project.path().join("assets");
    let external = project.path().join("external");
    fs::create_dir_all(root.join("Container"))?;
    fs::create_dir(&external)?;
    fs::write(external.join("file.txt"), "external")?;
    std::os::unix::fs::symlink(&external, root.join("Linked"))?;
    assert!(
        apply(&root, "game", &Change::Delete("Linked".into()), |_, _| Ok(
            ()
        ))
        .is_err()
    );
    fs::remove_file(root.join("Linked"))?;
    std::os::unix::fs::symlink(external.join("file.txt"), root.join("Container/link"))?;
    assert!(
        apply(
            &root,
            "game",
            &Change::Delete("Container".into()),
            |_, _| Ok(())
        )
        .is_err()
    );
    assert!(root.join("Container/link").is_symlink());
    assert_eq!(fs::read_to_string(external.join("file.txt"))?, "external");
    Ok(())
}
