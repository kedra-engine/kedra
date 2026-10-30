use bevy::reflect::ParsedPath;
use kedra_authoring::{environment::Edits, persistence::content_hash};

#[test]
fn environment_root_edits_save_existing_bytes_and_preserve_identity_and_conflicts()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("assets");
    std::fs::create_dir(&root)?;
    let text = "(version:1,background:Srgba((red:0.1,green:0.2,blue:0.3,alpha:1.0)),ambient_color:Srgba((red:1.0,green:1.0,blue:1.0,alpha:1.0)),ambient_brightness:100.0)";
    std::fs::write(root.join("day.environment.ron"), text)?;
    let metadata = r#"(id:"068849e8-2884-45ad-b814-eb80c88630e0",meta_format_version:"1.0",asset:Load(loader:"kedra_scene::environment::EnvironmentLoader",settings:()))"#;
    std::fs::write(root.join("day.environment.ron.meta"), metadata)?;
    let mut source = kedra_authoring_fs::source::read(&root, "game")?;
    let mut edits = Edits::default();
    let path = "day.environment.ron";
    edits.open(&source, path)?;
    let field = ParsedPath::parse("ambient_brightness")?;
    assert!(edits.edit(path, 1, &field, &-1.0f32).is_err());
    assert_eq!(edits.value(path).ok_or("Missing root")?.1, 1);
    assert!(!edits.unsettled());
    edits.edit(path, 1, &field, &400.0f32)?;
    assert!(edits.edit(path, 1, &field, &500.0f32).is_err());
    assert_eq!(std::fs::read_to_string(root.join(path))?, text);
    edits.history(path, false)?;
    assert!(!edits.dirty(path));
    edits.history(path, true)?;
    let request = edits.prepare_save(&source, path)?;
    assert_eq!(
        edits.prepare_save(&source, path)?.operation,
        request.operation
    );
    let receipt = kedra_authoring_fs::commit_environment(&root, &request, &source.assets[0])?;
    let mut foreign = receipt.clone();
    foreign.files[0].hash = Some("wrong".into());
    assert!(edits.acknowledge(&mut source, &foreign).is_err());
    assert!(edits.unsettled());
    edits.acknowledge(&mut source, &receipt)?;
    assert!(!edits.unsettled());
    let saved = std::fs::read_to_string(root.join(path))?;
    assert_eq!(
        std::fs::read_to_string(root.join("day.environment.ron.meta"))?,
        metadata
    );
    assert_eq!(source.assets[0].hash, content_hash(&saved));
    assert_eq!(
        kedra_scene::environment::Environment::parse(&saved)?.ambient_brightness,
        400.0
    );
    let revision = edits.value(path).ok_or("Missing root")?.1;
    edits.edit(path, revision, &field, &600.0f32)?;
    let request = edits.prepare_save(&source, path)?;
    std::fs::write(root.join(path), text)?;
    assert!(kedra_authoring_fs::commit_environment(&root, &request, &source.assets[0]).is_err());
    assert_eq!(std::fs::read_to_string(root.join(path))?, text);
    assert!(edits.dirty(path));
    std::fs::write(root.join(path), &saved)?;
    let replaced = metadata.replace(
        "068849e8-2884-45ad-b814-eb80c88630e0",
        "568849e8-2884-45ad-b814-eb80c88630e0",
    );
    std::fs::write(root.join("day.environment.ron.meta"), &replaced)?;
    assert!(kedra_authoring_fs::commit_environment(&root, &request, &source.assets[0]).is_err());
    assert_eq!(std::fs::read_to_string(root.join(path))?, saved);
    assert_eq!(
        std::fs::read_to_string(root.join("day.environment.ron.meta"))?,
        replaced
    );
    Ok(())
}
