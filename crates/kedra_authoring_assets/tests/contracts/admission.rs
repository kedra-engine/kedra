use bevy::{asset::AssetPath, prelude::*, world_serialization::WorldAsset};
use kedra_assets::delivery::{AssetRecord, FileDigest, Manifest, digest};
use kedra_authoring::persistence::{SourceAsset, SourceFile, SourceSnapshot};
use kedra_authoring_assets::Delivery;
use kedra_scene::document::PrefabDocument;
use std::{any::TypeId, collections::BTreeMap};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn source() -> (SourceSnapshot, Delivery) {
    let document = PrefabDocument::new("Source");
    let text = document.serialize().unwrap_or_else(|e| panic!("{e}"));
    let snapshot = SourceSnapshot {
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
