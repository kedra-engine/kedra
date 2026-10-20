//! Public import admission failures must not publish or modify authored inputs.
use std::fs;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn manifest_growth_is_rejected_before_public_output_and_smaller_delivery_roundtrips() -> Result<()>
{
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    for n in 0..160 {
        fs::write(source.join(format!("data-{n:03}.txt")), "data")?;
    }
    kedra_import::initialize(&source, |_| {})?;
    let before = fs::read(source.join("data-000.txt.meta"))?;
    let output = temp.path().join("too-large");
    let error = kedra_import::build(&source, &output, "game", "game", |_| {})
        .err()
        .ok_or("oversized manifest was published")?;
    assert!(
        error.contains("manifest") && error.contains("64 KiB"),
        "{error}"
    );
    assert!(!output.exists());
    assert_eq!(fs::read(source.join("data-000.txt.meta"))?, before);
    for n in 150..160 {
        fs::remove_file(source.join(format!("data-{n:03}.txt")))?;
        fs::remove_file(source.join(format!("data-{n:03}.txt.meta")))?;
    }
    let smaller = temp.path().join("smaller");
    kedra_import::build(&source, &smaller, "game", "game", |_| {})?;
    let (_, manifest) = kedra_assets::delivery::read_directory(&smaller, "game")?;
    assert_eq!(manifest.assets.len(), 150);
    Ok(())
}

#[test]
fn native_uri_delimiters_in_source_paths_are_rejected_without_initialization() -> Result<()> {
    for name in ["actor#variant.txt", "parent#variant/actor.txt"] {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("source");
        let asset = source.join(name);
        fs::create_dir_all(asset.parent().ok_or("missing parent")?)?;
        fs::write(&asset, "original")?;
        let error = kedra_import::initialize(&source, |_| {})
            .err()
            .ok_or("unaddressable source was initialized")?;
        assert!(error.contains("path"), "{error}");
        assert_eq!(fs::read_to_string(&asset)?, "original");
        assert!(!asset.with_extension("txt.meta").exists());
        let output = temp.path().join("output");
        assert!(kedra_import::build(&source, &output, "game", "game", |_| {}).is_err());
        assert!(!output.exists());
    }
    Ok(())
}

#[test]
fn malformed_metadata_unknown_processor_and_invalid_options_preserve_sources() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    fs::write(source.join("data.txt"), "original")?;
    kedra_import::initialize(&source, |_| {})?;
    let output = temp.path().join("output");
    for (profile, namespace) in [
        ("", "game"),
        (&"p".repeat(65), "game"),
        ("game", ""),
        ("game", "bad#source"),
        ("game", &"s".repeat(65)),
    ] {
        assert!(kedra_import::build(&source, &output, profile, namespace, |_| {}).is_err());
        assert!(!output.exists());
    }
    for metadata in [
        "not RON".into(),
        format!(
            "(id:{:?},meta_format_version:\"1.0\",asset:Process(processor:\"missing.Processor\",settings:()))",
            uuid::Uuid::new_v4().to_string()
        ),
    ] {
        fs::write(source.join("data.txt.meta"), &metadata)?;
        assert!(kedra_import::build(&source, &output, "game", "game", |_| {}).is_err());
        assert!(!output.exists());
        assert_eq!(fs::read_to_string(source.join("data.txt.meta"))?, metadata);
        assert_eq!(fs::read_to_string(source.join("data.txt"))?, "original");
    }
    Ok(())
}

#[test]
fn source_changes_during_processing_do_not_publish_a_stale_delivery() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    fs::write(source.join("data.txt"), "before")?;
    kedra_import::initialize(&source, |_| {})?;
    let sidecar = fs::read(source.join("data.txt.meta"))?;
    let output = temp.path().join("output");
    let mut changed = false;
    let mut mutation = Ok(());
    let result = kedra_import::build(&source, &output, "game", "game", |message| {
        if !changed && message.starts_with("Processing ") {
            mutation = fs::write(source.join("data.txt"), "after");
            changed = true;
        }
    });
    mutation?;
    assert!(changed);
    let error = result.err().ok_or("changed input was published")?;
    assert!(error.contains("Sources changed"), "{error}");
    assert!(!output.exists());
    assert_eq!(fs::read(source.join("data.txt.meta"))?, sidecar);
    Ok(())
}

#[test]
fn initialization_preserves_native_settings_ignored_sources_and_missing_input_errors() -> Result<()>
{
    let temp = tempfile::tempdir()?;
    assert!(kedra_import::initialize(&temp.path().join("missing"), |_| {}).is_err());
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    for name in ["data.json", "data.ron", "notice.txt", "unknown.bin"] {
        fs::write(source.join(name), "data")?;
    }
    let mut messages = Vec::new();
    assert_eq!(
        kedra_import::initialize(&source, |message| messages.push(message.to_owned()))?,
        4
    );
    assert_eq!(messages.len(), 4);
    let ignored = fs::read(source.join("unknown.bin.meta"))?;
    assert!(std::str::from_utf8(&ignored)?.contains("Ignore"));
    assert_eq!(kedra_import::initialize(&source, |_| {})?, 0);
    let output = temp.path().join("output");
    let manifest = kedra_import::build(&source, &output, "game", "game", |_| {})?;
    assert_eq!(manifest.assets.len(), 3);
    assert_eq!(fs::read(source.join("unknown.bin.meta"))?, ignored);
    let before = fs::read(output.join(kedra_assets::delivery::MANIFEST))?;
    assert!(kedra_import::build(&source, &output, "game", "game", |_| {}).is_err());
    assert_eq!(
        fs::read(output.join(kedra_assets::delivery::MANIFEST))?,
        before
    );
    assert!(kedra_import::build(&source, &source.join("nested"), "game", "game", |_| {}).is_err());
    Ok(())
}

#[test]
fn invalid_fonts_images_and_scene_sources_fail_without_publication() -> Result<()> {
    for name in ["bad.ttf", "bad.otf", "bad.png", "bad.prefab.ron"] {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("source");
        fs::create_dir(&source)?;
        fs::write(source.join(name), "invalid source")?;
        let initialized = kedra_import::initialize(&source, |_| {});
        let output = temp.path().join("output");
        if name.ends_with(".prefab.ron") {
            assert!(initialized.is_err());
            assert!(!source.join(format!("{name}.meta")).exists());
        } else {
            initialized?;
            let before = fs::read(source.join(format!("{name}.meta")))?;
            assert!(kedra_import::build(&source, &output, "game", "game", |_| {}).is_err());
            assert_eq!(fs::read(source.join(format!("{name}.meta")))?, before);
        }
        assert!(!output.exists());
        assert_eq!(fs::read_to_string(source.join(name))?, "invalid source");
    }
    Ok(())
}

#[test]
fn inventory_changes_during_verification_fail_without_publication() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    fs::write(source.join("data.txt"), "original")?;
    kedra_import::initialize(&source, |_| {})?;
    let output = temp.path().join("output");
    let mut inserted = false;
    let mut mutation = Ok(());
    let result = kedra_import::build(&source, &output, "game", "game", |message| {
        if message.starts_with("Verifying native runtime") {
            mutation = fs::write(source.join("new.txt"), "new authored source");
            inserted = true;
        }
    });
    mutation?;
    assert!(inserted);
    let error = result.err().ok_or("changed inventory was published")?;
    assert!(error.contains("inventory changed"), "{error}");
    assert!(!output.exists());
    assert_eq!(fs::read_to_string(source.join("data.txt"))?, "original");
    Ok(())
}

#[cfg(unix)]
#[test]
fn source_symlinks_and_dangling_output_symlinks_are_never_replaced() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fs::create_dir(&source)?;
    let outside = temp.path().join("outside.txt");
    fs::write(&outside, "keep")?;
    let linked = source.join("linked.txt");
    std::os::unix::fs::symlink(&outside, &linked)?;
    assert!(kedra_import::initialize(&source, |_| {}).is_err());
    let output = temp.path().join("output");
    assert!(kedra_import::build(&source, &output, "game", "game", |_| {}).is_err());
    assert!(!output.exists());
    assert_eq!(fs::read_to_string(&outside)?, "keep");
    fs::remove_file(&linked)?;
    fs::write(source.join("data%23literal.txt"), "data")?;
    kedra_import::initialize(&source, |_| {})?;
    std::os::unix::fs::symlink(temp.path().join("absent"), &output)?;
    assert!(kedra_import::build(&source, &output, "game", "game", |_| {}).is_err());
    assert!(fs::symlink_metadata(&output)?.file_type().is_symlink());
    assert!(!temp.path().join("absent").exists());
    let valid = temp.path().join("valid");
    kedra_import::build(&source, &valid, "game", "game", |_| {})?;
    let (_, manifest) = kedra_assets::delivery::read_directory(&valid, "game")?;
    assert_eq!(manifest.assets[0].path, "data%23literal.txt");
    Ok(())
}
