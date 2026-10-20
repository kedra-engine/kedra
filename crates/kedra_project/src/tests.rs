#![allow(
    clippy::unwrap_used,
    reason = "Assertions in isolated filesystem tests"
)]
use super::*;
use std::fs;

#[test]
fn create_open_roundtrip_preserves_unicode_and_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let project = create(dir.path(), "Кедра 日本語").unwrap();
    assert_eq!(open(&project.root).unwrap(), project);
    let before = fs::read(project.root.join(MANIFEST_FILE)).unwrap();
    assert!(create(dir.path(), "Кедра 日本語").is_err());
    assert_eq!(fs::read(project.root.join(MANIFEST_FILE)).unwrap(), before);
    assert!(project.assets.is_dir());
}

#[test]
fn invalid_names_and_missing_parent_leave_no_project() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "", ".", "..", "a/b", "a\\b", " name", "name.", "CON", "lpt1.txt", "a\n",
    ] {
        assert!(create(dir.path(), name).is_err(), "{name:?}");
    }
    assert!(create(&dir.path().join("missing"), "Game").is_err());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn creation_preflight_never_reserves_or_overwrites_a_target() {
    let dir = tempfile::tempdir().unwrap();
    let target = validate_creation(dir.path(), "Game").unwrap();
    assert!(!target.exists());
    fs::create_dir(&target).unwrap();
    assert!(validate_creation(dir.path(), "Game").is_err());
    assert!(create(dir.path(), "Game").is_err());
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    let file = dir.path().join("file");
    fs::write(&file, "content").unwrap();
    assert!(validate_creation(&file, "Game").is_err());
    assert!(validate_creation(dir.path(), "CON").is_err());
}

#[test]
fn open_is_read_only_and_compatibility_uses_format_not_release() {
    let dir = tempfile::tempdir().unwrap();
    let project = create(dir.path(), "Game").unwrap();
    let path = project.root.join(MANIFEST_FILE);
    let mut manifest = project.manifest;
    manifest.editor_version = "99.0.0".into();
    let source = toml::to_string(&manifest).unwrap();
    fs::write(&path, &source).unwrap();
    assert_eq!(
        open(&project.root).unwrap().manifest.editor_version,
        "99.0.0"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    for format in [0, 2] {
        manifest.format = format;
        let source = toml::to_string(&manifest).unwrap();
        fs::write(&path, &source).unwrap();
        assert!(matches!(
            open(&project.root),
            Err(ProjectError::UnsupportedFormat { .. })
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
}

#[test]
fn rejects_missing_malformed_oversized_and_escaping_sources() {
    let dir = tempfile::tempdir().unwrap();
    assert!(open(dir.path()).is_err());
    let project = create(dir.path(), "Game").unwrap();
    let path = project.root.join(MANIFEST_FILE);
    let mut manifest = project.manifest;
    for assets in ["../", "/", ""] {
        manifest.assets = assets.into();
        fs::write(&path, toml::to_string(&manifest).unwrap()).unwrap();
        assert!(open(&project.root).is_err());
    }
    for source in [
        "[broken".to_string(),
        "x".repeat(65537),
        "format = -1".into(),
    ] {
        fs::write(&path, source).unwrap();
        assert!(open(&project.root).is_err());
    }
}

#[cfg(unix)]
#[test]
fn rejects_symlink_asset_directory_outside_project() {
    let dir = tempfile::tempdir().unwrap();
    let project = create(dir.path(), "Game").unwrap();
    fs::remove_dir(&project.assets).unwrap();
    std::os::unix::fs::symlink(dir.path(), &project.assets).unwrap();
    assert!(open(&project.root).is_err());
}

#[test]
fn history_is_bounded_deduplicated_and_reports_corruption() {
    use recent::{MAX_RECENT, RecentProjects};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preferences/recent.toml");
    let mut history = RecentProjects::load(&path).unwrap();
    assert!(history.entries().is_empty());
    for n in 0..25 {
        history.remember(&create(dir.path(), &format!("Game {n}")).unwrap());
    }
    let project = open(&dir.path().join("Game 10")).unwrap();
    history.remember(&project);
    assert_eq!(history.entries().len(), MAX_RECENT);
    assert_eq!(history.entries()[0].root, project.root);
    history.save(&path).unwrap();
    assert_eq!(
        RecentProjects::load(&path).unwrap().entries(),
        history.entries()
    );
    history.save(&path).unwrap();
    fs::write(&path, "broken history").unwrap();
    assert!(RecentProjects::load(&path).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "broken history");
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, "file").unwrap();
    assert!(history.save(&blocker.join("recent.toml")).is_err());
}

#[test]
fn history_rejects_invalid_schema_roots_names_encoding_and_byte_limits_without_rewriting() {
    use recent::{RecentProject, RecentProjects};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("recent.toml");
    let entry = toml::to_string(&RecentProject {
        name: "Game".into(),
        root: directory.path().canonicalize().unwrap(),
    })
    .unwrap();
    let mut invalid = vec![
        b"format=2\nentries=[]".to_vec(),
        b"format=1\nentries=[]\nunknown=true".to_vec(),
        vec![255],
        vec![b' '; 65_537],
        format!("format=1\n{}", format!("[[entries]]\n{entry}").repeat(21)).into_bytes(),
        format!("format=1\n[[entries]]\n{entry}\n[[entries]]\n{entry}").into_bytes(),
    ];
    for (name, root) in [("Game", "relative"), ("bad/name", "/absolute")] {
        let entry = toml::to_string(&RecentProject {
            name: name.into(),
            root: root.into(),
        })
        .unwrap();
        invalid.push(format!("format=1\n[[entries]]\n{entry}").into_bytes());
    }
    for bytes in invalid {
        fs::write(&path, &bytes).unwrap();
        assert!(RecentProjects::load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    let mut exact = b"format=1\nentries=[]\n".to_vec();
    exact.resize(65_536, b' ');
    fs::write(&path, exact).unwrap();
    assert!(RecentProjects::load(&path).unwrap().entries().is_empty());
}

#[test]
fn history_file_failures_do_not_replace_existing_content() {
    use recent::RecentProjects;
    let directory = tempfile::tempdir().unwrap();
    let history = RecentProjects::default();
    assert!(history.save(std::path::Path::new("")).is_err());
    let destination = directory.path().join("directory-not-file");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("sentinel"), "keep").unwrap();
    assert!(RecentProjects::load(&destination).is_err());
    assert!(history.save(&destination).is_err());
    assert_eq!(
        fs::read_to_string(destination.join("sentinel")).unwrap(),
        "keep"
    );
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn project_identity_schema_and_regular_file_failures_leave_manifest_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let project = create(directory.path(), "Game").unwrap();
    let path = project.root.join(MANIFEST_FILE);
    let original = toml::to_string(&project.manifest).unwrap();
    let mut nil = project.manifest.clone();
    nil.id = uuid::Uuid::nil();
    let mut empty_release = project.manifest.clone();
    empty_release.editor_version = " ".into();
    for bytes in [
        toml::to_string(&nil).unwrap().into_bytes(),
        toml::to_string(&empty_release).unwrap().into_bytes(),
        format!("{original}\nunknown=true").into_bytes(),
        vec![255],
        b"format=1.5".to_vec(),
    ] {
        fs::write(&path, &bytes).unwrap();
        assert!(open(&project.root).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::write(&path, &original).unwrap();
    fs::remove_dir(&project.assets).unwrap();
    fs::write(&project.assets, "not a directory").unwrap();
    assert!(open(&project.root).is_err());
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(open(&project.root).is_err());
}

#[cfg(unix)]
#[test]
fn static_regular_manifest_and_history_symlinks_remain_supported() {
    use recent::RecentProjects;
    let directory = tempfile::tempdir().unwrap();
    let project = create(directory.path(), "Game").unwrap();
    let manifest = project.root.join(MANIFEST_FILE);
    let target = project.root.join("source-manifest.toml");
    fs::rename(&manifest, &target).unwrap();
    std::os::unix::fs::symlink(&target, &manifest).unwrap();
    assert_eq!(open(&project.root).unwrap(), project);
    let history = directory.path().join("history.toml");
    RecentProjects::default().save(&history).unwrap();
    let link = directory.path().join("history-link.toml");
    std::os::unix::fs::symlink(&history, &link).unwrap();
    assert!(RecentProjects::load(&link).unwrap().entries().is_empty());
}
