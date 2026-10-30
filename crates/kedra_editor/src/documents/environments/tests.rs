use super::*;
#[test]
fn scene_profile_assignment_is_atomic_scoped_and_revision_pinned()
-> Result<(), Box<dyn std::error::Error>> {
    use kedra_authoring::persistence::{SourceAsset, SourceFile, SourceSnapshot, content_hash};
    use kedra_scene::document::{DocumentKind, PrefabDocument};
    let mut types = bevy::reflect::TypeRegistry::default();
    let mut policies = ComponentPolicies::default();
    kedra_scene::spatial::register(&mut types, &mut policies)?;
    let mut document = PrefabDocument::new("Lighting scene");
    document.kind = DocumentKind::Scene;
    let text = document.serialize()?;
    let profile = "(version:1,background:Srgba((red:0.0,green:0.0,blue:0.0,alpha:1.0)),ambient_color:Srgba((red:1.0,green:1.0,blue:1.0,alpha:1.0)),ambient_brightness:100.0)";
    let (scope, compiled) = source::admit(
        SourceSnapshot {
            namespace: "game".into(),
            generation: 1,
            index: None,
            documents: vec![SourceFile {
                path: "Lighting.scene.ron".into(),
                source: text.clone(),
                hash: content_hash(&text),
            }],
            assets: vec![SourceAsset {
                path: "day.environment.ron".into(),
                id: Some(uuid::Uuid::new_v4()),
                hash: content_hash(profile),
                bytes: profile.len() as u64,
            }],
            environments: [("day.environment.ron".into(), profile.into())].into(),
        },
        &types,
        &policies,
    )?;
    let mut documents = Documents {
        compiled_revisions: scope
            .working
            .documents()
            .iter()
            .map(|(id, d)| (*id, d.revision))
            .collect(),
        scope: Some(scope),
        compiled,
        ..default()
    };
    let target = documents
        .target(document.id, document.root.into())
        .ok_or("Scene target missing")?;
    let expected = documents.revisions();
    assert!(
        documents
            .assign_environment(
                &target,
                &expected,
                "editor://day.environment.ron",
                &types,
                &policies
            )
            .is_err()
    );
    assert_eq!(documents.revisions(), expected);
    assert!(
        !documents
            .inspect(&target)?
            .components
            .contains_key("scene.environment")
    );
    documents.assign_environment(
        &target,
        &expected,
        "game://day.environment.ron",
        &types,
        &policies,
    )?;
    assert!(
        documents
            .inspect(&target)?
            .components
            .contains_key("scene.environment")
    );
    let accepted = documents.revisions();
    assert!(documents.select_asset_path("game://Lighting.scene.ron"));
    assert!(documents.active.is_none());
    assert!(
        documents.can_save(),
        "A selected scene asset owns Save without an active tab"
    );
    assert!(
        documents
            .assign_environment(
                &target,
                &expected,
                "game://day.environment.ron",
                &types,
                &policies
            )
            .is_err()
    );
    assert_eq!(documents.revisions(), accepted);
    assert_eq!(
        documents
            .scope
            .as_ref()
            .ok_or("Scope missing")?
            .source
            .documents[0]
            .source,
        text
    );
    let change = documents.prepare_history(false, &types, &policies)?;
    documents.publish_change(change);
    assert!(
        !documents
            .inspect(&target)?
            .components
            .contains_key("scene.environment")
    );
    Ok(())
}
