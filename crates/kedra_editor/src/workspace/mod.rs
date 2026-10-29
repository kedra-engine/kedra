//! Editor docking integration. Pure topology, retained projection and native
//! resource lifecycle remain separate owners, joined only at validated boundaries.
#[cfg(test)]
mod activation_tests;
mod asset_views;
mod document_views;
pub(crate) mod edit_scripts;
mod feedback;
#[cfg(debug_assertions)]
pub(crate) mod fixture;
mod headers;
#[cfg(test)]
mod hierarchy_tests;
mod history_context;
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
mod settlement;
mod source_history;

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
    app.add_plugins((
        kedra_script::LuauPlugin,
        kedra_inspector_luau::InspectorLuauPlugin,
    ));
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
    history_context: history_context::History,
}

pub(crate) fn install(app: &mut App) {
    source_history::install(app);
    scene_input::install(app);
    app.init_resource::<hosts::CloseRequests>()
        .init_resource::<input::CursorSample>()
        .add_observer(input::activate)
        .add_observer(popup::menu_event)
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
        history_context: default(),
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
        if let Some(panel) = state.panels.browsers.get_mut(&id) {
            match panel.dispatch(world, id, path, generation, action) {
                Err(error) => crate::views::fail(world, error),
                Ok(Some(path)) => {
                    if let Some(root) = world
                        .resource::<crate::model::ProjectManager>()
                        .project
                        .as_ref()
                        .map(|project| project.assets.clone())
                    {
                        state.documents.select_asset(world, root, path);
                    }
                }
                Ok(None) => (),
            }
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
pub(crate) fn select_context(
    world: &mut World,
    items: Vec<kedra_editor_context::SelectionItem>,
) -> Result<(), String> {
    if !world.contains_resource::<EditorWorkspace>() {
        return Err("Editor context is unavailable".into());
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        if settlement::drafts(world, &state) {
            return Err(
                "Accept or cancel pending property drafts before changing selection".into(),
            );
        }
        state.documents.replace_selection(items)
    })
}
pub(crate) fn drop_context(
    world: &mut World,
    view: ViewId,
    items: Vec<kedra_editor_context::SelectionItem>,
) {
    if !world
        .get_resource::<EditorWorkspace>()
        .is_some_and(|state| inspector_visible(state, view))
    {
        return;
    }
    if let Err(error) = select_context(world, items) {
        editor_script_feedback(world, &error);
    }
}

pub(crate) fn property_input(world: &mut World, event: kedra_ui::fields::TextFieldEvent) {
    let Some(view) = world
        .get::<crate::inspector::editing::SourceField>(event.entity)
        .map(|binding| binding.view)
    else {
        return;
    };
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        if !inspector_visible(&state, view) {
            return;
        }
        if let kedra_ui::fields::TextFieldOutcome::HistoryRequested { redo } = event.outcome {
            if state
                .panels
                .inspectors
                .get(&view)
                .is_some_and(|panel| panel.authorizes_input(world, event.entity))
                && !kedra_ui::fields::TextField::unsettled(world, event.entity)
                && world
                    .get::<kedra_ui::fields::FieldAccess>(event.entity)
                    .is_some_and(|access| !access.is_read_only())
                && let Err(error) = source_history::apply(world, &mut state, redo)
            {
                crate::views::fail(world, error);
            }
            return;
        }
        let state = &mut *state;
        let capture = history_context::capture(world, state);
        if let Some(panel) = state.panels.inspectors.get_mut(&view)
            && let Err(error) = panel.property_input(world, &mut state.documents, event)
        {
            crate::views::fail(world, error);
        }
        history_context::accepted(world, state, capture);
    });
}

pub(crate) fn refresh_inspector_presentations(world: &mut World) {
    if let Some(mut state) = world.get_resource_mut::<EditorWorkspace>() {
        for panel in state.panels.inspectors.values_mut() {
            panel.refresh_presentation();
        }
    }
}

pub(crate) fn property_control(world: &mut World, event: kedra_inspector::controls::ControlEvent) {
    let Some(view) = world
        .get::<crate::inspector::discrete::SourceControl>(event.entity)
        .map(|source| source.view)
    else {
        return;
    };
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        if !inspector_visible(&state, view) {
            return;
        }
        let state = &mut *state;
        let capture = history_context::capture(world, state);
        if let Some(panel) = state.panels.inspectors.get_mut(&view)
            && let Err(error) = panel.property_control(world, &mut state.documents, event)
        {
            crate::views::fail(world, error);
        }
        history_context::accepted(world, state, capture);
    });
}

pub(crate) fn property_reference(
    world: &mut World,
    event: kedra_inspector::resources::ReferenceEvent,
) {
    let Some(view) = world
        .get::<crate::inspector::discrete::SourceControl>(event.entity)
        .map(|source| source.view)
    else {
        return;
    };
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        if !inspector_visible(&state, view) {
            return;
        }
        let state = &mut *state;
        let capture = history_context::capture(world, state);
        if let Some(panel) = state.panels.inspectors.get_mut(&view)
            && let Err(error) = panel.property_reference(world, &mut state.documents, event)
        {
            crate::views::fail(world, error);
        }
        history_context::accepted(world, state, capture);
    });
}

pub(crate) fn browser_drag_path(
    world: &World,
    view: ViewId,
    folder: &std::path::Path,
    generation: u64,
    name: &std::ffi::OsStr,
) -> Option<String> {
    let root = &world
        .get_resource::<crate::model::ProjectManager>()?
        .project
        .as_ref()?
        .assets;
    let state = world.get_resource::<EditorWorkspace>()?;
    if !inspector_visible(state, view) {
        return None;
    }
    state
        .panels
        .browsers
        .get(&view)?
        .drag_path(root, folder, generation, name)
}

pub(crate) fn node_drag_available(
    world: &World,
    view: ViewId,
    document: kedra_scene::document::DocumentId,
) -> bool {
    world
        .get_resource::<EditorWorkspace>()
        .is_some_and(|state| {
            inspector_visible(state, view)
                && state.panels.hierarchies.contains_key(&view)
                && state.documents.active == Some(document)
        })
}

fn tick(world: &mut World) {
    static UPDATE: kedra_profiling::Marker =
        kedra_profiling::Marker::new("editor.workspace.update")
            .with_category(kedra_profiling::Category::Editor);
    let _scope = UPDATE.enter();
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        if (state.gesture.is_some()
            || state.pending.is_some()
            || state
                .surfaces
                .values()
                .any(|surface| surface.staged.is_some()))
            && let Some(mut demand) = world.get_resource_mut::<kedra_wakeup::WakeDemand>()
        {
            demand.active();
        }
        let result = (|| {
            hosts::poll(world, &mut state)?;
            if let Some(message) = state.documents.poll_save() {
                for panel in state.panels.inspectors.values() {
                    panel.end_edit_groups(world);
                }
                notice(world, &state, &message);
            }
            for (view, title) in state.documents.source_titles() {
                if state.titles.get(&view) != Some(&title) {
                    state.titles.insert(view, title);
                    state.rendered = None;
                }
            }
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
    static PROJECTION: kedra_profiling::Marker =
        kedra_profiling::Marker::new("editor.workspace.project")
            .with_category(kedra_profiling::Category::Editor);
    let _scope = PROJECTION.enter();
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
        if state
            .documents
            .opened
            .values()
            .any(|opened| opened.view == view)
        {
            state.documents.activate_view(view);
        } else {
            state.documents.active = None;
        }
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
pub(crate) fn editor_script_feedback(world: &mut World, message: &str) {
    if world.contains_resource::<EditorWorkspace>() {
        world.resource_scope(|world, state: Mut<EditorWorkspace>| notice(world, &state, message));
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
