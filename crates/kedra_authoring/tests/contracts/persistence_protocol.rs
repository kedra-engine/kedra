//! Pending saves and rejected receipts leave exact retry bytes and working history intact.
use crate::support::{Result, baseline, document, setup};
use bevy::prelude::*;
use kedra_authoring::persistence::*;
use std::collections::BTreeMap;

fn receipt(request: &CommitRequest) -> Receipt {
    Receipt {
        operation: request.operation,
        generation: request.expected_generation + 1,
        files: request
            .files
            .iter()
            .map(|file| SavedFile {
                document: file.document,
                path: file.path.clone(),
                revision: file.revision,
                hash: file.source.as_deref().map(content_hash),
            })
            .collect(),
    }
}

#[test]
fn malformed_receipts_and_pending_reconciliation_preserve_exact_operation() -> Result<()> {
    let (app, policies) = setup()?;
    let original = document()?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let source = baseline(std::slice::from_ref(&original))?;
    let mut workspace = Workspace::open(source.clone(), &types.read(), &policies, &mut loader);
    workspace.working.transact_checked(
        &BTreeMap::from([(original.id, 1)]),
        |documents| {
            documents
                .get_mut(&original.id)
                .ok_or_else(|| kedra_scene::Error("document missing".into()))?
                .title = "Submitted".into();
            Ok(())
        },
        |_| Ok(()),
    )?;
    let request = workspace
        .prepare_save(None, &types.read(), &policies, &mut loader)?
        .clone();
    let good = receipt(&request);
    for case in 0..7 {
        let mut bad = good.clone();
        match case {
            0 => bad.operation = uuid::Uuid::new_v4(),
            1 => bad.generation += 1,
            2 => {
                bad.files.pop();
            }
            3 => bad.files[0].path = "unexpected.prefab.ron".into(),
            4 => bad.files[0].revision += 1,
            5 => bad.files[0].document = None,
            _ => bad.files[1] = bad.files[0].clone(),
        }
        let before = ron::to_string(&workspace)?;
        assert!(
            workspace.acknowledge(&bad).is_err(),
            "receipt case {case} admitted"
        );
        assert_eq!(ron::to_string(&workspace)?, before);
    }
    let before = ron::to_string(&workspace)?;
    assert!(
        workspace
            .keep_working(source, &types.read(), &policies, &mut loader)
            .is_err()
    );
    assert!(workspace.refused(uuid::Uuid::new_v4()).is_err());
    assert_eq!(ron::to_string(&workspace)?, before);
    let mut restored: Workspace = ron::from_str(&before)?;
    assert_eq!(
        ron::to_string(&restored.pending())?,
        ron::to_string(&workspace.pending())?
    );
    restored.acknowledge(&good)?;
    assert!(!restored.working.dirty(original.id));
    assert!(restored.pending().is_none());
    workspace.refused(request.operation)?;
    assert!(workspace.pending().is_none());
    assert!(workspace.working.dirty(original.id));
    assert!(workspace.refused(request.operation).is_err());
    let next = workspace.prepare_save(None, &types.read(), &policies, &mut loader)?;
    assert_ne!(next.operation, request.operation);
    assert_eq!(
        ron::to_string(&next.files)?,
        ron::to_string(&request.files)?
    );
    Ok(())
}

#[test]
fn existing_document_save_preserves_catalog_and_never_acknowledges_newer_edits() -> Result<()> {
    let (app, policies) = setup()?;
    let original = document()?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let mut workspace = Workspace::open(
        baseline(std::slice::from_ref(&original))?,
        &types.read(),
        &policies,
        &mut loader,
    );
    workspace.working.transact_checked(
        &BTreeMap::from([(original.id, 1)]),
        |docs| {
            docs.get_mut(&original.id)
                .ok_or_else(|| kedra_scene::Error("document missing".into()))?
                .title = "Pinned".into();
            Ok(())
        },
        |_| Ok(()),
    )?;
    let request = workspace
        .prepare_existing_save(original.id, &types.read(), &policies, &mut loader)?
        .clone();
    assert!(
        request
            .files
            .iter()
            .all(|file| file.document == Some(original.id) && file.expected_hash.is_some())
    );
    assert!(
        request
            .files
            .iter()
            .all(|file| file.path != "catalog.prefabs.ron")
    );
    let revisions = BTreeMap::from([(
        original.id,
        workspace.working.documents()[&original.id].revision,
    )]);
    workspace.working.transact_checked(
        &revisions,
        |docs| {
            docs.get_mut(&original.id)
                .ok_or_else(|| kedra_scene::Error("document missing".into()))?
                .title = "Newer".into();
            Ok(())
        },
        |_| Ok(()),
    )?;
    workspace.acknowledge(&receipt(&request))?;
    assert!(workspace.source.index.is_none());
    assert_eq!(
        kedra_scene::document::PrefabDocument::parse(&workspace.source.documents[0].source)?.title,
        "Pinned"
    );
    assert_eq!(
        workspace.working.documents()[&original.id].document.title,
        "Newer"
    );
    assert!(workspace.working.dirty(original.id));
    assert!(workspace.working.can_undo());
    workspace.working.undo_checked(|_| Ok(()))?;
    assert_eq!(
        workspace.working.documents()[&original.id].document.title,
        "Pinned"
    );
    assert!(
        !workspace.working.dirty(original.id),
        "Acknowledgment must pin submitted content even when newer edits exist"
    );
    workspace.working.redo_checked(|_| Ok(()))?;
    assert!(workspace.working.dirty(original.id));
    Ok(())
}

#[test]
fn existing_document_save_pins_complete_coupled_groups_and_refuses_partial_receipts() -> Result<()>
{
    let (app, policies) = setup()?;
    let a = document()?;
    let mut b = a.clone();
    b.id = kedra_scene::document::DocumentId::new();
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let mut workspace = Workspace::open(
        baseline(&[a.clone(), b.clone()])?,
        &types.read(),
        &policies,
        &mut loader,
    );
    workspace.working.transact_checked(
        &BTreeMap::from([(a.id, 1), (b.id, 1)]),
        |docs| {
            for doc in docs.values_mut() {
                doc.title = "Coupled".into();
            }
            Ok(())
        },
        |_| Ok(()),
    )?;
    let index = workspace.source.index.clone();
    let request = workspace
        .prepare_existing_save(a.id, &types.read(), &policies, &mut loader)?
        .clone();
    assert_eq!(request.files.len(), 2);
    assert!(
        request
            .files
            .iter()
            .all(|file| file.document.is_some() && file.expected_hash.is_some())
    );
    let mut partial = receipt(&request);
    partial.files.pop();
    let before = ron::to_string(&workspace)?;
    assert!(workspace.acknowledge(&partial).is_err());
    assert_eq!(ron::to_string(&workspace)?, before);
    workspace.acknowledge(&receipt(&request))?;
    assert_eq!(
        ron::to_string(&workspace.source.index)?,
        ron::to_string(&index)?
    );
    assert!(!workspace.working.dirty(a.id) && !workspace.working.dirty(b.id));
    assert!(workspace.working.can_undo());
    Ok(())
}
