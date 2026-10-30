use bevy::{asset::AssetPath, prelude::*, world_serialization::WorldAsset};
use kedra_assets::delivery::{AssetRecord, FileDigest, Manifest, digest};
use kedra_authoring::persistence::{SourceAsset, SourceFile, SourceSnapshot};
use kedra_authoring_assets::Delivery;
use kedra_scene::document::PrefabDocument;
use std::{any::TypeId, collections::BTreeMap};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn authored_environment_revisions_keep_identity_and_require_exact_valid_source_bytes() -> TestResult
{
    let (mut source, mut delivery) = source();
    let path = "day.environment.ron";
    let old = "(version:1,background:Srgba((red:0.0,green:0.0,blue:0.0,alpha:1.0)),ambient_color:Srgba((red:1.0,green:1.0,blue:1.0,alpha:1.0)),ambient_brightness:20.0)";
    let new = old.replace("20.0", "450.0");
    let id = uuid::Uuid::new_v4();
    source.assets.push(SourceAsset {
        id: Some(id),
        path: path.into(),
        hash: digest(new.as_bytes()),
        bytes: new.len() as u64,
    });
    source.environments.insert(path.into(), new.clone());
    delivery.manifest.assets.push(AssetRecord {
        id,
        path: path.into(),
        loader: "kedra_scene::environment::EnvironmentLoader".into(),
    });
    delivery.manifest.files.insert(
        path.into(),
        FileDigest {
            bytes: old.len() as u64,
            sha256: digest(old.as_bytes()),
        },
    );
    let logical = AssetPath::parse("game://day.environment.ron");
    let ty = TypeId::of::<kedra_scene::environment::Environment>();
    assert!(delivery.accepts(&source, ty, &logical));
    assert!(
        delivery.matches(&delivery.source, &source).is_err(),
        "Runtime delivery still requires published bytes"
    );
    delivery.matches_resources(&delivery.source, &source)?;
    assert_eq!(
        kedra_authoring_assets::environment_source(&source, path)?.ambient_brightness,
        450.0
    );
    source.environments.insert(path.into(), old.into());
    assert!(!delivery.accepts(&source, ty, &logical));
    assert!(
        delivery
            .matches_resources(&delivery.source, &source)
            .is_err()
    );
    source.environments.insert(path.into(), new);
    source.assets[0].id = Some(uuid::Uuid::new_v4());
    assert!(!delivery.accepts(&source, ty, &logical));
    assert!(
        delivery
            .matches_resources(&delivery.source, &source)
            .is_err()
    );
    Ok(())
}

fn source() -> (SourceSnapshot, Delivery) {
    let document = PrefabDocument::new("Source");
    let text = document.serialize().unwrap_or_else(|e| panic!("{e}"));
    let snapshot = SourceSnapshot {
        environments: Default::default(),
        namespace: "game".into(),
        generation: 1,
        index: None,
        documents: vec![SourceFile {
            path: "one.scene.ron".into(),
            hash: digest(text.as_bytes()),
            source: text.clone(),
        }],
        assets: vec![],
    };
    let manifest = Manifest {
        format: 1,
        profile: "editor-desktop".into(),
        assets: vec![AssetRecord {
            id: document.id.0,
            path: "one.scene.ron".into(),
            loader: "kedra_scene::assets::DocumentLoader".into(),
        }],
        files: BTreeMap::from([(
            "one.scene.ron".into(),
            FileDigest {
                bytes: text.len() as u64,
                sha256: digest(text.as_bytes()),
            },
        )]),
    };
    (
        snapshot,
        Delivery {
            source: "project/assets".into(),
            manifest,
        },
    )
}

#[test]
fn dependency_admission_checks_native_loader_and_exact_inventory_before_requesting_assets()
-> TestResult {
    let (mut source, mut delivery) = source();
    for (path, loader, ty) in [
        (
            "picture.png",
            "bevy_image::image_loader::ImageLoader",
            TypeId::of::<Image>(),
        ),
        (
            "lighting.environment.ron",
            "kedra_scene::environment::EnvironmentLoader",
            TypeId::of::<kedra_scene::environment::Environment>(),
        ),
        (
            "model.glb",
            "bevy_gltf::loader::GltfLoader",
            TypeId::of::<WorldAsset>(),
        ),
        (
            "style.css",
            "bevy_flair_css_parser::loader::CssStyleSheetLoader",
            TypeId::of::<bevy_flair::style::StyleSheet>(),
        ),
    ] {
        let id = uuid::Uuid::new_v4();
        let hash = digest(path.as_bytes());
        source.assets.push(SourceAsset {
            id: Some(id),
            path: path.into(),
            hash: hash.clone(),
            bytes: path.len() as u64,
        });
        delivery.manifest.assets.push(AssetRecord {
            id,
            path: path.into(),
            loader: loader.into(),
        });
        delivery.manifest.files.insert(
            path.into(),
            FileDigest {
                bytes: path.len() as u64,
                sha256: hash,
            },
        );
        let logical = AssetPath::parse(&format!("game://{path}")).into_owned();
        assert!(delivery.accepts(&source, ty, &logical));
        let symbolic = AssetPath::parse(&format!(
            "game://{id}/asset.{}",
            path.rsplit('.').next().ok_or("extension")?
        ))
        .into_owned();
        assert!(delivery.accepts(&source, ty, &symbolic));
        assert!(!delivery.accepts(&source, TypeId::of::<Mesh>(), &logical));
        let other_type = if ty == TypeId::of::<Image>() {
            TypeId::of::<WorldAsset>()
        } else {
            TypeId::of::<Image>()
        };
        assert!(!delivery.accepts(&source, other_type, &logical));
        source.assets.last_mut().ok_or("asset")?.bytes += 1;
        assert!(!delivery.accepts(&source, ty, &logical));
        source.assets.last_mut().ok_or("asset")?.bytes -= 1;
        source.assets.last_mut().ok_or("asset")?.id = Some(uuid::Uuid::new_v4());
        assert!(!delivery.accepts(&source, ty, &logical));
        assert!(!delivery.accepts(&source, ty, &symbolic));
        source.assets.last_mut().ok_or("asset")?.id = Some(id);
        assert!(!delivery.accepts(&source, ty, &AssetPath::parse(&format!("editor://{path}"))));
    }
    delivery.matches(&delivery.source, &source)?;
    Ok(())
}

#[test]
fn delivery_binding_rejects_foreign_missing_and_stale_sources_without_mutation() -> TestResult {
    let (mut source, mut delivery) = source();
    let before = source.documents[0].source.clone();
    delivery.matches(&delivery.source, &source)?;
    assert!(
        delivery
            .matches(std::path::Path::new("other/assets"), &source)
            .is_err()
    );
    source.documents[0].hash = digest(b"changed");
    assert!(delivery.matches(&delivery.source, &source).is_err());
    source.documents[0].hash = digest(before.as_bytes());
    delivery.manifest.assets.clear();
    assert!(delivery.matches(&delivery.source, &source).is_err());
    assert_eq!(source.documents[0].source, before);
    Ok(())
}

#[test]
fn resource_previews_survive_document_changes_but_reject_stale_dependencies_and_foreign_scope()
-> TestResult {
    let (mut source, mut delivery) = source();
    let id = uuid::Uuid::new_v4();
    let asset = SourceAsset {
        id: Some(id),
        path: "texture.png".into(),
        hash: digest(b"texture"),
        bytes: 7,
    };
    source.assets.push(asset.clone());
    delivery.manifest.assets.push(AssetRecord {
        id,
        path: asset.path.clone(),
        loader: "bevy_image::image_loader::ImageLoader".into(),
    });
    delivery.manifest.files.insert(
        asset.path.clone(),
        FileDigest {
            bytes: asset.bytes,
            sha256: asset.hash.clone(),
        },
    );
    source.documents[0].hash = digest(b"saved edits");
    assert!(delivery.matches(&delivery.source, &source).is_err());
    delivery.matches_resources(&delivery.source, &source)?;
    let document = PrefabDocument::new("New scene").serialize()?;
    source.documents.push(SourceFile {
        path: "new.scene.ron".into(),
        hash: digest(document.as_bytes()),
        source: document.clone(),
    });
    delivery.matches_resources(&delivery.source, &source)?;
    assert_eq!(source.documents[1].source, document);
    source.documents.clear();
    delivery.matches_resources(&delivery.source, &source)?;
    assert!(delivery.matches(&delivery.source, &source).is_err());
    assert!(
        delivery
            .matches_resources(std::path::Path::new("foreign/assets"), &source)
            .is_err()
    );
    source.namespace = "foreign".into();
    assert!(
        delivery
            .matches_resources(&delivery.source, &source)
            .is_err()
    );
    source.namespace = "game".into();
    source.assets[0].hash = digest(b"changed texture");
    assert!(
        delivery
            .matches_resources(&delivery.source, &source)
            .is_err()
    );
    assert_eq!(source.assets[0].hash, digest(b"changed texture"));
    source.assets[0] = asset.clone();
    source.assets[0].bytes += 1;
    assert!(
        delivery
            .matches_resources(&delivery.source, &source)
            .is_err()
    );
    source.assets[0] = asset.clone();
    source.assets[0].id = Some(uuid::Uuid::new_v4());
    assert!(
        delivery
            .matches_resources(&delivery.source, &source)
            .is_err()
    );
    source.assets.clear();
    assert!(
        delivery
            .matches_resources(&delivery.source, &source)
            .is_err()
    );
    assert!(source.assets.is_empty());
    assert_eq!(delivery.manifest.files[&asset.path].sha256, asset.hash);
    Ok(())
}
