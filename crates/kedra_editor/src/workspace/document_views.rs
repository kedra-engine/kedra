//! Read-only source document tabs use ordinary docking, never another tab system.
use super::*;
use kedra_docking::model::{Edge, Node, View};

/// Reconcile only published group activations, including candidate commits that
/// bypass `apply`. Tool focus alone never changes the content target.
pub(super) fn reconcile_activation(state: &mut EditorWorkspace) {
    let actives: BTreeMap<_, _> = state
        .layout
        .snapshot()
        .nodes
        .iter()
        .filter_map(|(group, node)| {
            if let Node::Group { active, .. } = node {
                Some((*group, *active))
            } else {
                None
            }
        })
        .collect();
    let is_content = |view: ViewId| {
        state
            .layout
            .snapshot()
            .views
            .get(&view)
            .is_some_and(|view| {
                matches!(view.kind.as_str(), "document" | "asset-preview" | "preview")
            })
    };
    let changed = actives
        .iter()
        .filter(|(group, view)| {
            state.content_actives.get(group) != Some(view) && is_content(**view)
        })
        // A moved/floated active content view wins over the sibling that docking
        // activates in its vacated source group. Never depend on random group IDs.
        .min_by_key(|(group, view)| {
            (
                !state.content_actives.values().any(|old| old == *view),
                **group,
            )
        })
        .map(|(_, view)| *view);
    let retired = state
        .content_view
        .is_some_and(|view| state.layout.snapshot().location(view).is_none());
    let next = changed.or_else(|| {
        retired
            .then(|| actives.values().copied().find(|view| is_content(*view)))
            .flatten()
    });
    if let Some(view) = next {
        activate_content(state, view);
    } else if retired {
        state.content_view = None;
        state.documents.active = None;
    }
    state.content_actives = actives;
}

/// New/reopened content follows content groups, never an arbitrary tool group.
pub(super) fn content_destination(state: &EditorWorkspace) -> Destination {
    let snapshot = state.layout.snapshot();
    let group = snapshot
        .views
        .iter()
        .filter(|(_, view)| matches!(view.kind.as_str(), "document" | "asset-preview" | "preview"))
        .filter_map(|(id, view)| {
            snapshot
                .location(*id)
                .map(|(group, _)| (group, view.kind == "preview"))
        })
        .min_by_key(|(group, placeholder)| {
            (
                snapshot.host_of(*group) != Some(state.host),
                *placeholder,
                *group,
            )
        })
        .map(|(group, _)| group);
    if let Some(group) = group
        && let Some(Node::Group { tabs, .. }) = snapshot.nodes.get(&group)
    {
        return Destination::Tabs {
            group,
            index: tabs.len(),
        };
    }
    // Closing the last content tab collapses its group. Recreate the central
    // region above Assets (or inward of a side tool), without rearranging tools.
    for (kind, edge, fraction) in [
        ("assets", Edge::Top, 0.68),
        ("hierarchy", Edge::Right, 0.75),
        ("inspector", Edge::Left, 0.75),
    ] {
        if let Some(group) = snapshot
            .views
            .iter()
            .filter(|(_, view)| view.kind == kind)
            .filter_map(|(id, _)| snapshot.location(*id).map(|(group, _)| group))
            .find(|group| snapshot.host_of(*group) == Some(state.host))
        {
            return Destination::Edge {
                target: group,
                edge,
                fraction,
            };
        }
    }
    snapshot
        .hosts
        .get(&state.host)
        .and_then(|host| host.root)
        .map_or(Destination::EmptyHost(state.host), |target| {
            Destination::Edge {
                target,
                edge: Edge::Right,
                fraction: 0.7,
            }
        })
}

pub(super) fn poll(world: &mut World, state: &mut EditorWorkspace) {
    let requests: Vec<_> = state
        .panels
        .browsers
        .iter_mut()
        .filter_map(|(id, browser)| browser.take_open().map(|path| (*id, path)))
        .collect();
    for (origin, path) in requests {
        let root = world
            .resource::<crate::model::ProjectManager>()
            .project
            .as_ref()
            .map(|project| project.assets.clone());
        if let Some(root) = root {
            state.documents.origin = Some(origin);
            if let Err(error) = state.documents.request(world, root, path) {
                state.documents.error = Some(error);
            }
        }
    }
    if let Some(ready) = state.documents.poll() {
        let path = ready.path.clone();
        match publish(world, state, ready) {
            Err(error) => state.documents.error = Some(error),
            Ok(()) => {
                if let Some(browser) = state
                    .documents
                    .origin
                    .and_then(|id| state.panels.browsers.get_mut(&id))
                {
                    browser.clear_message();
                }
                if let Some(mut text) = state
                    .status
                    .and_then(|entity| world.get_mut::<Text>(entity))
                {
                    text.0 = format!(
                        "Source open: {} (accepted edits remain in memory until Save)",
                        crate::display_paths::asset(&path)
                    );
                }
            }
        }
    }
    if let Some(path) = state.documents.take_asset() {
        match super::asset_views::publish(world, state, path.clone()) {
            Err(error) => state.documents.error = Some(error),
            Ok(()) => {
                if let Some(browser) = state
                    .documents
                    .origin
                    .and_then(|id| state.panels.browsers.get_mut(&id))
                {
                    browser.clear_message();
                }
                if let Some(mut text) = state
                    .status
                    .and_then(|entity| world.get_mut::<Text>(entity))
                {
                    text.0 = format!(
                        "Asset open: {} (read-only)",
                        crate::display_paths::asset(&path)
                    );
                }
            }
        }
    }
    if let Some(error) = state.documents.error.take() {
        state.documents.failure = Some(error.clone());
        if let Some(browser) = state
            .documents
            .origin
            .and_then(|id| state.panels.browsers.get_mut(&id))
        {
            browser.message(format!("Cannot open source: {error}"));
        }
        notice(world, state, &error);
    }
}
fn publish(
    world: &mut World,
    state: &mut EditorWorkspace,
    ready: crate::documents::Ready,
) -> Result<(), String> {
    if let Some(opened) = state.documents.opened.get(&ready.document) {
        let view = opened.view;
        if state.layout.snapshot().hidden.contains_key(&view) {
            let target = content_destination(state);
            state
                .layout
                .execute(Command::Show {
                    view,
                    destination: target,
                })
                .map_err(|e| e.to_string())?;
        }
        return apply(world, state, Command::Activate(view));
    }
    let (title, information, selected) = state
        .documents
        .describe(ready.document)
        .ok_or("Admitted source disappeared")?;
    let key = ready.path.to_str().ok_or("Source path is not Unicode")?;
    let restored = state.layout.snapshot().views.iter().find_map(|(id, v)| {
        (v.kind == "document" && v.restore_key.as_deref() == Some(key)).then_some(*id)
    });
    let id = restored.unwrap_or_else(ViewId::new);
    let mut candidate = state
        .layout
        .snapshot()
        .clone()
        .into_workspace()
        .map_err(|e| e.to_string())?;
    if restored.is_none() {
        let destination = content_destination(state);
        candidate
            .execute(Command::Open {
                id,
                view: View {
                    kind: "document".into(),
                    restore_key: Some(key.into()),
                },
                destination,
            })
            .map_err(|e| e.to_string())?;
    } else if candidate.snapshot().hidden.contains_key(&id) {
        candidate
            .execute(Command::Show {
                view: id,
                destination: content_destination(state),
            })
            .map_err(|e| e.to_string())?;
    }
    let placeholders: Vec<_> = candidate
        .snapshot()
        .views
        .iter()
        .filter_map(|(id, v)| {
            (v.kind == "preview" && candidate.snapshot().location(*id).is_some()).then_some(*id)
        })
        .collect();
    for placeholder in placeholders {
        candidate
            .execute(Command::Hide(placeholder))
            .map_err(|e| e.to_string())?;
    }
    candidate
        .execute(Command::Activate(id))
        .map_err(|e| e.to_string())?;
    if restored.is_none() {
        let scene = mount(world, state.stash, id, &title, &information)?;
        state.renderer.register(id, scene.root)?;
        state.panels.scenes.insert(id, scene);
    } else if let Some(root) = state.renderer.content(id) {
        let text: Vec<_> = world
            .query::<(Entity, &kedra_ui::ElementId)>()
            .iter(world)
            .filter_map(|(e, key)| {
                key.0
                    .starts_with(&format!("panel/{id:?}/"))
                    .then_some((e, key.0.clone()))
            })
            .collect();
        for (e, key) in text {
            if let Some(mut value) = world.get_mut::<Text>(e) {
                if key.ends_with("/heading") {
                    value.0 = title.clone();
                } else if key.ends_with("/information") {
                    value.0 = information.clone();
                }
            }
        }
        if !world.entities().contains(root) {
            return Err("Document content was retired".into());
        }
    }
    state.layout = candidate;
    state.titles.insert(id, title);
    state.documents.published(ready, id, selected);
    if let Some(scene) = state.panels.scenes.get_mut(&id)
        && let Some(source) = state.documents.scene_source()
        && let Some(opened) = state
            .documents
            .opened
            .values()
            .find(|opened| opened.view == id)
    {
        state.panels.scene_layer = state
            .panels
            .scene_layer
            .checked_add(1)
            .ok_or("Scene layer counter exhausted")?;
        scene.prepare(world, &opened.graph, source, state.panels.scene_layer);
    }
    state.preferences.dirty = true;
    redraw(world, state)
}
pub(super) fn mount(
    world: &mut World,
    parent: Entity,
    id: ViewId,
    title: &str,
    information: &str,
) -> Result<crate::scene_view::View, String> {
    let mut view = crate::scene_view::View::mount(world, parent, id, title)?;
    view.message(world, information, false);
    Ok(view)
}
