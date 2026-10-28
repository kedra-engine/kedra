use super::*;
use bevy::prelude::*;
use bevy::reflect::TypeRegistry;
use kedra_assets::delivery::digest;
use kedra_authoring::source::{admit, compile_working};
use kedra_scene::reflection::ComponentPolicies;
use std::collections::BTreeMap;

#[test]
fn source_failure_labels_keep_context_without_absolute_filesystem_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let missing = root.path().join("日本語.png");
    let error = bytes(root.path(), &missing, 100)
        .err()
        .ok_or("Missing source was admitted")?;
    assert!(error.contains(&Path::new("assets").join("日本語.png").display().to_string()));
    assert!(!error.contains(root.path().to_string_lossy().as_ref()));
    std::fs::write(root.path().join("bad.png"), b"too large")?;
    let error = bytes(root.path(), &root.path().join("bad.png"), 1)
        .err()
        .ok_or("Oversized source was admitted")?;
    assert!(error.contains("assets"));
    assert!(error.contains("bad.png"));
    assert!(!error.contains(root.path().to_string_lossy().as_ref()));
    std::fs::write(root.path().join("bad.png.meta"), b"invalid sidecar")?;
    let error = read(root.path(), "game")
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
    let (mut scope, _) = admit(read(root.path(), "game")?, &types, &policies)?;
    assert_eq!(scope.working.documents().len(), 1);
    assert!(compile_working(&scope, &types, &policies)?.contains_key(&document.id));
    scope.working.transact_checked(
        &BTreeMap::from([(document.id, 1)]),
        |docs| {
            docs.get_mut(&document.id)
                .ok_or_else(|| kedra_scene::Error("document".into()))?
                .nodes[0]
                .components
                .insert(
                    "fixture.image".into(),
                    kedra_scene::document::ComponentData::ron(
                        1,
                        "(image:Path(\"game://outside.svg\"))",
                    )?,
                );
            Ok(())
        },
        |_| Ok(()),
    )?;
    let before = ron::to_string(&scope)?;
    assert!(
        compile_working(&scope, &types, &policies).is_err(),
        "Refresh must not admit an asset outside the original inventory"
    );
    assert_eq!(ron::to_string(&scope)?, before);
    std::fs::remove_file(root.path().join("icon.svg.meta"))?;
    assert!(admit(read(root.path(), "game")?, &types, &policies).is_err());
    assert_eq!(scope.working.documents().len(), 1);
    Ok(())
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
    assert!(read(root.path(), "game").is_err());
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
    assert!(read(root.path(), "game").is_err());
    std::fs::remove_file(root.path().join("linked"))?;
    let _socket = std::os::unix::net::UnixListener::bind(root.path().join("socket"))?;
    assert!(read(root.path(), "game").is_err());
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
