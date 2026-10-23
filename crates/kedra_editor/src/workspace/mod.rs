//! Editor docking integration. Pure topology, retained projection and native
//! resource lifecycle remain separate owners, joined only at validated boundaries.
#[cfg(test)]
mod activation_tests;
mod asset_views;
mod document_views;
mod feedback;
#[cfg(debug_assertions)]
pub(crate) mod fixture;
mod headers;
#[cfg(test)]
mod hierarchy_tests;
mod hit;
mod hosts;
mod input;
mod panels;
mod popup;
mod preset;
#[cfg(test)]
mod preset_tests;
mod scene_input;
mod scene_menu;
mod scene_tools;
mod scrolling;

pub(crate) fn bind_scene_controls(
    world: &mut World,
    view: &UiView,
    id: ViewId,
) -> Result<(), String> {
    use input::Action;
    for (name, action) in [
        (
            "projection",
            Action::SceneCommand(id, crate::scene_view::camera::Command::Projection),
        ),
        (
            "lighting",
            Action::SceneMenu(id, scene_menu::Kind::Lighting),
        ),
        ("grid", Action::SceneGrid(id)),
        ("views", Action::SceneMenu(id, scene_menu::Kind::Views)),
        (
            "select-menu",
            Action::SceneMenu(id, scene_menu::Kind::Selection),
        ),
        ("select-tool", Action::SceneFocus(id)),
        ("overlays", Action::SceneOverlays(id)),
        (
            "frame",
            Action::SceneCommand(id, crate::scene_view::camera::Command::FrameSelection),
        ),
    ] {
        let entity = view.element(name).map_err(|error| error.to_string())?;
        world.entity_mut(entity).insert(action);
        if let Some(mut node) = world.get_mut::<bevy::a11y::AccessibilityNode>(entity) {
            node.set_label(match name {
                "projection" => "Toggle perspective or orthographic projection",
                "lighting" => "Choose scene or editor lighting",
                "grid" => "Toggle grid and axes",
                "select-menu" => "Selection commands",
                "select-tool" => "Select objects",
                "overlays" => "Toggle selection overlays",
                "frame" => "Frame selection",
                _ => "Framing and standard views",
            });
        }
    }
    scene_tools::bind(world, view, id)
}
impl EditorWorkspace {
    pub(crate) fn update_ready(&self) -> bool {
        self.pending.is_none() && self.gesture.is_none() && self.documents.update_clean()
    }
    #[cfg(debug_assertions)]
    pub(crate) fn navigation_input(&self, world: &World) -> serde_json::Value {
        scene_input::diagnostics(world, self)
    }
    #[cfg(debug_assertions)]
    pub(crate) fn scene_state(&self, world: &World) -> serde_json::Value {
        serde_json::json!(
            self.panels
                .scenes
                .iter()
                .map(|(id, scene)| (id, scene.snapshot(world)))
                .collect::<BTreeMap<_, _>>()
        )
    }
}
mod storage;
pub(crate) fn save_for_update(world: &mut World) -> Result<(), String> {
    if !world.contains_resource::<EditorWorkspace>() {
        return Err("Editor workspace is unavailable".into());
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        storage::save_checked(world, &mut state)
    })
}
#[cfg(test)]
mod test_support;

#[cfg(test)]
pub(crate) fn browser_test_app() -> Result<App, Box<dyn std::error::Error>> {
    let (mut app, _, _) = test_support::setup()?;
    test_support::sources(app.world_mut())?;
    Ok(app)
}
#[cfg(test)]
mod workspace_tests;
use bevy::prelude::*;
use kedra_docking::{
    command::{Command, Destination},
    model::{HostId, NodeId, ViewId, Workspace},
};
use kedra_ui::authored::view::UiView;
use kedra_ui::docking::{Projection, Skin};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy)]
struct Surface {
    window: Entity,
    root: Entity,
    owned: Option<kedra_desktop::auxiliary::Surface>,
    // Restored hosts keep content in the live stash until native acknowledgment.
    staged: Option<std::time::Duration>,
}

#[derive(Resource)]
pub(crate) struct EditorWorkspace {
    pub(crate) layout: Workspace,
    pub(crate) host: HostId,
    pub(crate) window: Entity,
    // Diagnostic fixture identity and regression parent, not a surface owner.
    #[cfg(any(debug_assertions, test))]
    pub(crate) root: Entity,
    surfaces: BTreeMap<HostId, Surface>,
    renderer: Projection,
    skin: Option<Arc<Skin>>,
    stash: Entity,
    status: Option<Entity>,
    titles: BTreeMap<ViewId, String>,
    rendered: Option<u64>,
    gesture: Option<input::Gesture>,
    pending: Option<hosts::Pending>,
    menu: Vec<Entity>,
    preferences: storage::Preferences,
    measurements: Option<input::Measurement>,
    preferred: BTreeMap<NodeId, f32>,
    last_divider: Option<(NodeId, std::time::Duration)>,
    panels: panels::State,
    pub(crate) documents: crate::documents::Documents,
    focused_group: Option<NodeId>,
    content_view: Option<ViewId>,
    content_actives: BTreeMap<NodeId, ViewId>,
    feedback: Option<feedback::Feedback>,
}

pub(crate) fn install(app: &mut App) {
    scene_input::install(app);
    app.init_resource::<hosts::CloseRequests>()
        .init_resource::<input::CursorSample>()
        .add_observer(input::activate)
        .add_observer(popup::menu_event)
        .add_systems(
            PostUpdate,
            popup::constrain.before(bevy::ui::UiSystems::Layout),
        )
        .add_systems(
            Update,
            (hosts::requests, input::cursor, tick, scrolling::wheel)
                .chain()
                .after(crate::views::hydrate),
        )
        .add_systems(Last, storage::save_on_exit);
}

pub(crate) fn bind(world: &mut World, view: &UiView) -> Result<(), String> {
    let preset = preset::Preset::load(world)?;
    let window = world
        .resource::<kedra_desktop::DesktopSurface>()
        .window
        .filter(|window| world.get::<Window>(*window).is_some())
        .ok_or("Docking workspace has no live editor window")?;
    bind_root(world, window, view.root)?;
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        state.panels.preset = Some(preset);
        let shell = crate::views::mount_asset(
            world,
            view.root,
            "editor://prefabs/dock-workspace.prefab.ron",
            "workspace",
        )?;
        state.stash = shell.element("stash").map_err(|e| e.to_string())?;
        let root = shell.element("host").map_err(|e| e.to_string())?;
        let host = state.host;
        state
            .surfaces
            .get_mut(&host)
            .ok_or("Main host binding is absent")?
            .root = root;
        state.status = Some(shell.element("status").map_err(|e| e.to_string())?);
        for (key, action) in [
            ("panels", input::Action::Panels),
            ("reset", input::Action::Reset),
        ] {
            if let Ok(entity) = shell.element(key) {
                world.entity_mut(entity).insert(action);
            }
        }
        state.skin = Some(load_skin(world)?);
        state.preferences = storage::Preferences::load(world);
        if let Some(restored) = state.preferences.restored.take() {
            state.host = restored.main;
            let surface = state
                .surfaces
                .values()
                .next()
                .copied()
                .ok_or("Main surface is absent")?;
            state.surfaces.clear();
            let host = state.host;
            state.surfaces.insert(host, surface);
            state.layout = panels::restore(
                state.host,
                restored.layout,
                state
                    .panels
                    .preset
                    .as_ref()
                    .ok_or("Editor preset missing")?,
            )?;
            hosts::restore(world, &mut state, &restored.placements)?;
        } else if state.preferences.error.is_none() {
            panels::preset(&mut state)?;
        }
        panels::mount(world, &mut state)?;
        redraw(world, &mut state)
    })
}
pub(crate) fn bind_header(world: &mut World, view: &UiView) {
    for (key, action) in [
        ("editor.panels", input::Action::Panels),
        ("editor.project-menu", input::Action::Panels),
        ("editor.project-chevron", input::Action::Panels),
        ("editor.reset", input::Action::Reset),
    ] {
        if let Ok(entity) = view.element(key) {
            world.entity_mut(entity).insert(action);
        }
    }
    // Observation identities belong to the command, not its previous toolbar row.
    let panels = if cfg!(target_os = "macos") {
        "editor.project-menu"
    } else {
        "editor.panels"
    };
    for (key, identity) in [
        (panels, "workspace/panels"),
        ("editor.reset", "workspace/reset"),
    ] {
        if let Ok(entity) = view.element(key) {
            world
                .entity_mut(entity)
                .insert(kedra_ui::ElementId::new(identity));
        }
    }
}
fn load_skin(world: &World) -> Result<Arc<Skin>, String> {
    Ok(Arc::new(Skin {
        group: crate::views::compiled(world, "editor://prefabs/dock-group.prefab.ron")?,
        tab: crate::views::compiled(world, "editor://prefabs/dock-tab.prefab.ron")?,
        split: crate::views::compiled(world, "editor://prefabs/dock-split.prefab.ron")?,
    }))
}

fn bind_root(world: &mut World, window: Entity, root: Entity) -> Result<(), String> {
    if world.contains_resource::<EditorWorkspace>() {
        return Err("Docking workspace is already bound; retire it before replacement".into());
    }
    if world.get::<Window>(window).is_none() || !world.entities().contains(root) {
        return Err("Docking workspace cannot bind retired native resources".into());
    }
    let host = HostId::new();
    world.insert_resource(EditorWorkspace {
        layout: Workspace::new(host),
        host,
        window,
        #[cfg(any(debug_assertions, test))]
        root,
        surfaces: BTreeMap::from([(
            host,
            Surface {
                window,
                root,
                owned: None,
                staged: None,
            },
        )]),
        renderer: Projection::default(),
        skin: None,
        stash: root,
        status: None,
        titles: BTreeMap::new(),
        rendered: None,
        gesture: None,
        pending: None,
        menu: Vec::new(),
        preferences: storage::Preferences::default(),
        measurements: None,
        preferred: BTreeMap::new(),
        last_divider: None,
        panels: panels::State::default(),
        documents: crate::documents::Documents::default(),
        focused_group: None,
        content_view: None,
        content_actives: BTreeMap::new(),
        feedback: None,
    });
    Ok(())
}

pub(crate) fn retire(world: &mut World) {
    scene_input::cancel(world);
    // The startup surface lifecycle owns the root/window. Removing the logical
    // binding does not despawn feature content or claim document-close approval.
    if let Some(mut state) = world.remove_resource::<EditorWorkspace>() {
        for scene in state.panels.scenes.values_mut() {
            scene.retire(world);
        }
        storage::save(world, &mut state);
        hosts::retire(world, &mut state);
    }
}

pub(crate) fn browser_action(
    world: &mut World,
    id: ViewId,
    path: std::path::PathBuf,
    generation: u64,
    action: crate::asset_browser::Action,
) {
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        // A retired/hidden view or stale directory binding cannot admit an effect.
        if state.layout.snapshot().location(id).is_none() {
            return;
        }
        if let Some(panel) = state.panels.browsers.get_mut(&id)
            && let Err(error) = panel.dispatch(world, id, path, generation, action)
        {
            crate::views::fail(world, error);
        }
    });
}

pub(crate) fn hierarchy_action(world: &mut World, binding: crate::hierarchy::Binding) {
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        if state.layout.snapshot().location(binding.view).is_none() {
            return;
        }
        let state = &mut *state;
        if let Some(panel) = state.panels.hierarchies.get_mut(&binding.view)
            && let Err(error) = panel.dispatch(world, binding, &mut state.documents)
        {
            crate::views::fail(world, error);
        }
    });
}

pub(crate) fn inspector_action(world: &mut World, binding: crate::inspector::Binding) {
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        if !inspector_visible(&state, binding.view) {
            return;
        }
        if let Some(panel) = state.panels.inspectors.get_mut(&binding.view)
            && let Err(error) = panel.action(world, binding.action)
        {
            crate::views::fail(world, error);
        }
    });
}
pub(crate) fn inspector_disclosure(
    world: &mut World,
    disclosure: kedra_inspector::view::Disclosure,
) {
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        let view = state
            .panels
            .inspectors
            .iter()
            .find_map(|(id, p)| p.owns(&disclosure).then_some(*id));
        if let Some(view) = view.filter(|view| inspector_visible(&state, *view))
            && let Some(panel) = state.panels.inspectors.get_mut(&view)
            && let Err(error) =
                panel.action(world, crate::inspector::Action::Disclosure(disclosure))
        {
            crate::views::fail(world, error);
        }
    });
}
fn inspector_visible(state: &EditorWorkspace, view: ViewId) -> bool {
    let snapshot = state.layout.snapshot();
    snapshot.location(view).is_some_and(|(group, _)| {
        matches!(snapshot.nodes.get(&group), Some(kedra_docking::model::Node::Group { active, .. }) if *active == view)
            && snapshot.host_of(group).and_then(|host| snapshot.hosts.get(&host)).is_some_and(|host| host.maximized.is_none_or(|max| max == group))
    })
}

fn tick(world: &mut World) {
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        let result = (|| {
            hosts::poll(world, &mut state)?;
            panels::poll(world, &mut state);
            input::tick(world, &mut state)?;
            if state.rendered != Some(state.layout.revision()) {
                redraw(world, &mut state)?;
            }
            if state.gesture.is_none() {
                state.renderer.constrain(world, state.layout.snapshot());
            }
            for (group, node) in &state.layout.snapshot().nodes {
                if !matches!(node, kedra_docking::model::Node::Group { .. }) {
                    continue;
                }
                let focused = state.focused_group == Some(*group)
                    && state
                        .layout
                        .snapshot()
                        .host_of(*group)
                        .and_then(|host| state.surfaces.get(&host))
                        .and_then(|surface| world.get::<Window>(surface.window))
                        .is_some_and(|window| window.focused);
                if let Some(root) = state.renderer.group(*group) {
                    kedra_ui::docking::class(world, root, "focused", focused);
                }
            }
            storage::poll(world, &mut state);
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            input::cancel(world, &mut state);
            notice(world, &state, &error);
        }
    });
}

fn redraw(world: &mut World, state: &mut EditorWorkspace) -> Result<(), String> {
    document_views::reconcile_activation(state);
    for (id, node) in &state.layout.snapshot().nodes {
        if let kedra_docking::model::Node::Split { fraction, .. } = node {
            state.preferred.entry(*id).or_insert(*fraction);
        }
    }
    let Some(skin) = &state.skin else {
        return Ok(());
    };
    let hosts = state
        .surfaces
        .iter()
        .map(|(id, host)| {
            (
                *id,
                if host.staged.is_some() {
                    state.stash
                } else {
                    host.root
                },
            )
        })
        .collect();
    let destinations = headers::committed(world, state)?;
    state
        .renderer
        .set_header_destinations(world, destinations)?;
    state.renderer.sync(
        world,
        state.layout.snapshot(),
        &hosts,
        state.stash,
        skin,
        &state.titles,
        "dock",
        4.,
    )?;
    let groups: Vec<_> = world
        .query::<(Entity, &kedra_ui::docking::GroupTarget)>()
        .iter(world)
        .map(|(entity, target)| {
            (
                entity,
                panels::hierarchy_only(state.layout.snapshot(), target.group),
            )
        })
        .collect();
    for (entity, hierarchy) in groups {
        kedra_ui::docking::class(world, entity, "hierarchy-group", hierarchy);
    }
    state.rendered = Some(state.layout.revision());
    Ok(())
}
fn apply(world: &mut World, state: &mut EditorWorkspace, command: Command) -> Result<(), String> {
    let activated = if let Command::Activate(view) = &command {
        Some(*view)
    } else {
        None
    };
    state.layout.execute(command).map_err(|e| e.to_string())?;
    if let Some(view) = activated {
        activate_content(state, view);
    }
    redraw(world, state)?;
    state.preferences.dirty = true;
    Ok(())
}
fn activate_content(state: &mut EditorWorkspace, view: ViewId) {
    let content = state
        .layout
        .snapshot()
        .views
        .get(&view)
        .is_some_and(|view| matches!(view.kind.as_str(), "document" | "asset-preview" | "preview"));
    if content {
        state.content_view = Some(view);
        state.documents.active = None;
        state.documents.activate_view(view);
    }
}
fn notice(world: &mut World, state: &EditorWorkspace, message: &str) {
    warn!("Docking: {message}");
    if let Some(mut text) = state
        .status
        .and_then(|entity| world.get_mut::<Text>(entity))
    {
        text.0 = message.into();
    }
}
fn destination(state: &EditorWorkspace) -> Destination {
    state
        .layout
        .snapshot()
        .nodes
        .iter()
        .find_map(|(group, node)| {
            (matches!(node, kedra_docking::model::Node::Group { .. })
                && state.layout.snapshot().host_of(*group) == Some(state.host))
            .then_some(Destination::Tabs {
                group: *group,
                index: 0,
            })
        })
        .unwrap_or(Destination::EmptyHost(state.host))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kedra_docking::{
        command::{Command, Destination},
        model::{View, ViewId},
    };

    #[test]
    fn native_binding_owns_a_real_model_and_rejects_double_binding_and_retired_roots() {
        let mut world = World::new();
        let window = world.spawn(Window::default()).id();
        let root = world.spawn_empty().id();
        bind_root(&mut world, window, root).unwrap_or_else(|e| panic!("{e}"));
        let mut workspace = world.resource_mut::<EditorWorkspace>();
        let host = workspace.host;
        let view = ViewId::new();
        workspace
            .layout
            .execute(Command::Open {
                id: view,
                view: View {
                    kind: "test.editable".into(),
                    restore_key: Some("document:shared".into()),
                },
                destination: Destination::EmptyHost(host),
            })
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(
            world
                .resource::<EditorWorkspace>()
                .layout
                .snapshot()
                .location(view)
                .is_some()
        );
        assert!(bind_root(&mut world, window, root).is_err());
        retire(&mut world);
        assert!(world.entities().contains(root));
        assert!(world.get::<Window>(window).is_some());
        world.despawn(root);
        assert!(bind_root(&mut world, window, root).is_err());
        let root = world.spawn_empty().id();
        bind_root(&mut world, window, root).unwrap_or_else(|e| panic!("{e}"));
        let workspace = world.resource::<EditorWorkspace>();
        assert!(workspace.layout.snapshot().views.is_empty());
        assert_eq!(workspace.layout.revision(), 0);
    }
}
