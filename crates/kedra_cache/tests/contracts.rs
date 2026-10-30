//! The public project cache owns scope, byte admission and disposable persistence.
use kedra_cache::{Key, Limits, Store};
type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn persisted_namespaces_and_revisions_are_isolated_and_oversize_keeps_sources() -> Result {
    let project = tempfile::tempdir()?;
    let foreign = tempfile::tempdir()?;
    std::fs::write(project.path().join("source.txt"), b"source")?;
    let limits = Limits {
        entry_bytes: 8,
        total_bytes: 16,
        entries: 2,
    };
    let store = Store::project(project.path(), limits)?;
    let first = Key::new("thumbnails", 1, &[b"model", b"revision"]);
    let other = Key::new("another-provider", 1, &[b"model", b"revision"]);
    assert_eq!(store.get(&first)?, None);
    store.put(&first, b"pixels")?;
    store.put(&other, b"derived")?;
    let reopened = Store::project(project.path(), limits)?;
    assert_eq!(reopened.get(&first)?.as_deref(), Some(b"pixels".as_slice()));
    assert_eq!(
        reopened.get(&other)?.as_deref(),
        Some(b"derived".as_slice())
    );
    assert_eq!(Store::project(foreign.path(), limits)?.get(&first)?, None);
    assert_eq!(
        store.get(&Key::new("thumbnails", 2, &[b"model", b"revision"]))?,
        None
    );
    assert!(store.put(&first, b"oversized payload").is_err());
    assert_eq!(store.get(&first)?.as_deref(), Some(b"pixels".as_slice()));
    assert_eq!(std::fs::read(project.path().join("source.txt"))?, b"source");
    std::fs::remove_dir_all(project.path().join(".kedra/cache"))?;
    assert_eq!(store.get(&first)?, None);
    assert_eq!(std::fs::read(project.path().join("source.txt"))?, b"source");
    Ok(())
}

#[test]
fn global_quota_evicts_derived_entries_across_namespaces_without_touching_sources() -> Result {
    let project = tempfile::tempdir()?;
    let store = Store::project(
        project.path(),
        Limits {
            entry_bytes: 8,
            total_bytes: 8,
            entries: 1,
        },
    )?;
    let a = Key::new("a", 1, &[b"input"]);
    let b = Key::new("b", 1, &[b"input"]);
    store.put(&a, b"first")?;
    store.put(&b, b"second")?;
    assert_eq!(store.get(&a)?, None);
    assert_eq!(store.get(&b)?.as_deref(), Some(b"second".as_slice()));
    Ok(())
}

#[test]
fn missing_content_is_a_recoverable_miss_and_can_be_regenerated() -> Result {
    let project = tempfile::tempdir()?;
    let store = Store::project(
        project.path(),
        Limits {
            entry_bytes: 8,
            total_bytes: 8,
            entries: 1,
        },
    )?;
    let key = Key::new("thumb", 1, &[b"input"]);
    store.put(&key, b"pixels")?;
    let data = project.path().join(".kedra/cache/v1/data");
    let metadata = cacache::list_sync(&data)
        .next()
        .ok_or("Missing cache entry")??;
    cacache::remove_hash_sync(&data, &metadata.integrity)?;
    assert_eq!(store.get(&key)?, None);
    store.put(&key, b"pixels")?;
    assert_eq!(store.get(&key)?.as_deref(), Some(b"pixels".as_slice()));
    Ok(())
}

#[test]
fn invalid_limits_and_non_directory_cache_locations_do_not_overwrite_foreign_data() -> Result {
    let project = tempfile::tempdir()?;
    assert!(
        Store::project(
            project.path(),
            Limits {
                entry_bytes: 9,
                total_bytes: 8,
                entries: 1
            }
        )
        .is_err()
    );
    std::fs::write(project.path().join(".kedra"), b"foreign")?;
    let store = Store::project(project.path(), Limits::default())?;
    let key = Key::new("a", 1, &[]);
    assert!(store.get(&key).is_err());
    assert!(store.put(&key, b"value").is_err());
    assert_eq!(std::fs::read(project.path().join(".kedra"))?, b"foreign");
    Ok(())
}
