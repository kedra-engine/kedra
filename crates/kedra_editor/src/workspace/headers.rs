//! Host composition policy; reusable headers know nothing about dock topology.
use super::*;
use kedra_docking::{model::Node, snapshot::Snapshot};
use kedra_ui::docking::HeaderDestination;

pub(crate) fn bind_header(world: &mut World, view: &UiView) {
    for (key, action) in [
        ("editor.panels", input::Action::Panels),
        ("editor.save", input::Action::SaveSource),
        ("editor.reset", input::Action::Reset),
    ] {
        if let Ok(entity) = view.element(key) {
            world.entity_mut(entity).insert(action);
        }
    }
    if let Ok(entity) = view.element("editor.project-menu") {
        world.entity_mut(entity).remove::<(
            bevy::ui_widgets::Button,
            bevy::input_focus::tab_navigation::TabIndex,
        )>();
        kedra_ui::docking::class(world, entity, "project-label", true);
    }
    if let Ok(entity) = view.element("editor.project-chevron") {
        kedra_ui::docking::class(world, entity, "hidden", true);
    }
    for (key, identity) in [
        ("editor.panels", "workspace/panels"),
        ("editor.reset", "workspace/reset"),
    ] {
        if let Ok(entity) = view.element(key) {
            world
                .entity_mut(entity)
                .insert(kedra_ui::ElementId::new(identity));
        }
    }
}

pub(super) fn committed(
    world: &mut World,
    state: &EditorWorkspace,
) -> Result<BTreeMap<NodeId, HeaderDestination>, String> {
    let mut destinations = BTreeMap::new();
    for (id, surface) in &state.surfaces {
        if *id == state.host
            || surface.staged.is_some()
            || surface.owned.is_none()
            || state
                .layout
                .snapshot()
                .hosts
                .get(id)
                .is_none_or(|host| host.root.is_none())
        {
            continue;
        }
        let group = single_group(state.layout.snapshot(), *id);
        if let Some(destination) =
            crate::chrome::floating_destination(world, surface.window, group.is_some())?
            && let Some(group) = group
        {
            destinations.insert(group, destination);
        }
    }
    Ok(destinations)
}
fn single_group(snapshot: &Snapshot, host: HostId) -> Option<NodeId> {
    snapshot
        .hosts
        .get(&host)
        .and_then(|h| h.root)
        .filter(|id| matches!(snapshot.nodes.get(id), Some(Node::Group { .. })))
}

pub(super) fn measurement(
    world: &mut World,
    state: &EditorWorkspace,
    snapshot: &Snapshot,
    roots: &BTreeMap<HostId, Entity>,
) -> Result<BTreeMap<NodeId, HeaderDestination>, String> {
    let mut destinations = BTreeMap::new();
    for (host, root) in roots {
        if *host == state.host {
            continue;
        }
        let Some(group) = single_group(snapshot, *host) else {
            continue;
        };
        // Authored measurement uses exactly the same skin as the committed row.
        // It is a passive child of the measurement owner and cannot receive input.
        let view = crate::views::mount_asset(
            world,
            *root,
            "editor://prefabs/titlebar.prefab.ron",
            "measurement-caption",
        )?;
        let mut classes = bevy_flair::style::components::ClassList::new_with_classes([
            "titlebar",
            "floating-titlebar",
            "measurement-caption",
        ]);
        if cfg!(target_os = "macos") {
            classes.toggle("macos");
        }
        world
            .entity_mut(view.root)
            .insert((classes, Pickable::IGNORE));
        let row = view.element("window.tabs").map_err(|e| e.to_string())?;
        let menu = view.element("window.menu").map_err(|e| e.to_string())?;
        if cfg!(target_os = "macos") {
            let mut order = world
                .get::<Children>(view.root)
                .map(|c| c.to_vec())
                .unwrap_or_default();
            order.retain(|e| *e != menu);
            order.push(menu);
            world.entity_mut(view.root).replace_children(&order);
        }
        destinations.insert(group, HeaderDestination { row, menu });
    }
    Ok(destinations)
}
