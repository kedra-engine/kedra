//! Source ingestion owns no-clobber, identity allocation and rejected-batch integrity.
use kedra_import::ingest::import_files;
use std::{fs, path::Path};
type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn created_environment_has_admissible_source_and_identity_and_never_overwrites() -> Result {
    use kedra_import::ingest::create_environment;
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("Lighting"))?;
    create_environment(root.path(), Path::new("Lighting"), "夕方")?;
    let path = root.path().join("Lighting/夕方.environment.ron");
    let text = fs::read_to_string(&path)?;
    kedra_scene::environment::Environment::parse(&text)?;
    let meta = fs::read(path.with_extension("ron.meta"))?;
    let snapshot = kedra_authoring_fs::source::read(root.path(), "game")?;
    assert!(snapshot.assets[0].id.is_some());
    assert_eq!(snapshot.environments["Lighting/夕方.environment.ron"], text);
    assert!(create_environment(root.path(), Path::new("Lighting"), "夕方").is_err());
    assert_eq!(fs::read_to_string(&path)?, text);
    assert_eq!(fs::read(path.with_extension("ron.meta"))?, meta);
    for name in ["../escape", "bad.scene.ron", "CON", "bad/name"] {
        assert!(create_environment(root.path(), Path::new("Lighting"), name).is_err());
    }
    assert!(create_environment(root.path(), Path::new("../escape"), "day").is_err());
    let after = kedra_authoring_fs::source::read(root.path(), "game")?;
    assert_eq!(after.assets, snapshot.assets);
    assert_eq!(after.environments, snapshot.environments);
    Ok(())
}

#[test]
fn files_and_unicode_folder_trees_receive_fresh_metadata_without_moving_sources() -> Result {
    let outside = tempfile::tempdir()?;
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("Models"))?;
    let folder = outside.path().join("入口");
    fs::create_dir(&folder)?;
    fs::write(folder.join("Straße.txt"), "original")?;
    fs::write(folder.join("Straße.txt.meta"), "foreign identity")?;
    let single = outside.path().join("license.txt");
    fs::write(&single, "license")?;
    let mut progress = Vec::new();
    assert_eq!(
        import_files(
            root.path(),
            Path::new("Models"),
            &[folder.clone(), single.clone()],
            |p| progress.push(p.to_owned())
        )?,
        2
    );
    assert_eq!(
        fs::read_to_string(root.path().join("Models/入口/Straße.txt"))?,
        "original"
    );
    let metadata = fs::read_to_string(root.path().join("Models/入口/Straße.txt.meta"))?;
    assert!(metadata.contains("id:"));
    assert!(!metadata.contains("foreign identity"));
    assert_eq!(
        fs::read_to_string(folder.join("Straße.txt.meta"))?,
        "foreign identity"
    );
    assert_eq!(fs::read_to_string(single)?, "license");
    assert!(progress.iter().any(|p| p.contains("Imported")));
    Ok(())
}

#[test]
fn conflicts_and_invalid_names_reject_the_entire_batch_before_publication() -> Result {
    let outside = tempfile::tempdir()?;
    let root = tempfile::tempdir()?;
    let a = outside.path().join("a.txt");
    let b = outside.path().join("b.txt");
    fs::write(&a, "new a")?;
    fs::write(&b, "new b")?;
    fs::write(root.path().join("b.txt.meta"), "existing identity")?;
    assert!(import_files(root.path(), Path::new(""), &[a.clone(), b], |_| {}).is_err());
    assert!(!root.path().join("a.txt").exists());
    assert_eq!(
        fs::read_to_string(root.path().join("b.txt.meta"))?,
        "existing identity"
    );
    let invalid = outside.path().join("bad#name.txt");
    fs::write(&invalid, "bad")?;
    assert!(import_files(root.path(), Path::new(""), &[a.clone(), invalid], |_| {}).is_err());
    assert!(import_files(root.path(), Path::new("../escape"), &[a], |_| {}).is_err());
    assert!(!root.path().join("a.txt").exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn source_and_destination_links_refuse_without_writing_their_targets() -> Result {
    use std::os::unix::fs::symlink;
    let outside = tempfile::tempdir()?;
    let root = tempfile::tempdir()?;
    let file = outside.path().join("a.txt");
    fs::write(&file, "original")?;
    let link = outside.path().join("link.txt");
    symlink(&file, &link)?;
    assert!(import_files(root.path(), Path::new(""), &[link], |_| {}).is_err());
    symlink(outside.path(), root.path().join("linked"))?;
    assert!(
        import_files(
            root.path(),
            Path::new("linked"),
            std::slice::from_ref(&file),
            |_| {}
        )
        .is_err()
    );
    assert!(!outside.path().join("a.txt.meta").exists());
    assert_eq!(fs::read_to_string(file)?, "original");
    Ok(())
}

#[test]
fn imported_directory_cannot_occupy_another_assets_sidecar_path() -> Result {
    let outside = tempfile::tempdir()?;
    let root = tempfile::tempdir()?;
    let file = outside.path().join("asset.txt");
    let folder = outside.path().join("asset.txt.meta");
    fs::write(&file, "original")?;
    fs::create_dir(&folder)?;
    fs::write(folder.join("child.txt"), "child")?;
    assert!(import_files(root.path(), Path::new(""), &[file, folder], |_| {}).is_err());
    assert_eq!(fs::read_dir(root.path())?.count(), 0);
    Ok(())
}
