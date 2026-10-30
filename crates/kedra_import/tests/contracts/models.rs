//! Shared admission precedes the native Bevy glTF loader in the delivery pipeline.
use bevy::{
    asset::meta::{AssetAction, AssetMeta},
    gltf::{
        GltfLoader,
        gltf::binary::{Glb, Header},
    },
};
use std::borrow::Cow;

#[test]
fn independent_model_selectors_and_ignored_raw_glb_fail_before_processing()
-> Result<(), Box<dyn std::error::Error>> {
    let cycle = serde_json::json!({"asset":{"version":"2.0"},"nodes":[{"children":[0]}],"scenes":[{"nodes":[0]}]});
    let mut binary = Vec::new();
    Glb {
        header: Header {
            magic: *b"glTF",
            version: 2,
            length: 0,
        },
        json: Cow::Owned(serde_json::to_vec(&cycle)?),
        bin: None,
    }
    .to_writer(&mut binary)?;
    for (path, bytes, action) in [
        ("renamed.bin", binary.clone(), "Ignore"),
        (
            "model.bin",
            serde_json::to_vec(&cycle)?,
            "Load(loader:\"bevy_gltf::loader::GltfLoader\",settings:())",
        ),
        ("malformed.GLB", b"not a binary model".to_vec(), "Ignore"),
        ("ignored.glb", binary, "Ignore"),
    ] {
        let directory = tempfile::tempdir()?;
        let source = directory.path().join("source");
        std::fs::create_dir(&source)?;
        let meta = format!(
            "(id:{:?},meta_format_version:\"1.0\",asset:{action})",
            uuid::Uuid::new_v4().to_string()
        );
        std::fs::write(source.join(path), &bytes)?;
        std::fs::write(source.join(format!("{path}.meta")), &meta)?;
        let output = directory.path().join("delivery");
        let mut processing_started = false;
        let error = kedra_import::build(&source, &output, "test", "game", |message| {
            processing_started |= message.starts_with("Processing ");
        })
        .err()
        .ok_or("unsafe raw source published")?;
        assert!(error.contains("GLB"), "{path}: {error}");
        assert!(
            !processing_started,
            "{path}: processor started before admission"
        );
        assert!(!output.exists());
        assert_eq!(std::fs::read(source.join(path))?, bytes);
        assert_eq!(
            std::fs::read_to_string(source.join(format!("{path}.meta")))?,
            meta
        );
    }
    Ok(())
}

#[test]
fn native_loader_receives_reader_failure_for_integrity_matching_cycle()
-> Result<(), Box<dyn std::error::Error>> {
    let Some(directory) = std::env::var_os("KEDRA_IMPORT_MODEL_PROBE") else {
        let temp = tempfile::tempdir()?;
        return crate::support::bounded_probe(
            "models::native_loader_receives_reader_failure_for_integrity_matching_cycle",
            &[("KEDRA_IMPORT_MODEL_PROBE", temp.path().as_os_str())],
            std::time::Duration::from_secs(30),
        );
    };
    use bevy::{
        asset::{AssetApp, AssetLoadError, AssetPlugin, LoadState},
        prelude::*,
    };
    use kedra_assets::delivery::{AssetRecord, FileDigest, Manifest, digest};
    let directory = std::path::PathBuf::from(directory);
    let temp = directory.as_path();
    Glb {
        header: Header {
            magic: *b"glTF",
            version: 2,
            length: 0,
        },
        json: Cow::Owned(serde_json::to_vec(&serde_json::json!({
            "asset":{"version":"2.0"}, "nodes":[{"children":[0]}], "scenes":[{"nodes":[0]}]
        }))?),
        bin: None,
    }
    .to_writer(std::fs::File::create(temp.join("bad.glb"))?)?;
    kedra_import::initialize(temp, |_| {})?;
    let mut manifest = Manifest {
        format: 1,
        profile: "test".into(),
        assets: vec![AssetRecord {
            id: uuid::Uuid::new_v4(),
            path: "bad.glb".into(),
            loader: "bevy_gltf::loader::GltfLoader".into(),
        }],
        files: Default::default(),
    };
    for member in ["bad.glb", "bad.glb.meta"] {
        let bytes = std::fs::read(temp.join(member))?;
        manifest.files.insert(
            member.into(),
            FileDigest {
                bytes: bytes.len() as u64,
                sha256: digest(&bytes),
            },
        );
    }
    manifest.verify("test", |path| {
        std::fs::read(temp.join(path)).map_err(|e| e.to_string())
    })?;
    let mut app = App::new();
    kedra_assets::register_source(
        &mut app,
        "game",
        temp.to_str().ok_or("fixture path")?.into(),
        manifest,
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            use_asset_processor_override: Some(false),
            watch_for_changes_override: Some(false),
            ..default()
        },
        bevy::image::ImagePlugin::default(),
        bevy::mesh::MeshPlugin,
    ))
    .init_asset::<bevy::pbr::StandardMaterial>()
    .init_asset::<bevy::world_serialization::WorldAsset>()
    .insert_resource(bevy::image::CompressedImageFormatSupport(
        bevy::image::CompressedImageFormats::NONE,
    ))
    .add_plugins(bevy::gltf::GltfPlugin::default());
    app.finish();
    app.cleanup();
    let server = app.world().resource::<AssetServer>().clone();
    let handle: Handle<bevy::gltf::Gltf> = server.load("game://bad.glb");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        app.update();
        if let Some(LoadState::Failed(error)) = server.get_load_state(handle.id()) {
            assert!(
                matches!(error.as_ref(), AssetLoadError::AssetReaderError(_)),
                "{error}"
            );
            assert!(
                error.to_string().contains("GLB hierarchy contains a cycle"),
                "{error}"
            );
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "native reader failure timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(
        app.world()
            .resource::<Assets<bevy::gltf::Gltf>>()
            .is_empty()
    );
    Ok(())
}

#[test]
fn expansion_limit_fails_before_processing_without_publication_or_source_changes()
-> Result<(), Box<dyn std::error::Error>> {
    use serde_json::json;
    // The complete topology/error matrix belongs to kedra_assets::model.
    // Here one bounded-expansion failure verifies importer wiring and publication.
    let model =
        json!({"asset":{"version":"2.0"},"nodes":[{}],"scenes":vec![json!({"nodes":[0]});2049]});
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    std::fs::create_dir(&source)?;
    Glb {
        header: Header {
            magic: *b"glTF",
            version: 2,
            length: 0,
        },
        json: Cow::Owned(serde_json::to_vec(&model)?),
        bin: None,
    }
    .to_writer(std::fs::File::create(source.join("bad.glb"))?)?;
    kedra_import::initialize(&source, |_| {})?;
    let before = std::fs::read(source.join("bad.glb"))?;
    let meta_before = std::fs::read(source.join("bad.glb.meta"))?;
    let output = temp.path().join("delivery");
    let mut processing_started = false;
    let error = kedra_import::build(&source, &output, "test", "game", |message| {
        processing_started |= message.starts_with("Processing ");
    })
    .err()
    .ok_or("invalid topology was published")?;
    assert!(error.contains("GLB"), "{error}");
    assert!(
        !processing_started,
        "native AssetProcessor started before admission"
    );
    assert!(!output.exists());
    assert_eq!(std::fs::read(source.join("bad.glb"))?, before);
    assert_eq!(std::fs::read(source.join("bad.glb.meta"))?, meta_before);
    Ok(())
}
#[test]
fn static_glb_metadata_and_headless_delivery_use_native_loader()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    std::fs::create_dir(&source)?;
    let json = serde_json::to_vec(
        &serde_json::json!({"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"name":"Static fixture"}]}),
    )?;
    let glb = Glb {
        header: Header {
            magic: *b"glTF",
            version: 2,
            length: 0,
        },
        json: Cow::Owned(json),
        bin: None,
    };
    glb.to_writer(std::fs::File::create(source.join("fixture.glb"))?)?;
    kedra_import::initialize(&source, |_| {})?;
    let meta: AssetMeta<GltfLoader, ()> =
        ron::de::from_bytes(&std::fs::read(source.join("fixture.glb.meta"))?)?;
    let AssetAction::Load { settings, .. } = meta.asset else {
        return Err("Model was not assigned a native loader".into());
    };
    assert!(!settings.load_cameras && !settings.load_lights && !settings.load_animations);
    let before = std::fs::read(source.join("fixture.glb"))?;
    let manifest = kedra_import::build(
        &source,
        &temp.path().join("delivery"),
        "editor-desktop",
        "game",
        |_| {},
    )?;
    assert_eq!(manifest.assets.len(), 1);
    assert_eq!(manifest.assets[0].loader, "bevy_gltf::loader::GltfLoader");
    assert_eq!(std::fs::read(source.join("fixture.glb"))?, before);
    Ok(())
}
