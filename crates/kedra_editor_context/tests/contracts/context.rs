use crate::support::*;
use kedra_authoring::transaction::EditTransaction;
use kedra_editor_context::{
    AssetKind, AssetLibrary, DragPayload, ScopeId, Selection, SelectionItem,
};
use std::collections::BTreeSet;

#[test]
fn exact_source_paths_and_lifetimes_preserve_identity() -> Result {
    let fixture = Fixture::new()?;
    let library = fixture.library();
    let asset = library.asset("game://Images/Маяк.png").ok_or("asset")?;
    assert_eq!(asset.identity(), fixture.source.assets[0].id);
    assert_eq!(asset.path(), "game://Images/Маяк.png");
    assert_eq!(
        library.asset("game://Images").ok_or("folder")?.kind(),
        &AssetKind::Folder
    );
    assert_eq!(
        library.asset("game://").ok_or("root")?.kind(),
        &AssetKind::Folder
    );
    for path in [
        "editor://Images/Маяк.png",
        "game://images/Маяк.png",
        "game://Images/../Images/Маяк.png",
        "game://Images/",
        "game:///Images",
        "game://Images//Маяк.png",
        "game://Images/Маяк.png#part",
        "game://missing",
    ] {
        assert!(library.asset(path).is_none(), "Accepted {path}");
    }
    let document = library
        .asset("game://Scenes/Main.prefab.ron")
        .ok_or("document")?;
    assert_eq!(document.kind(), &AssetKind::Document(fixture.document.id));
    assert_eq!(
        library.root(&document).ok_or("root node")?.node,
        fixture.document.root.into()
    );
    let retired = AssetLibrary::new(ScopeId::default(), &fixture.source, &fixture.working);
    assert!(!retired.contains_asset(&asset));
    assert!(retired.root(&document).is_none());
    assert!(!fixture.working.can_undo());
    assert!(!fixture.working.dirty(fixture.document.id));
    Ok(())
}

#[test]
fn selection_deduplicates_and_refuses_foreign_or_oversized_requests_atomically() -> Result {
    let fixture = Fixture::new()?;
    let library = fixture.library();
    let asset = SelectionItem::Asset(library.asset("game://Images/Маяк.png").ok_or("asset")?);
    let node = SelectionItem::Node(
        library
            .node(fixture.document.id, fixture.child.into())
            .ok_or("node")?,
    );
    let mut selection = Selection::default();
    selection.replace(vec![asset.clone(), node.clone(), asset.clone()], &library)?;
    assert_eq!(selection.items(), &[asset.clone(), node.clone()]);
    assert_eq!(selection.active(), Some(&node));
    let before = selection.clone();
    selection.replace(vec![asset.clone(), node.clone()], &library)?;
    assert_eq!(selection, before);
    let foreign = Fixture::new()?;
    let foreign = SelectionItem::Node(
        foreign
            .library()
            .node(foreign.document.id, foreign.document.root.into())
            .ok_or("foreign")?,
    );
    assert!(
        selection
            .replace(vec![asset.clone(), foreign.clone()], &library)
            .is_err()
    );
    assert_eq!(selection, before);
    assert!(selection.toggle(foreign, &library).is_err());
    assert_eq!(selection, before);
    assert!(
        selection
            .replace(vec![asset.clone(); 4097], &library)
            .is_err()
    );
    assert_eq!(selection, before);
    selection.toggle(node, &library)?;
    assert_eq!(selection.active(), Some(&asset));
    selection.toggle(asset, &library)?;
    assert!(selection.items().is_empty());
    assert!(selection.active().is_none());
    assert!(!fixture.working.can_undo());
    Ok(())
}

#[test]
fn drag_keeps_all_homogeneous_members_and_rejects_mixed_or_empty_selection() -> Result {
    let fixture = Fixture::new()?;
    let library = fixture.library();
    let asset = SelectionItem::Asset(library.asset("game://Images/Маяк.png").ok_or("asset")?);
    let folder = SelectionItem::Asset(library.asset("game://Scenes").ok_or("folder")?);
    let node = SelectionItem::Node(
        library
            .node(fixture.document.id, fixture.child.into())
            .ok_or("node")?,
    );
    let mut selection = Selection::default();
    assert!(DragPayload::capture(&selection).is_err());
    assert!(!DragPayload::Assets(vec![]).current(&library));
    assert!(!DragPayload::Nodes(vec![]).current(&library));
    selection.replace(vec![asset.clone(), folder], &library)?;
    let payload = DragPayload::capture(&selection)?;
    assert!(matches!(&payload, DragPayload::Assets(assets) if assets.len() == 2));
    assert!(payload.current(&library));
    selection.replace(vec![asset, node.clone()], &library)?;
    let before = selection.clone();
    assert!(DragPayload::capture(&selection).is_err());
    assert_eq!(selection, before);
    selection.replace(vec![node], &library)?;
    assert!(matches!(
        DragPayload::capture(&selection)?,
        DragPayload::Nodes(_)
    ));
    assert!(!payload.current(&AssetLibrary::new(
        ScopeId::default(),
        &fixture.source,
        &fixture.working
    )));
    Ok(())
}

#[test]
fn reconciliation_retires_removed_nodes_without_rewriting_source_history() -> Result {
    let mut fixture = Fixture::new()?;
    let library = fixture.library();
    let asset = SelectionItem::Asset(library.asset("game://Images/Маяк.png").ok_or("asset")?);
    let node = SelectionItem::Node(
        library
            .node(fixture.document.id, fixture.child.into())
            .ok_or("node")?,
    );
    let mut selection = Selection::default();
    selection.replace(vec![node.clone()], &library)?;
    let payload = DragPayload::capture(&selection)?;
    selection.replace(vec![asset.clone(), node], &library)?;
    let mut edit = EditTransaction::begin(
        &fixture.working,
        "Remove child",
        BTreeSet::from([fixture.document.id]),
    )?;
    edit.edit(fixture.document.id, |document| {
        document.nodes.retain(|node| node.id != fixture.child);
        Ok(())
    })?;
    edit.commit(&mut fixture.working, |_| Ok(()))?;
    let history = fixture.working.undo_entry().ok_or("history")?.id;
    let revision = fixture.working.documents()[&fixture.document.id].revision;
    assert!(!payload.current(&fixture.library()));
    selection.reconcile(&fixture.library());
    assert_eq!(selection.items(), &[asset]);
    assert_eq!(fixture.working.undo_entry().ok_or("history")?.id, history);
    assert_eq!(
        fixture.working.documents()[&fixture.document.id].revision,
        revision
    );
    fixture.working.undo_checked(|_| Ok(()))?;
    assert!(payload.current(&fixture.library()));
    selection.reconcile(&AssetLibrary::new(
        ScopeId::default(),
        &fixture.source,
        &fixture.working,
    ));
    assert!(selection.items().is_empty());
    Ok(())
}
