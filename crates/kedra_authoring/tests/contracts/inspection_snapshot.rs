//! Source admission can retain its exact validated compilation for read-only tools.
use bevy::reflect::TypeRegistry;
use kedra_authoring::persistence::{SourceFile, SourceSnapshot, Workspace, content_hash};
use kedra_scene::{document::PrefabDocument, reflection::ComponentPolicies};
use std::collections::BTreeMap;

#[test]
fn successful_admission_returns_matching_compilation_and_failure_returns_none()
-> Result<(), Box<dyn std::error::Error>> {
    let document = PrefabDocument::new("Source-owned inspection");
    let source = ron::to_string(&document)?;
    let snapshot = SourceSnapshot {
        environments: Default::default(),
        namespace: "game".into(),
        generation: 1,
        documents: vec![SourceFile {
            path: "source.prefab.ron".into(),
            hash: content_hash(&source),
            source,
        }],
        assets: vec![],
        index: None,
    };
    let mut types = TypeRegistry::default();
    let policies = ComponentPolicies::default();
    types.register::<bevy::prelude::Name>();
    types.register::<bevy::prelude::ChildOf>();
    types.register::<bevy::prelude::Children>();
    struct NoAssets;
    impl bevy::asset::LoadFromPath for NoAssets {
        fn load_from_path_erased(
            &mut self,
            _: std::any::TypeId,
            _: bevy::asset::AssetPath<'static>,
        ) -> bevy::asset::UntypedHandle {
            panic!("No assets in this source")
        }
    }
    let (workspace, compiled) =
        Workspace::open_compiled(snapshot.clone(), &types, &policies, &mut NoAssets);
    assert!(workspace.diagnostic.is_none());
    assert_eq!(compiled.len(), 1);
    assert_eq!(compiled[&document.id].graph().root, document.root.into());
    assert_eq!(
        workspace.working.documents()[&document.id].document,
        document
    );
    let mut broken = snapshot;
    broken.documents[0].source = "invalid".into();
    let (failed, compiled) =
        Workspace::open_compiled(broken.clone(), &types, &policies, &mut NoAssets);
    assert!(failed.diagnostic.is_some());
    assert!(compiled.is_empty());
    assert!(failed.working.documents().is_empty());
    assert_eq!(failed.source.documents[0].source, "invalid");
    let _: BTreeMap<_, _> = compiled;
    Ok(())
}
