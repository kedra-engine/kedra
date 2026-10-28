//! Source history owns bounded retention and saved-content semantics, without UI.
use super::support::*;
use kedra_authoring::{
    transaction::EditTransaction,
    workspace::{HistoryPolicy, WorkingSet},
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn returning_to_saved_content_is_clean_without_rewinding_revisions() -> Result<()> {
    let doc = document()?;
    let mut working = WorkingSet::default();
    working.open(doc.clone())?;
    let mut edit = EditTransaction::begin(&working, "Change", BTreeSet::from([doc.id]))?;
    edit.edit(doc.id, |doc| {
        doc.title = "Changed".into();
        Ok(())
    })?;
    edit.commit(&mut working, |_| Ok(()))?;
    let revision = working.documents()[&doc.id].revision;
    assert!(working.dirty(doc.id));
    let before = ron::to_string(&working)?;
    assert!(
        working
            .undo_checked(|_| Err(kedra_scene::Error("refuse".into())))
            .is_err()
    );
    assert_eq!(ron::to_string(&working)?, before);
    working.undo_checked(|_| Ok(()))?;
    assert!(!working.dirty(doc.id));
    assert!(working.documents()[&doc.id].revision > revision);
    working.redo_checked(|_| Ok(()))?;
    assert!(working.dirty(doc.id));
    Ok(())
}

#[test]
fn payload_refusal_preserves_redo_and_unaffected_documents_do_not_fill_history() -> Result<()> {
    let doc = document()?;
    let mut unrelated = document()?;
    unrelated.title = "x".repeat(70_000);
    let mut working = WorkingSet::default();
    working.open(doc.clone())?;
    working.open(unrelated.clone())?;
    working.set_history_policy(HistoryPolicy {
        max_entries: 2,
        max_payload_bytes: 65_536,
    })?;
    let mut edit = EditTransaction::begin(&working, "Small", BTreeSet::from([doc.id]))?;
    edit.edit(doc.id, |doc| {
        doc.title = "Small".into();
        Ok(())
    })?;
    edit.commit(&mut working, |_| Ok(()))?;
    assert_eq!(
        working.undo_entry().ok_or("history")?.documents,
        BTreeSet::from([doc.id])
    );
    working.undo_checked(|_| Ok(()))?;
    let before = ron::to_string(&working)?;
    let mut edit = EditTransaction::begin(&working, "Large", BTreeSet::from([doc.id]))?;
    edit.edit(doc.id, |doc| {
        doc.title = "x".repeat(70_000);
        Ok(())
    })?;
    assert!(edit.commit(&mut working, |_| Ok(())).is_err());
    assert!(!edit.is_closed());
    assert_eq!(ron::to_string(&working)?, before);
    assert!(working.can_redo());
    assert!(
        working
            .set_history_policy(HistoryPolicy {
                max_entries: 0,
                max_payload_bytes: 65_536
            })
            .is_err()
    );
    assert_eq!(ron::to_string(&working)?, before);
    assert_eq!(working.catalog().documents[&unrelated.id], unrelated);
    Ok(())
}

#[test]
fn grouped_delta_keeps_first_baselines_across_documents_at_retention_limit() -> Result<()> {
    let first = document()?;
    let second = document()?;
    let mut working = WorkingSet::default();
    working.open(first.clone())?;
    working.open(second.clone())?;
    working.set_history_policy(HistoryPolicy {
        max_entries: 1,
        max_payload_bytes: 65_536,
    })?;
    let mut prior = EditTransaction::begin(&working, "Prior", BTreeSet::from([first.id]))?;
    prior.edit(first.id, |doc| {
        doc.title = "Prior".into();
        Ok(())
    })?;
    prior.commit(&mut working, |_| Ok(()))?;
    let baseline = working.catalog().documents;
    let mut group = working.edit_group();
    let mut identity = None;
    for (id, title) in [
        (first.id, "First"),
        (second.id, "Second"),
        (first.id, "Latest"),
    ] {
        let revisions: BTreeMap<_, _> = working
            .documents()
            .iter()
            .map(|(id, doc)| (*id, doc.revision))
            .collect();
        working.transact_grouped_named(
            &mut group,
            "Gesture",
            &revisions,
            |docs| {
                docs.get_mut(&id)
                    .ok_or_else(|| kedra_scene::Error("document".into()))?
                    .title = title.into();
                Ok(())
            },
            |_| Ok(()),
        )?;
        let id = working.undo_entry().ok_or("history")?.id;
        assert_eq!(*identity.get_or_insert(id), id);
    }
    let after = working.catalog().documents;
    working.undo_checked(|_| Ok(()))?;
    assert_eq!(working.catalog().documents, baseline);
    assert!(!working.can_undo());
    working.redo_checked(|_| Ok(()))?;
    assert_eq!(working.catalog().documents, after);
    Ok(())
}
