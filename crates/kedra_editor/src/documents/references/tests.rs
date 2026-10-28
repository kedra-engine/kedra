//! Source reference admission owns type/source/delivery guards and symbolic
//! publication. Native loader readiness is separately owned by the UI host.
use super::*;
use kedra_assets::delivery::{AssetRecord, FileDigest, Manifest};
use kedra_authoring::persistence::{SourceAsset, SourceFile, SourceSnapshot, content_hash};
use kedra_scene::document::{ComponentData, PrefabDocument};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[derive(Component, Reflect)]
#[reflect(Component)]
struct ImageEntries(Vec<kedra_ui::authored::components::UiImageDensity>);
#[test]
fn typed_reference_candidates_preserve_symbolic_paths_and_reject_foreign_or_stale_delivery_without_source_edits()
-> TestResult {
    let mut app = crate::workspace::browser_test_app()?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    kedra_scene::spatial::register(
        &mut registry.write(),
        &mut app.world_mut().resource_mut::<ComponentPolicies>(),
    )?;
    app.world_mut()
        .resource_mut::<ComponentPolicies>()
        .register::<ImageEntries>(&mut registry.write(), "image.entries", 1, |_| Ok(()))?;
    let types = registry.read();
    let policies = app.world().resource::<ComponentPolicies>();
    let mut doc = PrefabDocument::new("Reference source");
    doc.nodes[0]
        .components
        .insert("image.entries".into(), ComponentData::ron(1, "([])")?);
    doc.nodes[0].components.insert("ui.image".into(), ComponentData::ron(1,
        "(variants:[(density:1.0,image:Path(\"game://first.png\"))],color:Srgba((red:1.0,green:1.0,blue:1.0,alpha:1.0)))")?);
    doc.nodes[0].components.insert("scene.surface".into(), ComponentData::ron(1,
        "(color:Srgba((red:1.0,green:1.0,blue:1.0,alpha:1.0)),texture:None,metallic:0.0,roughness:0.5)")?);
    let text = doc.serialize()?;
    let files = ["first.png", "second.png", "style.css"].map(|path| SourceAsset {
        id: Some(uuid::Uuid::new_v4()),
        path: path.into(),
        hash: content_hash(""),
        bytes: 0,
    });
    let snapshot = SourceSnapshot {
        namespace: "game".into(),
        generation: 1,
        documents: vec![SourceFile {
            path: "image.prefab.ron".into(),
            source: text.clone(),
            hash: content_hash(&text),
        }],
        assets: files.to_vec(),
        index: None,
    };
    let (scope, compiled) = super::super::source::admit(snapshot, &types, policies)?;
    let mut documents = Documents {
        scope: Some(scope),
        compiled,
        ..default()
    };
    documents.compiled_revisions = documents.revisions();
    let target = documents.target(doc.id, doc.root.into()).ok_or("target")?;
    let root = std::path::PathBuf::from("reference-fixture");
    let mut delivery = Delivery {
        source: root.clone(),
        manifest: Manifest {
            format: 1,
            profile: "editor-desktop".into(),
            assets: vec![AssetRecord {
                id: doc.id.0,
                path: "image.prefab.ron".into(),
                loader: "native prefab".into(),
            }],
            files: BTreeMap::from([(
                "image.prefab.ron".into(),
                FileDigest {
                    bytes: text.len() as u64,
                    sha256: content_hash(&text),
                },
            )]),
        },
    };
    for asset in &files {
        delivery.manifest.assets.push(AssetRecord {
            id: asset.id.ok_or("id")?,
            path: asset.path.clone(),
            loader: if asset.path.ends_with(".png") {
                "bevy_image::image_loader::ImageLoader".into()
            } else {
                "bevy_flair_css_parser::loader::CssStyleSheetLoader".into()
            },
        });
        delivery.manifest.files.insert(
            asset.path.clone(),
            FileDigest {
                bytes: asset.bytes,
                sha256: asset.hash.clone(),
            },
        );
    }
    let baseline = documents.revisions();
    let field = ParsedPath::parse("variants[0].image")?;
    let variants = ParsedPath::parse("variants")?;
    let server = app.world().resource::<AssetServer>();
    let entries = ParsedPath::parse(".0")?;
    let first_image = documents
        .reference_value(
            ReferenceRequest {
                target: &target,
                expected: &baseline,
                component: "image.entries",
                field: &entries,
                path: Some("game://second.png"),
            },
            &types,
            policies,
            ResourceContext {
                root: &root,
                delivery: Some(&delivery),
                server,
            },
        )?
        .ok_or("first required image")?;
    let entries_value = first_image
        .try_downcast_ref::<Vec<kedra_ui::authored::components::UiImageDensity>>()
        .ok_or("native image entries")?;
    assert_eq!(entries_value[0].density, 1.);
    assert_eq!(
        entries_value[0]
            .image
            .path()
            .map(ToString::to_string)
            .as_deref(),
        Some("game://second.png")
    );
    let candidate = documents.prepare_property(
        &target,
        &baseline,
        "image.entries",
        &[kedra_authoring::properties::PropertyEdit {
            path: entries,
            value: first_image.as_ref(),
        }],
        &types,
        policies,
    )?;
    drop(candidate);
    assert_eq!(documents.revisions(), baseline);
    assert!(!documents.can_history(false));
    assert!(
        documents
            .reference_value(
                ReferenceRequest {
                    target: &target,
                    expected: &baseline,
                    component: "ui.image",
                    field: &variants,
                    path: Some("game://second.png")
                },
                &types,
                policies,
                ResourceContext {
                    root: &root,
                    delivery: Some(&delivery),
                    server
                }
            )
            .is_err(),
        "the empty-list constructor cannot replace or append to a populated image list"
    );
    let texture = ParsedPath::parse("texture")?;
    assert!(
        documents
            .reference_value(
                ReferenceRequest {
                    target: &target,
                    expected: &baseline,
                    component: "scene.surface",
                    field: &texture,
                    path: None
                },
                &types,
                policies,
                ResourceContext {
                    root: &root,
                    delivery: None,
                    server
                }
            )?
            .is_none()
    );
    assert_eq!(documents.revisions(), baseline);
    assert!(!documents.can_history(false));
    let request = |path| ReferenceRequest {
        target: &target,
        expected: &baseline,
        component: "ui.image",
        field: &field,
        path,
    };
    for path in [
        "editor://second.png",
        "game://style.css",
        "game://absent.png",
        "/absolute/second.png",
    ] {
        assert!(
            documents
                .reference_value(
                    request(Some(path)),
                    &types,
                    policies,
                    ResourceContext {
                        root: &root,
                        delivery: Some(&delivery),
                        server
                    }
                )
                .is_err(),
            "{path}"
        );
        assert_eq!(documents.revisions(), baseline);
        assert!(!documents.can_history(false));
    }
    assert!(
        documents
            .reference_value(
                request(None),
                &types,
                policies,
                ResourceContext {
                    root: &root,
                    delivery: Some(&delivery),
                    server
                }
            )
            .is_err()
    );
    let mut foreign = delivery.clone();
    foreign.source = "another-source".into();
    assert!(
        documents
            .reference_value(
                request(Some("game://second.png")),
                &types,
                policies,
                ResourceContext {
                    root: &root,
                    delivery: Some(&foreign),
                    server
                }
            )
            .is_err()
    );
    let mut stale = delivery.clone();
    stale
        .manifest
        .files
        .get_mut("second.png")
        .ok_or("file")?
        .bytes = 1;
    assert!(
        documents
            .reference_value(
                request(Some("game://second.png")),
                &types,
                policies,
                ResourceContext {
                    root: &root,
                    delivery: Some(&stale),
                    server
                }
            )
            .is_err()
    );
    assert_eq!(documents.revisions(), baseline);
    assert!(
        documents
            .reference_value(
                request(Some("game://first.png")),
                &types,
                policies,
                ResourceContext {
                    root: &root,
                    delivery: Some(&delivery),
                    server
                }
            )?
            .is_none()
    );
    assert_eq!(documents.revisions(), baseline);
    assert!(!documents.can_history(false));
    assert!(
        server.get_handle::<Image>("game://first.png").is_none(),
        "a no-op must not request native loading"
    );
    // The generic native construction preserves an exact logical path even though
    // the original source compilation has never requested this second image.
    let value = documents
        .reference_value(
            request(Some("game://second.png")),
            &types,
            policies,
            ResourceContext {
                root: &root,
                delivery: Some(&delivery),
                server,
            },
        )?
        .ok_or("new resource proposal")?;
    let change = documents.prepare_property(
        &target,
        &baseline,
        "ui.image",
        &[kedra_authoring::properties::PropertyEdit {
            path: field.clone(),
            value: value.as_ref(),
        }],
        &types,
        policies,
    )?;
    assert_eq!(
        documents.revisions(),
        baseline,
        "preparation is not publication"
    );
    documents.publish_change(change);
    let source = &documents.scope.as_ref().ok_or("scope")?.working.documents()[&doc.id].document;
    let encoded = source.serialize()?;
    assert!(encoded.contains("game://second.png"));
    let inspection = documents.inspect(&target)?;
    let effective = inspection
        .value("ui.image", &types, policies)?
        .try_downcast_ref::<kedra_ui::authored::components::UiImage>()
        .ok_or("image")?;
    assert_eq!(
        inspection
            .dependency_path(&effective.variants[0].image.clone().untyped())
            .as_deref(),
        Some("game://second.png")
    );
    assert!(
        documents
            .reference_value(
                request(Some("game://first.png")),
                &types,
                policies,
                ResourceContext {
                    root: &root,
                    delivery: Some(&delivery),
                    server
                }
            )
            .is_err()
    );
    let undo = documents.prepare_history(false, &types, policies)?;
    documents.publish_change(undo);
    assert_eq!(
        documents.scope.as_ref().ok_or("scope")?.working.documents()[&doc.id].document,
        doc
    );
    assert!(!documents.can_history(false));
    let restored = documents.revisions();
    assert!(documents.can_history(true));
    assert!(
        documents
            .reference_value(
                ReferenceRequest {
                    target: &target,
                    expected: &restored,
                    component: "ui.image",
                    field: &field,
                    path: Some("game://first.png")
                },
                &types,
                policies,
                ResourceContext {
                    root: &root,
                    delivery: Some(&delivery),
                    server
                }
            )?
            .is_none()
    );
    assert_eq!(documents.revisions(), restored);
    assert!(
        documents.can_history(true),
        "same-path assignment must preserve Redo"
    );
    assert_eq!(
        documents.scope.as_ref().ok_or("scope")?.source.documents[0].source,
        text,
        "source admission/Save baseline is unchanged"
    );
    Ok(())
}
