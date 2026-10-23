//! Authoring workspace preset, empty tool slots and the read-only Asset Browser.
use super::*;
#[cfg(test)]
use kedra_docking::model::Edge;
use kedra_docking::model::{Node, View};
use kedra_ui::docking::class;

#[derive(Default)]
pub(super) struct State {
    pub(super) preset: Option<super::preset::Preset>,
    pub(super) browsers: BTreeMap<ViewId, crate::asset_browser::Panel>,
    pub(super) hierarchies: BTreeMap<ViewId, crate::hierarchy::Panel>,
    pub(super) inspectors: BTreeMap<ViewId, crate::inspector::Panel>,
    pub(super) scenes: BTreeMap<ViewId, crate::scene_view::View>,
    pub(super) scene_layer: usize,
}

/// The 232px Figma specimen must not weaken the minima of a mixed tool group.
pub(super) fn hierarchy_only(snapshot: &kedra_docking::snapshot::Snapshot, group: NodeId) -> bool {
    matches!(snapshot.nodes.get(&group),Some(Node::Group{tabs,..}) if !tabs.is_empty() && tabs.iter().all(|id|snapshot.views.get(id).is_some_and(|view|view.kind=="hierarchy")))
}

/// Replace only the retired demonstration preset. Current user layouts and unknown
/// extension descriptors remain intact; Assets keeps its existing view identity.
pub(super) fn restore(
    host: HostId,
    snapshot: kedra_docking::snapshot::Snapshot,
    preset: &super::preset::Preset,
) -> Result<Workspace, String> {
    snapshot.validate().map_err(|e| e.to_string())?;
    if snapshot.views.values().any(|view| retired(&view.kind)) {
        let records = snapshot
            .views
            .into_iter()
            .filter(|(_, view)| !retired(&view.kind))
            .collect();
        preset.arrange(host, records).map(|(layout, _)| layout)
    } else {
        snapshot.into_workspace().map_err(|e| e.to_string())
    }
}

pub(super) fn retired(kind: &str) -> bool {
    matches!(kind, "project" | "notes" | "activity")
}

pub(super) fn preset(state: &mut EditorWorkspace) -> Result<(), String> {
    arrange(state, BTreeMap::new())
}
fn arrange(state: &mut EditorWorkspace, records: BTreeMap<ViewId, View>) -> Result<(), String> {
    let (layout, focused_group) = state
        .panels
        .preset
        .as_ref()
        .ok_or("Editor preset is not configured")?
        .arrange(state.host, records)?;
    state.layout = layout;
    state.focused_group = Some(focused_group);
    Ok(())
}

#[cfg(test)]
fn arranged(host: HostId, records: BTreeMap<ViewId, View>) -> Result<(Workspace, NodeId), String> {
    super::preset::Preset::test_source()?.arrange(host, records)
}
pub(super) fn mount(world: &mut World, state: &mut EditorWorkspace) -> Result<(), String> {
    let records = state.layout.snapshot().views.clone();
    for (id, record) in records {
        if state.renderer.content(id).is_some() {
            continue;
        }
        if record.kind == "document" || record.kind == "asset-preview" {
            let scene = super::document_views::mount(
                world,
                state.stash,
                id,
                "Source document",
                "Reopen this source from Assets to validate the current source inventory.",
            )?;
            state.renderer.register(id, scene.root)?;
            state.panels.scenes.insert(id, scene);
            state.titles.insert(
                id,
                record
                    .restore_key
                    .as_deref()
                    .and_then(|key| std::path::Path::new(key).file_name())
                    .map_or_else(
                        || "Source document".into(),
                        |name| name.to_string_lossy().into_owned(),
                    ),
            );
            continue;
        }
        if record.kind == "hierarchy" {
            let panel = crate::hierarchy::Panel::mount(world, state.stash, id)?;
            state.renderer.register(id, panel.root())?;
            state.panels.hierarchies.insert(id, panel);
            state.titles.insert(id, "Hierarchy".into());
            continue;
        }
        if record.kind == "inspector" {
            let panel = crate::inspector::Panel::mount(world, state.stash, id)?;
            state.renderer.register(id, panel.root())?;
            state.panels.inspectors.insert(id, panel);
            state.titles.insert(id, "Inspector".into());
            continue;
        }
        if record.kind == "preview" {
            let view = crate::views::mount_asset(
                world,
                state.stash,
                "editor://prefabs/empty-panel.prefab.ron",
                &format!("panel/{id:?}"),
            )?;
            state.titles.insert(id, "Document Preview".into());
            state.renderer.register(id, view.root)?;
            continue;
        }
        if record.kind == "assets"
            && let Some(project) = world
                .resource::<crate::model::ProjectManager>()
                .project
                .as_ref()
        {
            let panel =
                crate::asset_browser::Panel::mount(world, state.stash, id, project.assets.clone())?;
            state.renderer.register(id, panel.root())?;
            state.panels.browsers.insert(id, panel);
            state.titles.insert(id, "Assets".into());
            continue;
        }
        let (title, information) = information(&record.kind);
        let view = crate::views::mount_asset(
            world,
            state.stash,
            "editor://prefabs/dock-content.prefab.ron",
            &format!("panel/{id:?}"),
        )?;
        let heading = view.element("heading").map_err(|e| e.to_string())?;
        let text = view.element("information").map_err(|e| e.to_string())?;
        let draft = view.element("draft").map_err(|e| e.to_string())?;
        if let Some(mut value) = world.get_mut::<Text>(heading) {
            value.0 = title.clone();
        }
        if let Some(mut value) = world.get_mut::<Text>(text) {
            value.0 = information;
        }
        if let Some(parent) = world.get::<ChildOf>(draft).map(ChildOf::parent) {
            class(world, parent, "hidden", true);
        }
        state.titles.insert(id, title);
        state.renderer.register(id, view.root)?;
    }
    Ok(())
}
pub(super) fn poll(world: &mut World, state: &mut EditorWorkspace) {
    for (id, panel) in &mut state.panels.browsers {
        if let Err(error) = panel.tick(world, *id) {
            crate::views::fail(world, error);
        }
    }
    super::document_views::poll(world, state);
    for (id, scene) in &mut state.panels.scenes {
        let host = state
            .layout
            .snapshot()
            .location(*id)
            .and_then(
                |(group, _)| match state.layout.snapshot().nodes.get(&group) {
                    Some(Node::Group { active, .. }) if active == id => {
                        state.layout.snapshot().host_of(group)
                    }
                    _ => None,
                },
            )
            .and_then(|host| state.surfaces.get(&host))
            .filter(|surface| surface.staged.is_none());
        let dpr = host
            .and_then(|surface| world.get::<Window>(surface.window))
            .map_or(1., Window::scale_factor);
        if let Err(error) = scene.tick(world, host.is_some(), dpr) {
            scene.retire(world);
            scene.message(world, &format!("Scene View failed: {error}"), true);
        }
        let selected = state
            .documents
            .opened
            .values()
            .find(|document| document.view == *id)
            .and_then(|document| document.selected.as_ref());
        if let Err(error) = scene.select(world, selected) {
            scene.message(
                world,
                &format!("Selection bounds unavailable: {error}"),
                false,
            );
        }
    }
    for (id, panel) in &mut state.panels.hierarchies {
        if let Err(error) = panel.tick(world, *id, &state.documents) {
            crate::views::fail(world, error);
        }
    }
    for (id, panel) in &mut state.panels.inspectors {
        if let Err(error) = panel.tick(world, *id, &state.documents) {
            crate::views::fail(world, error);
        }
    }
}
fn information(kind: &str) -> (String, String) {
    match kind {
        "assets" => ("Assets".into(),"No project is open. Source browsing requires a validated project.".into()),
        other => (format!("Unavailable: {other}"),"This panel's extension is not installed. Its layout record is retained. Close hides it; reopening does not discard its restore key.".into()),
    }
}

pub(super) fn reset(world: &mut World, state: &mut EditorWorkspace) -> Result<(), String> {
    if state.pending.is_some() {
        return Err("Wait for floating window creation to complete".into());
    }
    let (mut layout, mut focused_group) = state
        .panels
        .preset
        .as_ref()
        .ok_or("Editor preset is not configured")?
        .arrange(state.host, state.layout.snapshot().views.clone())?;
    if let Some(opened) = state
        .documents
        .active
        .and_then(|id| state.documents.opened.get(&id))
    {
        layout
            .execute(Command::Activate(opened.view))
            .map_err(|e| e.to_string())?;
        let placeholders: Vec<_> = layout
            .snapshot()
            .views
            .iter()
            .filter_map(|(id, view)| (view.kind == "preview").then_some(*id))
            .collect();
        for id in placeholders {
            layout
                .execute(Command::Hide(id))
                .map_err(|e| e.to_string())?;
        }
        focused_group = layout
            .snapshot()
            .location(opened.view)
            .ok_or("Open document view disappeared")?
            .0;
    }
    storage::allow_reset(state)?;
    // Recompose known instances rather than recreating native editors.
    let ids: Vec<_> = state.layout.snapshot().views.keys().copied().collect();
    for id in &ids {
        if let Some(root) = state.renderer.content(*id) {
            world.entity_mut(root).insert(ChildOf(state.stash));
        }
    }
    state.layout = layout;
    state.focused_group = Some(focused_group);
    mount(world, state)?;
    state.rendered = None;
    state.preferences.dirty = true;
    // Projection detaches surviving roots before old native hierarchies retire.
    redraw(world, state)?;
    hosts::retire(world, state);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_open_recreates_central_content_and_joins_existing_asset_tabs() -> Result<(), String> {
        let mut world = World::new();
        let window = world.spawn(Window::default()).id();
        let root = world.spawn_empty().id();
        super::super::bind_root(&mut world, window, root)?;
        world.resource_scope(|_,mut state:Mut<EditorWorkspace>|{
            let (layout,_)=arranged(state.host,BTreeMap::new())?;state.layout=layout;
            let tool=|state:&EditorWorkspace,kind:&str|state.layout.snapshot().views.iter().find_map(|(id,v)|(v.kind==kind).then_some(*id)).ok_or("Tool missing");
            let preview=tool(&state,"preview")?;let hierarchy=tool(&state,"hierarchy")?;let assets=tool(&state,"assets")?;
            state.focused_group=state.layout.snapshot().location(hierarchy).map(|(group,_)|group);
            state.layout.execute(Command::Hide(preview)).map_err(|e|e.to_string())?;
            let destination=super::super::document_views::content_destination(&state);
            assert!(matches!(destination,Destination::Edge{target,edge:Edge::Top,..} if Some(target)==state.layout.snapshot().location(assets).map(|(group,_)|group)));
            let asset=ViewId::new();
            state.layout.execute(Command::Open{id:asset,view:View{kind:"asset-preview".into(),restore_key:Some("ship.glb".into())},destination}).map_err(|e|e.to_string())?;
            let group=state.layout.snapshot().location(asset).ok_or("Asset group missing")?.0;
            assert_ne!(Some(group),state.focused_group);
            let destination=super::super::document_views::content_destination(&state);
            assert_eq!(destination,Destination::Tabs{group,index:1});
            let document=ViewId::new();
            state.layout.execute(Command::Open{id:document,view:View{kind:"document".into(),restore_key:Some("Main.scene.ron".into())},destination}).map_err(|e|e.to_string())?;
            assert_eq!(state.layout.snapshot().location(document).map(|(group,_)|group),Some(group));
            Ok(())
        })
    }
    #[test]
    fn narrow_hierarchy_minimum_is_not_applied_to_mixed_groups() -> Result<(), String> {
        let (mut layout, _) = arranged(HostId::new(), BTreeMap::new())?;
        let find = |kind: &str| {
            layout
                .snapshot()
                .views
                .iter()
                .find_map(|(id, view)| (view.kind == kind).then_some(*id))
                .ok_or("Missing tool")
        };
        let hierarchy = find("hierarchy")?;
        let assets = find("assets")?;
        let group = layout
            .snapshot()
            .location(hierarchy)
            .ok_or("Hierarchy group")?
            .0;
        assert!(hierarchy_only(layout.snapshot(), group));
        layout
            .execute(Command::Move {
                selection: kedra_docking::command::Selection::View(assets),
                destination: Destination::Tabs { group, index: 1 },
            })
            .map_err(|e| e.to_string())?;
        assert!(!hierarchy_only(layout.snapshot(), group));
        Ok(())
    }
    #[test]
    fn reset_keeps_source_documents_in_preview_group_not_assets() -> Result<(), String> {
        let document = ViewId::new();
        let record = View {
            kind: "document".into(),
            restore_key: Some("scenes/Main.scene.ron".into()),
        };
        let (layout, _) = arranged(HostId::new(), BTreeMap::from([(document, record.clone())]))?;
        let find = |kind: &str| {
            layout
                .snapshot()
                .views
                .iter()
                .find_map(|(id, view)| (view.kind == kind).then_some(*id))
                .ok_or_else(|| format!("Missing {kind}"))
        };
        let location = |id| layout.snapshot().location(id).map(|(group, _)| group);
        assert_eq!(location(document), location(find("preview")?));
        assert_ne!(location(document), location(find("assets")?));
        assert_eq!(layout.snapshot().views[&document], record);
        Ok(())
    }
    #[test]
    fn r15_failed_reset_preserves_published_layout_roots_and_preferences() {
        use kedra_docking::{
            model::Host,
            snapshot::{MAX_VIEWS, Snapshot, VERSION},
        };
        let mut world = World::new();
        let window = world.spawn(Window::default()).id();
        let root = world.spawn_empty().id();
        super::super::bind_root(&mut world, window, root).unwrap_or_else(|e| panic!("{e}"));
        world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            state.panels.preset =
                Some(super::super::preset::Preset::test_source().unwrap_or_else(|e| panic!("{e}")));
            let group = NodeId::new();
            let tabs: Vec<_> = (0..MAX_VIEWS).map(|_| ViewId::new()).collect();
            let snapshot = Snapshot {
                version: VERSION,
                hosts: BTreeMap::from([(
                    state.host,
                    Host {
                        root: Some(group),
                        maximized: None,
                    },
                )]),
                nodes: BTreeMap::from([(
                    group,
                    Node::Group {
                        tabs: tabs.clone(),
                        active: tabs[0],
                    },
                )]),
                views: tabs
                    .iter()
                    .map(|id| {
                        (
                            *id,
                            View {
                                kind: "unknown".into(),
                                restore_key: Some("日本語".into()),
                            },
                        )
                    })
                    .collect(),
                hidden: BTreeMap::new(),
            };
            let bytes = snapshot.encode().unwrap_or_else(|e| panic!("{e}"));
            state.layout = snapshot
                .clone()
                .into_workspace()
                .unwrap_or_else(|e| panic!("{e}"));
            let parent = world.spawn_empty().id();
            let content = world.spawn(ChildOf(parent)).id();
            state
                .renderer
                .register(tabs[0], content)
                .unwrap_or_else(|e| panic!("{e}"));
            state.preferences.error = Some("Preserve rejected input".into());
            assert!(reset(world, &mut state).is_err());
            assert!(
                state.layout.snapshot() == &snapshot,
                "Failed reset changed published topology"
            );
            assert_eq!(
                state
                    .layout
                    .snapshot()
                    .encode()
                    .unwrap_or_else(|e| panic!("{e}")),
                bytes
            );
            assert_eq!(
                world.get::<ChildOf>(content).map(ChildOf::parent),
                Some(parent)
            );
            assert_eq!(
                state.preferences.error.as_deref(),
                Some("Preserve rejected input")
            );
            assert!(!state.preferences.dirty);
        });
    }

    #[test]
    fn r15_reset_candidate_accepts_exact_limit_without_losing_unknown_records() {
        let records: BTreeMap<_, _> = (0..kedra_docking::snapshot::MAX_VIEWS - 4)
            .map(|_| {
                (
                    ViewId::new(),
                    View {
                        kind: "unknown".into(),
                        restore_key: Some("日本語".into()),
                    },
                )
            })
            .collect();
        let (candidate, focused) =
            arranged(HostId::new(), records.clone()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            candidate.snapshot().views.len(),
            kedra_docking::snapshot::MAX_VIEWS
        );
        assert!(matches!(
            candidate.snapshot().nodes.get(&focused),
            Some(Node::Group { .. })
        ));
        for (id, record) in records {
            assert_eq!(candidate.snapshot().views.get(&id), Some(&record));
        }
        assert!(candidate.snapshot().encode().is_ok());
    }
    #[test]
    fn preset_reset_preserves_view_identity_and_unavailable_extension_records() {
        let mut world = World::new();
        let window = world.spawn(Window::default()).id();
        let root = world.spawn_empty().id();
        super::super::bind_root(&mut world, window, root).unwrap_or_else(|e| panic!("{e}"));
        let mut state = world.resource_mut::<EditorWorkspace>();
        state.panels.preset =
            Some(super::super::preset::Preset::test_source().unwrap_or_else(|e| panic!("{e}")));
        preset(&mut state).unwrap_or_else(|e| panic!("{e}"));
        let id = ViewId::new();
        let destination = super::super::destination(&state);
        state
            .layout
            .execute(Command::Open {
                id,
                view: View {
                    kind: "unavailable.extension".into(),
                    restore_key: Some("document:日本語".into()),
                },
                destination,
            })
            .unwrap_or_else(|e| panic!("{e}"));
        let records = state.layout.snapshot().views.clone();
        state.layout = Workspace::new(state.host);
        arrange(&mut state, records.clone()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(state.layout.snapshot().views, records);
        assert_eq!(
            state
                .layout
                .snapshot()
                .nodes
                .values()
                .filter(|node| matches!(node, Node::Group { .. }))
                .count(),
            4
        );
    }
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod lifecycle_tests;
