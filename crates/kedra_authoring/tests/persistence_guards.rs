//! Exact output admission and identity/path ownership precede Save publication.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated persistence fixture assertions"
)]
mod common;
use bevy::prelude::*;
use common::{Result, baseline, document, setup};
use kedra_authoring::persistence::*;
use kedra_scene::{assets::CatalogLoader, document::*};
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
fn save_rejects_oversized_and_pretty_expanded_documents_atomically() -> Result<()> {
    let (app, policies) = setup()?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    for expanded in [false, true] {
        let original = document()?;
        let mut edited = original.clone();
        if expanded {
            while ron::ser::to_string_pretty(&edited, ron::ser::PrettyConfig::default())?.len()
                <= 65_536
            {
                edited.nodes.push(kedra_scene::document::Node {
                    id: NodeId::new(),
                    name: "Child".repeat(16),
                    parent: Some(edited.root.into()),
                    components: BTreeMap::new(),
                });
            }
            let compact = ron::to_string(&edited)?;
            assert!(
                compact.len() <= 65_536,
                "fixture must isolate pretty-print expansion"
            );
            PrefabDocument::parse(&compact)?;
        } else {
            edited.title = "x".repeat(65_536);
        }
        let mut workspace = Workspace::open(
            baseline(std::slice::from_ref(&original))?,
            &types.read(),
            &policies,
            &mut loader,
        );
        workspace.working.transact_checked(
            &BTreeMap::from([(original.id, 1)]),
            |documents| {
                documents.insert(edited.id, edited.clone());
                Ok(())
            },
            |_| Ok(()),
        )?;
        let before = ron::to_string(&workspace)?;
        assert!(
            workspace
                .prepare_save(None, &types.read(), &policies, &mut loader)
                .is_err()
        );
        assert_eq!(ron::to_string(&workspace)?, before);
        assert!(workspace.pending().is_none());
        assert!(workspace.working.dirty(original.id));
    }
    Ok(())
}

#[test]
fn exact_document_byte_boundary_roundtrips_and_rejected_retry_keeps_pending_bytes() -> Result<()> {
    let (app, policies) = setup()?;
    let mut original = document()?;
    original.title.clear();
    let fixed = original.serialize()?.len();
    let mut workspace = {
        let types = app.world().resource::<AppTypeRegistry>();
        let mut loader = app.world().resource::<AssetServer>();
        Workspace::open(
            baseline(std::slice::from_ref(&original))?,
            &types.read(),
            &policies,
            &mut loader,
        )
    };
    workspace.working.transact_checked(
        &BTreeMap::from([(original.id, 1)]),
        |documents| {
            documents.get_mut(&original.id).unwrap().title = "x".repeat(65_536 - fixed);
            Ok(())
        },
        |_| Ok(()),
    )?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let request = workspace
        .prepare_save(None, &types.read(), &policies, &mut loader)?
        .clone();
    let source = request
        .files
        .iter()
        .find(|f| f.document == Some(original.id))
        .unwrap()
        .source
        .as_ref()
        .unwrap();
    assert_eq!(source.len(), 65_536);
    PrefabDocument::parse(source)?;
    let revision = workspace.working.documents()[&original.id].revision;
    workspace.working.transact_checked(
        &BTreeMap::from([(original.id, revision)]),
        |documents| {
            documents.get_mut(&original.id).unwrap().title.push('x');
            Ok(())
        },
        |_| Ok(()),
    )?;
    assert!(
        workspace
            .prepare_save(None, &types.read(), &policies, &mut loader)
            .is_err()
    );
    assert_eq!(
        ron::to_string(workspace.pending().unwrap())?,
        ron::to_string(&request)?
    );
    workspace.acknowledge(&receipt(&request))?;
    assert!(workspace.working.dirty(original.id));
    let reopened = Workspace::open(
        workspace.source.clone(),
        &types.read(),
        &policies,
        &mut loader,
    );
    assert!(reopened.diagnostic.is_none());
    let before = ron::to_string(&workspace)?;
    assert!(
        workspace
            .prepare_save(None, &types.read(), &policies, &mut loader)
            .is_err()
    );
    assert_eq!(ron::to_string(&workspace)?, before);
    Ok(())
}

#[test]
fn save_rejects_oversized_or_empty_catalog_without_installing_a_request() -> Result<()> {
    let (app, policies) = setup()?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    for count in [0, 80] {
        let documents = (0..count).map(|_| document()).collect::<Result<Vec<_>>>()?;
        let mut source = baseline(&documents)?;
        for file in &mut source.documents {
            file.path = format!("{}/{}", "x".repeat(900), file.path);
        }
        let mut workspace = Workspace::open(source, &types.read(), &policies, &mut loader);
        assert!(workspace.diagnostic.is_none());
        let before = ron::to_string(&workspace)?;
        assert!(
            workspace
                .prepare_save(None, &types.read(), &policies, &mut loader)
                .is_err()
        );
        assert_eq!(ron::to_string(&workspace)?, before);
    }
    Ok(())
}

#[test]
fn reconciliation_rejects_reused_dirty_paths_atomically() -> Result<()> {
    let (app, policies) = setup()?;
    let a = document()?;
    let b = document()?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    for asset_replacement in [false, true] {
        let mut workspace = Workspace::open(
            baseline(std::slice::from_ref(&a))?,
            &types.read(),
            &policies,
            &mut loader,
        );
        workspace.working.transact_checked(
            &BTreeMap::from([(a.id, 1)]),
            |documents| {
                documents.get_mut(&a.id).unwrap().title = "Dirty A".into();
                Ok(())
            },
            |_| Ok(()),
        )?;
        let old_path = workspace.path(a.id).unwrap().to_owned();
        let mut incoming = baseline(if asset_replacement {
            &[]
        } else {
            std::slice::from_ref(&b)
        })?;
        incoming.generation = 2;
        if asset_replacement {
            incoming.assets.push(SourceAsset {
                id: None,
                path: old_path.clone(),
                hash: content_hash("external asset"),
                bytes: 14,
            });
        } else {
            incoming.documents[0].path = old_path;
        }
        let before = ron::to_string(&workspace)?;
        assert!(
            workspace
                .keep_working(incoming, &types.read(), &policies, &mut loader)
                .is_err()
        );
        assert_eq!(ron::to_string(&workspace)?, before);
        // Refusal does not poison a subsequent valid Save or its exact receipt.
        let request = workspace
            .prepare_save(None, &types.read(), &policies, &mut loader)?
            .clone();
        workspace.acknowledge(&receipt(&request))?;
        assert!(
            Workspace::open(workspace.source, &types.read(), &policies, &mut loader)
                .diagnostic
                .is_none()
        );
    }
    Ok(())
}

#[test]
fn independent_save_guard_rejects_duplicate_restored_path_assignments() -> Result<()> {
    let (app, policies) = setup()?;
    let a = document()?;
    let b = document()?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let workspace = Workspace::open(
        baseline(&[a.clone(), b.clone()])?,
        &types.read(),
        &policies,
        &mut loader,
    );
    // Workspace is serializable for host session restoration; reject conflicting restored state too.
    let old = workspace.path(b.id).unwrap();
    let reused = workspace.path(a.id).unwrap();
    let encoded = ron::to_string(&workspace)?;
    let (baseline, assignments) = encoded.split_once("paths:").unwrap();
    let mut workspace: Workspace = ron::from_str(&format!(
        "{baseline}paths:{}",
        assignments.replace(old, reused)
    ))?;
    workspace.working.transact_checked(
        &BTreeMap::from([(a.id, 1)]),
        |documents| {
            documents.get_mut(&a.id).unwrap().title = "Dirty A".into();
            Ok(())
        },
        |_| Ok(()),
    )?;
    let before = ron::to_string(&workspace)?;
    assert!(
        workspace
            .prepare_save(None, &types.read(), &policies, &mut loader)
            .is_err()
    );
    assert_eq!(ron::to_string(&workspace)?, before);
    Ok(())
}

#[test]
fn undo_creation_keeps_save_inventory_live_and_redo_restores_coupling() -> Result<()> {
    let (app, policies) = setup()?;
    let a = document()?;
    let b = document()?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let mut workspace = Workspace::open(
        baseline(std::slice::from_ref(&a))?,
        &types.read(),
        &policies,
        &mut loader,
    );
    workspace.working.transact_checked(
        &BTreeMap::from([(a.id, 1)]),
        |documents| {
            documents.get_mut(&a.id).unwrap().title = "Edited A".into();
            documents.insert(b.id, b.clone());
            Ok(())
        },
        |_| Ok(()),
    )?;
    workspace.assign_new_path(b.id)?;
    workspace.working.undo_checked(|_| Ok(()))?;
    for selected in [Some(a.id), None] {
        let mut candidate = workspace.clone();
        let request = candidate
            .prepare_save(selected, &types.read(), &policies, &mut loader)?
            .clone();
        assert!(!request.files.iter().any(|file| file.document == Some(b.id)));
        candidate.acknowledge(&receipt(&request))?;
        assert!(!candidate.index_dirty());
        assert!(
            Workspace::open(
                candidate.source.clone(),
                &types.read(),
                &policies,
                &mut loader
            )
            .diagnostic
            .is_none()
        );
        candidate.working.redo_checked(|_| Ok(()))?;
        assert_eq!(candidate.working.save_group(a.id), [a.id, b.id].into());
        let request = candidate
            .prepare_save(Some(a.id), &types.read(), &policies, &mut loader)?
            .clone();
        assert!(request.files.iter().any(|file| file.document == Some(b.id)));
    }
    Ok(())
}

#[test]
fn accepted_catalog_output_uses_the_native_loader_contract() -> Result<()> {
    let (app, policies) = setup()?;
    let a = document()?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let mut workspace = Workspace::open(baseline(&[a])?, &types.read(), &policies, &mut loader);
    let request = workspace
        .prepare_save(None, &types.read(), &policies, &mut loader)?
        .clone();
    let index = request
        .files
        .iter()
        .find(|file| file.document.is_none())
        .unwrap()
        .source
        .as_ref()
        .unwrap();
    let paths = CatalogLoader::parse(index)?;
    assert_eq!(paths.len(), 1);
    workspace.acknowledge(&receipt(&request))?;
    assert!(
        Workspace::open(workspace.source, &types.read(), &policies, &mut loader)
            .diagnostic
            .is_none()
    );
    Ok(())
}

#[test]
fn exact_serialized_catalog_boundary_is_admitted_but_one_more_byte_is_atomic_failure() -> Result<()>
{
    let (app, policies) = setup()?;
    let documents = (0..16).map(|_| document()).collect::<Result<Vec<_>>>()?;
    let mut source = baseline(&documents)?;
    for file in &mut source.documents {
        file.path = format!("{}{}", "x".repeat(4000 - file.path.len()), file.path);
    }
    let index_bytes = |source: &SourceSnapshot| -> Result<String> {
        let mut paths: Vec<_> = source
            .documents
            .iter()
            .map(|file| format!("game://{}", file.path))
            .collect();
        paths.sort();
        Ok(ron::ser::to_string_pretty(&paths, ron::ser::PrettyConfig::default())? + "\n")
    };
    let mut extra = 65_536 - index_bytes(&source)?.len();
    for file in &mut source.documents {
        let count = extra.min(4089 - file.path.len());
        file.path.insert_str(0, &"x".repeat(count));
        extra -= count;
    }
    assert_eq!(extra, 0, "fixture must fit valid individual native paths");
    assert_eq!(index_bytes(&source)?.len(), 65_536);
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let mut workspace = Workspace::open(source.clone(), &types.read(), &policies, &mut loader);
    let request = workspace
        .prepare_save(None, &types.read(), &policies, &mut loader)?
        .clone();
    let index = request
        .files
        .iter()
        .find(|file| file.document.is_none())
        .unwrap()
        .source
        .as_ref()
        .unwrap();
    assert_eq!(index.len(), 65_536);
    assert_eq!(CatalogLoader::parse(index)?.len(), 16);
    workspace.acknowledge(&receipt(&request))?;
    assert!(
        Workspace::open(
            workspace.source.clone(),
            &types.read(),
            &policies,
            &mut loader
        )
        .diagnostic
        .is_none()
    );
    source
        .documents
        .iter_mut()
        .find(|file| file.path.len() < 4089)
        .unwrap()
        .path
        .insert(0, 'x');
    source.generation = workspace.source.generation + 1;
    workspace.keep_working(source, &types.read(), &policies, &mut loader)?;
    let before = ron::to_string(&workspace)?;
    assert!(
        workspace
            .prepare_save(None, &types.read(), &policies, &mut loader)
            .is_err()
    );
    assert_eq!(ron::to_string(&workspace)?, before);
    Ok(())
}

#[test]
fn collision_free_external_rename_retains_dirty_values_and_saves_a_reopenable_inventory()
-> Result<()> {
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
        |documents| {
            documents.get_mut(&original.id).unwrap().title = "Retained local edit".into();
            Ok(())
        },
        |_| Ok(()),
    )?;
    let mut incoming = baseline(std::slice::from_ref(&original))?;
    incoming.documents[0].path = "renamed.prefab.ron".into();
    incoming.generation = 2;
    workspace.keep_working(incoming, &types.read(), &policies, &mut loader)?;
    assert_eq!(workspace.path(original.id), Some("renamed.prefab.ron"));
    assert_eq!(
        workspace.working.documents()[&original.id].document.title,
        "Retained local edit"
    );
    let request = workspace
        .prepare_save(None, &types.read(), &policies, &mut loader)?
        .clone();
    assert_eq!(
        request
            .files
            .iter()
            .filter(|file| file.document.is_some())
            .count(),
        1
    );
    workspace.acknowledge(&receipt(&request))?;
    assert!(
        Workspace::open(workspace.source, &types.read(), &policies, &mut loader)
            .diagnostic
            .is_none()
    );
    Ok(())
}
