use super::*;
use bevy::prelude::*;

#[test]
fn source_failure_labels_keep_context_without_absolute_filesystem_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let missing = root.path().join("日本語.png");
    let error = bytes(root.path(), &missing, 100)
        .err()
        .ok_or("Missing source was admitted")?;
    assert!(error.contains(&crate::display_paths::asset(Path::new("日本語.png"))));
    assert!(!error.contains(root.path().to_string_lossy().as_ref()));
    std::fs::write(root.path().join("bad.png"), b"too large")?;
    let error = bytes(root.path(), &root.path().join("bad.png"), 1)
        .err()
        .ok_or("Oversized source was admitted")?;
    assert!(error.contains("assets"));
    assert!(error.contains("bad.png"));
    assert!(!error.contains(root.path().to_string_lossy().as_ref()));
    std::fs::write(root.path().join("bad.png.meta"), b"invalid sidecar")?;
    let error = read(root.path())
        .err()
        .ok_or("Invalid source was admitted")?;
    assert!(error.contains("bad.png.meta"));
    assert!(!error.contains(root.path().to_string_lossy().as_ref()));
    Ok(())
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct ImageReference {
    image: Handle<Image>,
}

#[test]
fn reflected_asset_components_validate_without_materializing_runtime_assets()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let id = Uuid::new_v4();
    std::fs::write(
        root.path().join("icon.svg"),
        "source bytes are not runtime-decoded by Open",
    )?;
    std::fs::write(
        root.path().join("icon.svg.meta"),
        format!(
            "(id:\"{id}\",meta_format_version:\"1.0\",asset:Load(loader:\"source-fixture\",settings:()))"
        ),
    )?;
    let mut document = PrefabDocument::new("Source with asset reference");
    document.nodes[0].components.insert(
        "fixture.image".into(),
        kedra_scene::document::ComponentData::ron(1, "(image:Path(\"game://icon.svg#2x\"))")?,
    );
    std::fs::write(root.path().join("source.prefab.ron"), document.serialize()?)?;
    let mut types = TypeRegistry::default();
    types.register::<Handle<Image>>();
    let mut policies = ComponentPolicies::default();
    policies.register::<ImageReference>(&mut types, "fixture.image", 1, |_| Ok(()))?;
    let scope = admit(read(root.path())?, &types, &policies)?;
    assert_eq!(scope.working.documents().len(), 1);
    std::fs::remove_file(root.path().join("icon.svg.meta"))?;
    assert!(admit(read(root.path())?, &types, &policies).is_err());
    assert_eq!(scope.working.documents().len(), 1);
    Ok(())
}
#[test]
fn source_handles_validate_native_address_forms_without_runtime_loading() {
    let id = Uuid::new_v4();
    let snapshot = SourceSnapshot {
        namespace: "game".into(),
        generation: 1,
        documents: Vec::new(),
        assets: vec![SourceAsset {
            id: Some(id),
            path: "icons/a.svg".into(),
            hash: "0".repeat(64),
            bytes: 16,
        }],
        index: None,
    };
    for valid in [
        "game://icons/a.svg#2x".to_string(),
        format!("game://{id}/asset.svg#2x"),
    ] {
        let mut loader = SourceHandles {
            source: &snapshot,
            error: None,
        };
        let _ = loader
            .load_from_path_erased(TypeId::of::<bevy::prelude::Image>(), AssetPath::from(valid));
        assert!(loader.error.is_none());
    }
    for invalid in [
        format!("game://{id}/not-an-asset"),
        "editor://icons/a.svg".into(),
        "game://missing.svg".into(),
    ] {
        let mut loader = SourceHandles {
            source: &snapshot,
            error: None,
        };
        let _ = loader.load_from_path_erased(
            TypeId::of::<bevy::prelude::Image>(),
            AssetPath::from(invalid),
        );
        assert!(loader.error.is_some());
    }
}

#[test]
fn source_recheck_rejects_changed_sidecars_missing_and_oversized_files()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let source = PrefabDocument::new("Source").serialize()?;
    let path = Path::new("source.prefab.ron");
    std::fs::write(root.path().join(path), &source)?;
    let expected = digest(source.as_bytes());
    check(root.path(), path, &expected)?;
    std::fs::write(
        root.path().join("source.prefab.ron.meta"),
        format!(
            "(id:\"{}\",meta_format_version:\"1.0\",asset:Load(loader:\"source-fixture\",settings:()))",
            Uuid::new_v4()
        ),
    )?;
    assert!(check(root.path(), path, &expected).is_err());
    std::fs::remove_file(root.path().join("source.prefab.ron.meta"))?;
    std::fs::write(root.path().join(path), "x".repeat(65_537))?;
    assert!(read(root.path()).is_err());
    std::fs::remove_file(root.path().join(path))?;
    assert!(check(root.path(), path, &expected).is_err());
    Ok(())
}

#[cfg(unix)]
#[test]
fn source_inventory_refuses_linked_and_special_members() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    std::os::unix::fs::symlink(outside.path(), root.path().join("linked"))?;
    assert!(read(root.path()).is_err());
    std::fs::remove_file(root.path().join("linked"))?;
    let _socket = std::os::unix::net::UnixListener::bind(root.path().join("socket"))?;
    assert!(read(root.path()).is_err());
    Ok(())
}
#[test]
fn asset_recheck_rejects_sidecar_identity_replacement_and_removal()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let data = b"bounded test image bytes";
    std::fs::write(root.path().join("image.png"), data)?;
    let id = uuid::Uuid::new_v4();
    let meta = |id| {
        format!(
            "(id:\"{id}\",meta_format_version:\"1.0\",asset:Load(loader:\"bevy_image::image_loader::ImageLoader\",settings:()))"
        )
    };
    std::fs::write(root.path().join("image.png.meta"), meta(id))?;
    super::check_inventory(
        root.path(),
        std::path::Path::new("image.png"),
        &kedra_assets::delivery::digest(data),
        Some(id),
    )?;
    std::fs::write(
        root.path().join("image.png.meta"),
        meta(uuid::Uuid::new_v4()),
    )?;
    assert!(
        super::check_inventory(
            root.path(),
            std::path::Path::new("image.png"),
            &kedra_assets::delivery::digest(data),
            Some(id)
        )
        .is_err()
    );
    std::fs::remove_file(root.path().join("image.png.meta"))?;
    assert!(
        super::check_inventory(
            root.path(),
            std::path::Path::new("image.png"),
            &kedra_assets::delivery::digest(data),
            Some(id)
        )
        .is_err()
    );
    Ok(())
}
