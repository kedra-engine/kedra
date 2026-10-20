//! Product preset placement and the one-way retirement of demonstration tools.
use super::*;
use kedra_docking::{
    command::Selection,
    model::{Axis, Edge, Node, View},
};
use test_support::TestResult;

fn group(layout: &Workspace, kind: &str) -> Result<NodeId, String> {
    let id = layout
        .snapshot()
        .views
        .iter()
        .find_map(|(id, view)| (view.kind == kind).then_some(*id))
        .ok_or_else(|| format!("Missing preset view: {kind}"))?;
    layout
        .snapshot()
        .location(id)
        .map(|(group, _)| group)
        .ok_or_else(|| format!("Preset view is hidden: {kind}"))
}

#[test]
fn preset_places_four_independent_visible_tools_around_preview() -> TestResult {
    let (mut app, _, _) = input::regression_tests::setup()?;
    let mut state = app.world_mut().resource_mut::<EditorWorkspace>();
    panels::preset(&mut state)?;
    let snapshot = state.layout.snapshot();
    assert_eq!(snapshot.views.len(), 4);
    assert!(snapshot.hidden.is_empty());
    let root = snapshot.hosts[&state.host].root.ok_or("root")?;
    let Node::Split {
        axis: Axis::Horizontal,
        first,
        second,
        fraction,
        ..
    } = snapshot.nodes[&root]
    else {
        panic!("left column split")
    };
    assert_eq!(first, group(&state.layout, "hierarchy")?);
    assert!((fraction - 0.20).abs() < 0.001);
    let Node::Split {
        axis: Axis::Horizontal,
        first,
        second: inspector,
        fraction,
        ..
    } = snapshot.nodes[&second]
    else {
        panic!("right column split")
    };
    assert_eq!(inspector, group(&state.layout, "inspector")?);
    assert!((fraction - 0.75).abs() < 0.001);
    let Node::Split {
        axis: Axis::Vertical,
        first: preview,
        second: assets,
        fraction,
        ..
    } = snapshot.nodes[&first]
    else {
        panic!("center stack")
    };
    assert_eq!(preview, group(&state.layout, "preview")?);
    assert_eq!(assets, group(&state.layout, "assets")?);
    assert!((fraction - 0.68).abs() < 0.001);
    assert_eq!(state.focused_group, Some(preview));
    Ok(())
}

#[test]
fn retired_demo_layout_is_replaced_but_assets_and_unknown_descriptors_survive() -> TestResult {
    let host = HostId::new();
    let mut old = Workspace::new(host);
    let assets = ViewId::new();
    old.execute(Command::Open {
        id: assets,
        view: View {
            kind: "assets".into(),
            restore_key: Some("source-root".into()),
        },
        destination: Destination::EmptyHost(host),
    })?;
    let source = old.snapshot().location(assets).ok_or("assets")?.0;
    let unknown = ViewId::new();
    for (id, kind) in [
        (ViewId::new(), "project"),
        (ViewId::new(), "notes"),
        (ViewId::new(), "activity"),
        (unknown, "extension.tool"),
    ] {
        old.execute(Command::Open {
            id,
            view: View {
                kind: kind.into(),
                restore_key: Some("日本語".into()),
            },
            destination: Destination::Tabs {
                group: source,
                index: 0,
            },
        })?;
    }
    old.execute(Command::Hide(assets))?;
    let candidate = panels::restore(host, old.snapshot().clone())?;
    assert_eq!(candidate.snapshot().views.len(), 5);
    for id in [assets, unknown] {
        assert_eq!(candidate.snapshot().views[&id], old.snapshot().views[&id]);
    }
    assert!(candidate.snapshot().location(assets).is_some());
    assert!(
        !candidate
            .snapshot()
            .views
            .values()
            .any(|v| matches!(v.kind.as_str(), "project" | "notes" | "activity"))
    );
    assert_eq!(
        old.snapshot().views.len(),
        5,
        "Migration must not mutate its input"
    );
    Ok(())
}

#[test]
fn current_custom_layout_is_restored_without_rearranging_or_reopening_tools() -> TestResult {
    let (mut app, _, _) = input::regression_tests::setup()?;
    let mut state = app.world_mut().resource_mut::<EditorWorkspace>();
    panels::preset(&mut state)?;
    let snapshot = state.layout.snapshot();
    let preview = snapshot
        .views
        .iter()
        .find_map(|(id, v)| (v.kind == "preview").then_some(*id))
        .ok_or("preview")?;
    let assets = group(&state.layout, "assets")?;
    state.layout.execute(Command::Move {
        selection: Selection::View(preview),
        destination: Destination::Edge {
            target: assets,
            edge: Edge::Left,
            fraction: 0.5,
        },
    })?;
    state.layout.execute(Command::Hide(preview))?;
    let before = state.layout.snapshot().clone();
    let restored = panels::restore(state.host, before.clone())?;
    assert_eq!(restored.snapshot(), &before);
    Ok(())
}
