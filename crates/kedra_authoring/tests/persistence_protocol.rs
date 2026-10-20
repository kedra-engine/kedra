//! Pending saves and rejected receipts leave exact retry bytes and working history intact.
mod common;
use bevy::prelude::*;
use common::{Result, baseline, document, setup};
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
