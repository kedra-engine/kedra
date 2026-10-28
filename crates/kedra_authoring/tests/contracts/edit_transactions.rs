//! Scoped edit leases share the same source journal as all native transactions.
use super::support::*;
use kedra_authoring::{transaction::EditTransaction, workspace::WorkingSet};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn scoped_batch_is_isolated_until_one_named_commit_and_history_never_reexecutes_it() -> Result<()> {
    let first = document()?;
    let mut second = document()?;
    second.title = "Second".into();
    let mut working = WorkingSet::default();
    working.open(first.clone())?;
    working.open(second.clone())?;
    let before = working.catalog();
    let mut edit = EditTransaction::begin(
        &working,
        "Arrange prefabs",
        BTreeSet::from([first.id, second.id]),
    )?;
    edit.edit(first.id, |document| {
        document.nodes[0].name = "入口".into();
        Ok(())
    })?;
    edit.edit(second.id, |document| {
        document.nodes[0].name = "Exit".into();
        Ok(())
    })?;
    assert_eq!(working.catalog().documents, before.documents);
    assert!(!working.can_undo());
    assert!(
        edit.commit(&mut working, |_| Err(kedra_scene::Error(
            "host refuses".into()
        )))
        .is_err()
    );
    assert_eq!(working.catalog().documents, before.documents);
    assert!(!edit.is_closed() && !working.can_undo());
    edit.commit(&mut working, |_| Ok(()))?;
    let after = working.catalog();
    let entry = working.undo_entry().ok_or("history")?;
    let id = entry.id;
    assert_eq!(entry.label, "Arrange prefabs");
    assert_eq!(entry.documents, BTreeSet::from([first.id, second.id]));
    assert!(edit.commit(&mut working, |_| Ok(())).is_err());
    working.undo_checked(|_| Ok(()))?;
    assert_eq!(working.catalog().documents, before.documents);
    assert!(!working.can_undo());
    assert_eq!(working.redo_entry().ok_or("redo")?.id, id);
    working.redo_checked(|_| Ok(()))?;
    assert_eq!(working.catalog().documents, after.documents);
    assert_eq!(working.undo_entry().ok_or("undo")?.id, id);
    Ok(())
}

#[test]
fn ungranted_foreign_stale_cancelled_and_noop_edits_preserve_working_state_and_redo() -> Result<()>
{
    let first = document()?;
    let second = document()?;
    let mut working = WorkingSet::default();
    working.open(first.clone())?;
    working.open(second.clone())?;
    let grants = BTreeSet::from([first.id]);
    let mut edit = EditTransaction::begin(&working, "Batch", grants.clone())?;
    assert!(
        edit.edit(second.id, |document| {
            document.title = "Foreign".into();
            Ok(())
        })
        .is_err()
    );
    let mut candidate = edit.candidate()?.clone();
    candidate
        .documents
        .get_mut(&second.id)
        .ok_or("second")?
        .title = "Foreign".into();
    assert!(edit.stage(candidate).is_err());
    let foreign: WorkingSet = ron::from_str(&ron::to_string(&working)?)?;
    assert!(
        edit.check(&foreign).is_err(),
        "serialized copies cannot borrow the edit lineage"
    );
    let expected: BTreeMap<_, _> = working
        .documents()
        .iter()
        .map(|(id, value)| (*id, value.revision))
        .collect();
    working.transact_named(
        "Native edit",
        &expected,
        |documents| {
            documents
                .get_mut(&first.id)
                .ok_or_else(|| kedra_scene::Error("first".into()))?
                .title = "Native".into();
            Ok(())
        },
        |_| Ok(()),
    )?;
    assert!(edit.commit(&mut working, |_| Ok(())).is_err());
    working.undo_checked(|_| Ok(()))?;
    let baseline = ron::to_string(&working)?;
    let mut noop = EditTransaction::begin(&working, "No-op", grants.clone())?;
    noop.commit(&mut working, |_| Ok(()))?;
    assert_eq!(ron::to_string(&working)?, baseline);
    let mut cancelled = EditTransaction::begin(&working, "Cancel", grants)?;
    cancelled.edit(first.id, |document| {
        document.title = "Unpublished".into();
        Ok(())
    })?;
    cancelled.cancel();
    assert!(cancelled.commit(&mut working, |_| Ok(())).is_err());
    drop(cancelled);
    assert_eq!(ron::to_string(&working)?, baseline);
    assert!(working.can_redo());
    Ok(())
}

#[test]
fn named_gesture_retains_first_identity_and_rejected_candidate_keeps_its_prior_values() -> Result<()>
{
    let doc = document()?;
    let mut working = WorkingSet::default();
    working.open(doc.clone())?;
    let mut group = working.edit_group();
    let mut identity = None;
    for title in ["First", "Second"] {
        let expected = BTreeMap::from([(doc.id, working.documents()[&doc.id].revision)]);
        working.transact_grouped_named(
            &mut group,
            title,
            &expected,
            |documents| {
                documents
                    .get_mut(&doc.id)
                    .ok_or_else(|| kedra_scene::Error("document".into()))?
                    .title = title.into();
                Ok(())
            },
            |_| Ok(()),
        )?;
        let entry = working.undo_entry().ok_or("history")?;
        assert_eq!(entry.label, "First");
        assert_eq!(*identity.get_or_insert(entry.id), entry.id);
    }
    let mut edit = EditTransaction::begin(&working, "Candidate", BTreeSet::from([doc.id]))?;
    let before = edit.candidate()?.documents.clone();
    assert!(
        edit.rename(doc.id, doc.root.into(), "Rejected".into(), |_| Err(
            kedra_scene::Error("reject".into())
        ))
        .is_err()
    );
    assert_eq!(edit.candidate()?.documents, before);
    assert!(
        edit.edit(doc.id, |value| {
            value.id = kedra_scene::document::DocumentId::new();
            Ok(())
        })
        .is_err()
    );
    assert_eq!(edit.candidate()?.documents, before);
    let mut inventory = edit.candidate()?.clone();
    inventory.documents.clear();
    assert!(edit.stage(inventory).is_err());
    assert_eq!(edit.candidate()?.documents, before);
    assert!(EditTransaction::begin(&working, "\n", BTreeSet::from([doc.id])).is_err());
    assert!(EditTransaction::begin(&working, "Empty", BTreeSet::new()).is_err());
    working.undo_checked(|_| Ok(()))?;
    assert_eq!(working.catalog().documents[&doc.id], doc);
    assert!(!working.can_undo());
    Ok(())
}
