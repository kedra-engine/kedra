use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn listing_is_read_only_sorted_classified_and_bounded() -> TestResult {
    let root = tempfile::tempdir()?;
    std::fs::create_dir(root.path().join("日本語"))?;
    for name in [
        "b.prefab.ron",
        "a.scene.ron",
        "ignored.meta",
        "bad#name.scene.ron",
    ] {
        std::fs::write(root.path().join(name), "not parsed or executed")?;
    }
    let listing = list(root.path(), Path::new(""))?;
    assert_eq!(listing.entries.len(), 4);
    assert_eq!(listing.entries[0].kind, Kind::Folder);
    assert!(listing.entries.iter().any(|e| e.kind == Kind::Scene));
    assert!(listing.entries.iter().any(|e| e.kind == Kind::Unavailable));
    for i in 0..520 {
        std::fs::write(root.path().join(format!("file-{i:04}")), [])?;
    }
    let listing = list(root.path(), Path::new(""))?;
    assert!(listing.truncated);
    assert_eq!(listing.entries.len(), MAX_ENTRIES);
    assert_eq!(
        std::fs::read_to_string(root.path().join("b.prefab.ron"))?,
        "not parsed or executed"
    );
    assert!(list(root.path(), Path::new("../outside")).is_err());
    assert!(list(root.path(), Path::new("missing")).is_err());
    Ok(())
}
#[test]
fn stale_completion_failure_and_refresh_preserve_current_state() -> TestResult {
    let root = tempfile::tempdir()?;
    std::fs::write(root.path().join("é.txt"), [])?;
    let mut model = Model::default();
    let first = model.request(PathBuf::new(), None)?;
    let second = model.request(PathBuf::new(), None)?;
    assert!(!model.complete(&first, Err("obsolete".into())));
    assert!(model.complete(&second, list(root.path(), Path::new(""))));
    assert!(model.select(&OsString::from("é.txt")));
    model.filter = "É".into();
    assert_eq!(model.visible().count(), 1);
    let refresh = model.request(PathBuf::new(), None)?;
    model.complete(&refresh, Err("permission denied".into()));
    assert_eq!(
        model.selected.as_deref(),
        Some(std::ffi::OsStr::new("é.txt"))
    );
    assert_eq!(model.entries.len(), 1);
    assert!(!model.select(&OsString::from("é.txt")));
    let refresh = model.request(PathBuf::new(), None)?;
    model.complete(&refresh, list(root.path(), Path::new("")));
    assert_eq!(
        model.selected.as_deref(),
        Some(std::ffi::OsStr::new("é.txt"))
    );
    assert!(model.history(false).is_none());
    Ok(())
}

#[test]
fn unicode_search_and_history_keep_hidden_selection_and_scroll() -> TestResult {
    let root = tempfile::tempdir()?;
    std::fs::write(root.path().join("Straße.txt"), [])?;
    std::fs::create_dir(root.path().join("child"))?;
    let mut model = Model::default();
    let request = model.request(PathBuf::new(), None)?;
    model.complete(&request, list(root.path(), Path::new("")));
    model.select(&OsString::from("Straße.txt"));
    model.filter = "STRASSE".into();
    assert_eq!(model.visible().count(), 1);
    model.filter = "no matches".into();
    assert_eq!(model.visible().count(), 0);
    assert_eq!(
        model.selected.as_deref(),
        Some(std::ffi::OsStr::new("Straße.txt"))
    );
    model.remember(208.);
    let request = model.request("child".into(), None)?;
    model.complete(&request, list(root.path(), Path::new("child")));
    let (path, position) = model.history(false).ok_or("history")?;
    let request = model.request(path.clone(), Some(position))?;
    model.complete(&request, list(root.path(), &path));
    assert_eq!(model.scroll(), 208.);
    assert_eq!(model.filter, "no matches");
    assert_eq!(
        model.selected.as_deref(),
        Some(std::ffi::OsStr::new("Straße.txt"))
    );
    Ok(())
}
#[test]
fn history_publishes_only_successful_locations_and_failed_navigation_keeps_location() -> TestResult
{
    let root = tempfile::tempdir()?;
    std::fs::create_dir(root.path().join("child"))?;
    let mut model = Model::default();
    for path in ["", "child"] {
        let request = model.request(path.into(), None)?;
        model.complete(&request, list(root.path(), Path::new(path)));
    }
    let request = model.request("missing".into(), None)?;
    model.complete(&request, Err("missing".into()));
    assert_eq!(model.path, PathBuf::from("child"));
    let (path, position) = model.history(false).ok_or("back")?;
    let request = model.request(path.clone(), Some(position))?;
    model.complete(&request, list(root.path(), &path));
    assert_eq!(model.path, PathBuf::new());
    assert_eq!(
        model.history(true).map(|e| e.0),
        Some(PathBuf::from("child"))
    );
    Ok(())
}
#[cfg(unix)]
#[test]
fn links_special_files_and_non_unicode_names_cannot_be_navigated() -> TestResult {
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    let root = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    symlink(outside.path(), root.path().join("link"))?;
    // Some native filesystems reject invalid UTF-8 names before listing them.
    assert_eq!(
        classify(&OsString::from_vec(vec![0xff]), false),
        Kind::Unavailable
    );
    let _socket = std::os::unix::net::UnixListener::bind(root.path().join("socket"))?;
    assert!(list(root.path(), Path::new("link")).is_err());
    assert!(
        list(root.path(), Path::new(""))
            .is_ok_and(|l| l.entries.iter().all(|e| e.kind == Kind::Unavailable))
    );
    Ok(())
}
