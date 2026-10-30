//! Source-only compilation retains exact logical paths without runtime loading.
use super::*;
#[test]
fn source_handles_validate_native_address_forms_without_runtime_loading() {
    let id = Uuid::new_v4();
    let snapshot = SourceSnapshot {
        environments: Default::default(),
        namespace: "game".into(),
        generation: 1,
        documents: Vec::new(),
        assets: vec![crate::persistence::SourceAsset {
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
        let mut loader = SourceHandles::new(&snapshot);
        loader.load_from_path_erased(TypeId::of::<bevy::image::Image>(), AssetPath::from(valid));
        assert!(loader.error.is_none());
    }
    for invalid in [
        format!("game://{id}/not-an-asset"),
        "editor://icons/a.svg".into(),
        "game://missing.svg".into(),
    ] {
        let mut loader = SourceHandles::new(&snapshot);
        loader.load_from_path_erased(TypeId::of::<bevy::image::Image>(), AssetPath::from(invalid));
        assert!(loader.error.is_some());
    }
    let mut loader = SourceHandles::new(&snapshot);
    let mut load = |label: &str| {
        loader.load_from_path_erased(
            TypeId::of::<bevy::image::Image>(),
            AssetPath::from(format!("game://icons/a.svg#{label}")),
        )
    };
    let one = load("1x");
    assert_ne!(one.id(), load("2x").id());
    assert_eq!(load("1x").id(), one.id());
    assert!(loader.error.is_none());
}
