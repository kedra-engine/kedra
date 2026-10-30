//! Imported inventories must not invalidate accepted source edits or their history.
use super::*;

#[test]
fn import_refresh_preserves_source_history_and_refuses_external_document_changes()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    let document = kedra_scene::document::PrefabDocument::new("Original");
    let id = document.id;
    let root: NodeAddress = document.root.into();
    std::fs::write(
        directory.path().join("Arena.prefab.ron"),
        document.serialize()?,
    )?;
    let types = bevy::reflect::TypeRegistry::default();
    let policies = ComponentPolicies::default();
    let (scope, compiled) = source::admit(
        source_files::read(directory.path(), "game")?,
        &types,
        &policies,
    )?;
    let mut documents = Documents {
        scope: Some(scope),
        compiled,
        ..default()
    };
    let mut edit = documents.begin_edit("Rename root", &["game://Arena.prefab.ron".into()])?;
    let node = edit
        .nodes(
            documents.scope.as_ref().ok_or("scope")?,
            "game://Arena.prefab.ron",
        )?
        .into_iter()
        .find(|n| n.address == root)
        .ok_or("root")?;
    edit.rename(
        documents.scope.as_ref().ok_or("scope")?,
        &node,
        "Accepted",
        &types,
        &policies,
    )?;
    documents.commit_edit(&mut edit, &types, &policies)?;
    let history = documents.history_id(false);
    let asset = outside.path().join("license.txt");
    std::fs::write(&asset, "license")?;
    kedra_import::ingest::import_files(
        directory.path(),
        std::path::Path::new(""),
        &[asset],
        |_| {},
    )?;
    documents.imported(
        source_files::read(directory.path(), "game")?,
        &types,
        &policies,
    )?;
    assert_eq!(documents.history_id(false), history);
    assert_eq!(
        documents.compiled[&id].graph().nodes[&root].name,
        "Accepted"
    );
    assert!(
        documents
            .scope
            .as_ref()
            .ok_or("scope")?
            .source
            .assets
            .iter()
            .any(|a| a.path == "license.txt" && a.id.is_some())
    );
    let revision = documents.revisions();
    let mut changed = document.clone();
    changed.nodes[0].name = "External".into();
    std::fs::write(
        directory.path().join("Arena.prefab.ron"),
        changed.serialize()?,
    )?;
    assert!(
        documents
            .imported(
                source_files::read(directory.path(), "game")?,
                &types,
                &policies
            )
            .is_err()
    );
    assert_eq!(documents.revisions(), revision);
    assert_eq!(documents.history_id(false), history);
    let change = documents.prepare_history(false, &types, &policies)?;
    documents.publish_change(change);
    assert_eq!(
        documents.compiled[&id].graph().nodes[&root].name,
        "Original"
    );
    Ok(())
}
