//! Window-aware gestures, candidate measurement and native control bindings.
#[path = "dock_menu.rs"]
mod menu;
use super::*;
use bevy::{input_focus::tab_navigation::TabIndex, ui::Node as UiNode, ui_widgets::Activate};
use kedra_docking::{
    command::Selection,
    model::{Axis, Node},
    session::Session,
};
use kedra_ui::docking::{Control, Divider, Geometry, GroupTarget, TabLabel, class, geometry, rect};
use menu::menu;

#[derive(Component, Clone, Copy)]
pub(super) enum Action {
    Panels,
    Reset,
    Float(NodeId),
    FloatView(ViewId),
    Maximize(NodeId),
    Redock(NodeId),
    Hide(NodeId),
    Show(ViewId),
    Select(ViewId),
    BrowserOpen(ViewId),
    BrowserRefresh(ViewId),
    HierarchySearch(ViewId),
    HierarchyReveal(ViewId),
    HierarchyDetails(ViewId),
    InspectorSearch(ViewId),
    InspectorLock(ViewId),
    InspectorRaw(ViewId),
    InspectorDetails(ViewId),
    SceneCommand(ViewId, crate::scene_view::camera::Command),
    SceneMenu(ViewId, super::scene_menu::Kind),
    SceneProjection(ViewId, bool),
    SceneGrid(ViewId),
    SceneLight(ViewId, bool),
    SceneClear(ViewId),
    SceneOverlays(ViewId),
    SceneFocus(ViewId),
    SceneAxis(ViewId, crate::scene_view::orientation::Axis),
}
#[derive(Resource, Default)]
pub(super) struct CursorSample {
    pub(super) window: Option<Entity>,
    pub(super) position: Vec2,
}
pub(super) fn cursor(
    mut events: MessageReader<bevy::window::CursorMoved>,
    mut sample: ResMut<CursorSample>,
) {
    for event in events.read() {
        sample.window = Some(event.window);
        sample.position = event.position;
    }
}

pub(super) enum Gesture {
    Pending {
        selection: Selection,
        view: ViewId,
        host: HostId,
        start: Vec2,
    },
    Drag {
        selection: Selection,
        host: HostId,
        session: Session,
        destination: Option<Destination>,
    },
    Resize {
        host: HostId,
        split: NodeId,
        session: Session,
        axis: Axis,
        bounds: Rect,
    },
}
pub(super) struct Measurement {
    projection: Projection,
    roots: BTreeMap<HostId, Entity>,
    stash: Entity,
    overlay: Entity,
    snapshot: kedra_docking::snapshot::Snapshot,
    target: NodeId,
    ready: u8,
    bounds: Option<(Rect, Rect)>,
}

pub(super) fn activate(
    event: On<Activate>,
    controls: Query<&Control>,
    actions: Query<&Action>,
    drags: Query<(), With<super::scene_tools::DragControl>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut commands: Commands,
) {
    if drags.contains(event.entity)
        && !keys
            .as_ref()
            .is_some_and(|keys| keys.any_just_pressed([KeyCode::Space, KeyCode::Enter]))
    {
        return;
    }
    let control = controls.get(event.entity).ok().copied();
    let action = actions.get(event.entity).ok().copied();
    if control.is_none() && action.is_none() {
        return;
    }
    let anchor = event.entity;
    commands.queue(move |world: &mut World| dispatch(world, control, action, anchor));
}
fn dispatch(world: &mut World, control: Option<Control>, action: Option<Action>, anchor: Entity) {
    if !world.contains_resource::<EditorWorkspace>() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
        let result = (|| {
            if let Some(control) = control {
                match control {
                    Control::Tab(view) => {
                        state.focused_group = state
                            .layout
                            .snapshot()
                            .location(view)
                            .map(|(group, _)| group);
                        if state.gesture.is_none() {
                            apply(world, &mut state, Command::Activate(view))?;
                        }
                    }
                    Control::Close(view) => {
                        cancel(world, &mut state);
                        apply(world, &mut state, Command::Hide(view))?;
                    }
                    Control::Menu(group) => menu(world, &mut state, Some(group), anchor)?,
                    Control::ResetSplit(split) => {
                        let keys = world.resource::<ButtonInput<KeyCode>>();
                        if state.gesture.is_none()
                            && (keys.any_pressed([KeyCode::Space, KeyCode::Enter])
                                || keys.any_just_released([KeyCode::Space, KeyCode::Enter]))
                        {
                            let fraction = state
                                .preferred
                                .get(&split)
                                .copied()
                                .ok_or("Preferred split ratio is absent")?;
                            apply(world, &mut state, Command::Resize { split, fraction })?;
                        }
                    }
                }
            }
            if let Some(action) = action {
                dismiss(world, &mut state);
                match action {
                    Action::SceneCommand(view, command) => {
                        super::scene_input::cancel(world);
                        if let Some(camera) = state
                            .panels
                            .scenes
                            .get(&view)
                            .and_then(|scene| scene.navigation(world))
                        {
                            crate::scene_view::camera::command(world, camera, command)?;
                        }
                    }
                    Action::SceneMenu(view, kind) => {
                        super::scene_menu::open(world, &mut state, view, anchor, kind)?;
                    }
                    Action::SceneProjection(view, orthographic) => {
                        super::scene_tools::projection(world, &state, view, orthographic)?;
                    }
                    Action::SceneGrid(view) => {
                        if let Some(scene) = state.panels.scenes.get_mut(&view) {
                            scene.grid(world);
                        }
                    }
                    Action::SceneClear(view) => {
                        state.documents.clear_selection(view);
                    }
                    Action::SceneOverlays(view) => {
                        if let Some(scene) = state.panels.scenes.get_mut(&view) {
                            scene.toggle_overlays(world);
                        }
                    }
                    Action::SceneFocus(view) => {
                        if let Some(scene) = state.panels.scenes.get(&view) {
                            scene.focus_canvas(world);
                        }
                    }
                    Action::SceneAxis(view, axis) => {
                        super::scene_tools::align(world, &state, view, axis)?;
                    }
                    Action::SceneLight(view, enabled) => {
                        if let Some(scene) = state.panels.scenes.get_mut(&view) {
                            scene.light(world, enabled);
                        }
                    }
                    Action::Panels => menu(world, &mut state, None, anchor)?,
                    Action::Reset => {
                        cancel(world, &mut state);
                        panels::reset(world, &mut state)?;
                    }
                    Action::Float(group) => {
                        cancel(world, &mut state);
                        hosts::float(world, &mut state, Selection::Group(group))?;
                    }
                    Action::FloatView(view) => {
                        cancel(world, &mut state);
                        hosts::float(world, &mut state, Selection::View(view))?;
                    }
                    Action::Maximize(group) => maximize(world, &mut state, group)?,
                    Action::Redock(group) => {
                        cancel(world, &mut state);
                        let target = destination(&state);
                        apply(
                            world,
                            &mut state,
                            Command::Move {
                                selection: Selection::Group(group),
                                destination: target,
                            },
                        )?;
                    }
                    Action::Hide(group) => {
                        let tabs = match state.layout.snapshot().nodes.get(&group) {
                            Some(Node::Group { tabs, .. }) => tabs.clone(),
                            _ => return Err("Group was retired".into()),
                        };
                        for view in tabs {
                            apply(world, &mut state, Command::Hide(view))?;
                        }
                    }
                    Action::Show(view) => {
                        let target = bookmark_destination(&state, view);
                        apply(
                            world,
                            &mut state,
                            Command::Show {
                                view,
                                destination: target,
                            },
                        )?;
                    }
                    Action::Select(view) => {
                        apply(world, &mut state, Command::Activate(view))?;
                    }
                    Action::InspectorSearch(view)
                    | Action::InspectorLock(view)
                    | Action::InspectorRaw(view)
                    | Action::InspectorDetails(view) => {
                        if super::inspector_visible(&state, view)
                            && let Some(panel) = state.panels.inspectors.get_mut(&view)
                        {
                            panel.action(
                                world,
                                match action {
                                    Action::InspectorSearch(_) => crate::inspector::Action::Search,
                                    Action::InspectorLock(_) => crate::inspector::Action::Lock,
                                    Action::InspectorRaw(_) => crate::inspector::Action::Raw,
                                    _ => crate::inspector::Action::Details,
                                },
                            )?;
                        }
                    }
                    Action::HierarchySearch(view)
                    | Action::HierarchyReveal(view)
                    | Action::HierarchyDetails(view) => {
                        if state.layout.snapshot().location(view).is_some() {
                            let state = &mut *state;
                            if let Some(panel) = state.panels.hierarchies.get_mut(&view) {
                                panel.dispatch(
                                    world,
                                    crate::hierarchy::Binding {
                                        view,
                                        document: state.documents.active,
                                        action: match action {
                                            Action::HierarchySearch(_) => {
                                                crate::hierarchy::Action::FocusSearch
                                            }
                                            Action::HierarchyReveal(_) => {
                                                crate::hierarchy::Action::Reveal
                                            }
                                            _ => crate::hierarchy::Action::Details,
                                        },
                                    },
                                    &mut state.documents,
                                )?;
                            }
                        }
                    }
                    Action::BrowserOpen(view) | Action::BrowserRefresh(view) => {
                        if state.layout.snapshot().location(view).is_some()
                            && let Some(browser) = state.panels.browsers.get_mut(&view)
                        {
                            browser.action(
                                world,
                                view,
                                if matches!(action, Action::BrowserOpen(_)) {
                                    crate::asset_browser::Action::Open
                                } else {
                                    crate::asset_browser::Action::Refresh
                                },
                            )?;
                        }
                    }
                }
            }
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            notice(world, &state, &error);
        }
    });
}
pub(super) fn bookmark_destination(state: &EditorWorkspace, view: ViewId) -> Destination {
    if let Some(bookmark) = state.layout.snapshot().hidden.get(&view)
        && let Some(Node::Group { tabs, .. }) = state.layout.snapshot().nodes.get(&bookmark.group)
    {
        return Destination::Tabs {
            group: bookmark.group,
            index: bookmark.index.min(tabs.len()),
        };
    }
    destination(state)
}
pub(super) fn dismiss(world: &mut World, state: &mut EditorWorkspace) {
    use bevy::input_focus::IsFocused;
    let owner = state
        .menu
        .iter()
        .find(|root| world.is_focus_within(**root))
        .and_then(|root| world.get::<ChildOf>(*root).map(ChildOf::parent));
    if let Some(owner) = owner
        && let Some(mut focus) = world.get_resource_mut::<bevy::input_focus::InputFocus>()
    {
        focus.set(owner, bevy::input_focus::FocusCause::Navigated);
    }
    for root in state.menu.drain(..) {
        world.despawn(root);
    }
}

fn maximize(world: &mut World, state: &mut EditorWorkspace, group: NodeId) -> Result<(), String> {
    cancel(world, state);
    let host = state
        .layout
        .snapshot()
        .host_of(group)
        .ok_or("Group was retired")?;
    let current = state.layout.snapshot().hosts[&host].maximized;
    apply(
        world,
        state,
        Command::Maximize {
            host,
            group: if current == Some(group) {
                None
            } else {
                Some(group)
            },
        },
    )
}

pub(super) fn tick(world: &mut World, state: &mut EditorWorkspace) -> Result<(), String> {
    state
        .menu
        .retain(|entity| world.entities().contains(*entity));
    keyboard(world, state)?;
    let keys = world.resource::<ButtonInput<KeyCode>>();
    if keys.just_pressed(KeyCode::Escape) {
        cancel(world, state);
        dismiss(world, state);
        return Ok(());
    }
    if state.pending.is_some() {
        return Ok(());
    }
    let pressed = world
        .resource::<ButtonInput<MouseButton>>()
        .just_pressed(MouseButton::Left);
    let released = world
        .resource::<ButtonInput<MouseButton>>()
        .just_released(MouseButton::Left);
    let held = world
        .resource::<ButtonInput<MouseButton>>()
        .pressed(MouseButton::Left);
    if state.gesture.is_some()
        && !state.surfaces.values().any(|surface| {
            world
                .get::<Window>(surface.window)
                .is_some_and(|w| w.focused)
        })
    {
        debug!("Cancelling dock gesture because no live native host remains focused");
        cancel(world, state);
        return Ok(());
    }
    if pressed && state.gesture.is_none() {
        if !state.menu.is_empty() {
            return Ok(());
        }
        if let Some((host, point)) = pointer(world, state) {
            state.focused_group = world
                .query::<(Entity, &GroupTarget)>()
                .iter(world)
                .find_map(|(entity, target)| {
                    (target.host == host
                        && state.renderer.group(target.group) == Some(entity)
                        && super::hit::group(world, state, target.host, target.group, point))
                    .then_some(target.group)
                })
                .or(state.focused_group);
            let shift = world
                .resource::<ButtonInput<KeyCode>>()
                .any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
            let label =
                world
                    .query::<(Entity, &TabLabel)>()
                    .iter(world)
                    .find_map(|(entity, label)| {
                        (label.host == host
                            && super::hit::label(world, state, entity, label, point))
                        .then_some(*label)
                    });
            if let Some(label) = label {
                state.gesture = Some(Gesture::Pending {
                    selection: if shift {
                        Selection::Group(label.group)
                    } else {
                        Selection::View(label.view)
                    },
                    view: label.view,
                    host,
                    start: point,
                });
            } else if !super::hit::scrollbar(world, state, host, point) {
                let divider = world.query::<(Entity, &Divider)>().iter(world).find_map(
                    |(entity, divider)| {
                        (divider.host == host
                            && super::hit::divider(world, state, entity, divider, point))
                        .then_some(*divider)
                    },
                );
                if let Some(divider) = divider
                    && let Some(Node::Split { axis, .. }) =
                        state.layout.snapshot().nodes.get(&divider.split)
                {
                    let now = world.resource::<Time<Real>>().elapsed();
                    let double = state.last_divider.is_some_and(|(split, time)| {
                        split == divider.split
                            && now.saturating_sub(time) < std::time::Duration::from_millis(400)
                    });
                    state.last_divider = Some((divider.split, now));
                    if double {
                        let fraction = state
                            .preferred
                            .get(&divider.split)
                            .copied()
                            .ok_or("Preferred split ratio is absent")?;
                        apply(
                            world,
                            state,
                            Command::Resize {
                                split: divider.split,
                                fraction,
                            },
                        )?;
                        return Ok(());
                    }
                    let bounds = state
                        .renderer
                        .group(divider.split)
                        .and_then(|e| rect(world, e))
                        .ok_or("Divider layout is unavailable")?;
                    state.gesture = Some(Gesture::Resize {
                        host,
                        split: divider.split,
                        axis: *axis,
                        bounds,
                        session: Session::new(&state.layout),
                    });
                }
            }
        }
    }
    let Some(gesture) = state.gesture.take() else {
        return Ok(());
    };
    match gesture {
        Gesture::Pending {
            selection,
            view,
            host,
            start,
        } => {
            if released {
                apply(world, state, Command::Activate(view))?;
            } else if let Some((_, point)) = pointer(world, state)
                && point.distance(start) > 5.
            {
                state.gesture = Some(Gesture::Drag {
                    selection,
                    host,
                    session: Session::new(&state.layout),
                    destination: None,
                });
            } else if held {
                state.gesture = Some(Gesture::Pending {
                    selection,
                    view,
                    host,
                    start,
                });
            }
        }
        Gesture::Drag {
            selection,
            host,
            mut session,
            destination: previous,
        } => {
            let feedback = if let Some((target_host, point)) = pointer(world, state) {
                Some(super::feedback::update(
                    world,
                    state,
                    selection,
                    target_host,
                    point,
                )?)
            } else {
                super::feedback::clear(world, state);
                None
            };
            let target = feedback.as_ref().and_then(|target| target.destination);
            let insertion = feedback.as_ref().is_some_and(|target| target.insertion);
            if target != previous
                || (!insertion && target.is_some() && state.measurements.is_none())
            {
                clear_measurement(world, state);
                if let Some(destination) = target {
                    match session.update(
                        &state.layout,
                        Command::Move {
                            selection,
                            destination,
                        },
                    ) {
                        Ok(candidate) => {
                            if !insertion {
                                measurement(world, state, candidate.snapshot(), selection)?;
                            }
                        }
                        Err(error) => {
                            debug!(%error, ?selection, ?target, "Dock candidate rejected");
                            if held && !released {
                                state.gesture = Some(Gesture::Drag {
                                    selection,
                                    host,
                                    session,
                                    destination: target,
                                });
                            }
                            if released {
                                super::feedback::clear(world, state);
                            }
                            return Ok(());
                        }
                    }
                }
            }
            update_measurement(world, state)?;
            if released {
                let ready = insertion || state.measurements.as_ref().is_some_and(|m| m.ready >= 3);
                debug!(?selection, ?target, insertion, ready, pointer = ?pointer(world, state), revision = state.layout.revision(), "Dock drop admission");
                clear_measurement(world, state);
                super::feedback::clear(world, state);
                if let Some(destination) = target
                    && ready
                    && super::feedback::admissible(world, state, selection, destination)
                {
                    let active = selected_view(state, selection);
                    let changed = session
                        .finish(&mut state.layout)
                        .map_err(|e| e.to_string())?;
                    debug!(
                        changed,
                        revision = state.layout.revision(),
                        "Dock drop completed"
                    );
                    state.focused_group = active.and_then(|view| {
                        state
                            .layout
                            .snapshot()
                            .location(view)
                            .map(|(group, _)| group)
                    });
                    state.preferences.dirty = true;
                    redraw(world, state)?;
                } else if target.is_none() && pointer(world, state).is_none() {
                    let coordinates = state.surfaces.get(&host).is_some_and(|surface| {
                        bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
                            windows
                                .get_window(surface.window)
                                .is_some_and(|window| window.inner_position().is_ok())
                        })
                    });
                    if !coordinates && state.surfaces.len() > 1 {
                        return Err("This backend cannot locate cross-window drag targets. Use the explicit Float/Redock group menu; the layout was not changed.".into());
                    }
                    hosts::float(world, state, selection)?;
                }
            } else if held {
                state.gesture = Some(Gesture::Drag {
                    selection,
                    host,
                    session,
                    destination: target,
                });
            }
        }
        Gesture::Resize {
            host,
            split,
            mut session,
            axis,
            bounds,
        } => {
            if let Some((pointer_host, point)) = pointer(world, state)
                && pointer_host == host
            {
                let (position, length) = if axis == Axis::Horizontal {
                    (point.x - bounds.min.x, bounds.width())
                } else {
                    (point.y - bounds.min.y, bounds.height())
                };
                let Some(Node::Split { first, second, .. }) =
                    state.layout.snapshot().nodes.get(&split)
                else {
                    return Err("Resize split was retired".into());
                };
                let a = state
                    .renderer
                    .minimum(world, state.layout.snapshot(), *first);
                let b = state
                    .renderer
                    .minimum(world, state.layout.snapshot(), *second);
                let divider = state.renderer.divider_size(world, split, axis);
                let available = (length - divider).max(1.);
                let (min, max) = if axis == Axis::Horizontal {
                    (a.x / available, 1. - b.x / available)
                } else {
                    (a.y / available, 1. - b.y / available)
                };
                if min >= max {
                    return Ok(());
                }
                let fraction =
                    ((position - divider * 0.5) / available).clamp(min.max(0.001), max.min(0.999));
                let candidate = session
                    .update(&state.layout, Command::Resize { split, fraction })
                    .map_err(|e| e.to_string())?;
                let skin = state.skin.as_ref().ok_or("Docking skin is absent")?.clone();
                let hosts = state
                    .surfaces
                    .iter()
                    .map(|(id, surface)| (*id, surface.root))
                    .collect();
                state.renderer.sync(
                    world,
                    candidate.snapshot(),
                    &hosts,
                    state.stash,
                    &skin,
                    &state.titles,
                    "dock",
                    4.,
                )?;
                state.renderer.constrain(world, candidate.snapshot());
            }
            if released {
                session
                    .finish(&mut state.layout)
                    .map_err(|e| e.to_string())?;
                state.rendered = None;
                state.preferences.dirty = true;
            } else if held {
                state.gesture = Some(Gesture::Resize {
                    host,
                    split,
                    session,
                    axis,
                    bounds,
                });
            } else {
                redraw(world, state)?;
            }
        }
    }
    Ok(())
}
pub(super) fn selected_view(state: &EditorWorkspace, selection: Selection) -> Option<ViewId> {
    match selection {
        Selection::View(view) => Some(view),
        Selection::Group(group) => match state.layout.snapshot().nodes.get(&group) {
            Some(Node::Group { active, .. }) => Some(*active),
            _ => None,
        },
    }
}
fn keyboard(world: &mut World, state: &mut EditorWorkspace) -> Result<(), String> {
    let focus = world.resource::<bevy::input_focus::InputFocus>().get();
    if let Some(divider) = focus
        .and_then(|entity| world.get::<Divider>(entity))
        .copied()
    {
        let keys = world.resource::<ButtonInput<KeyCode>>();
        let delta = if keys.any_just_pressed([KeyCode::ArrowRight, KeyCode::ArrowDown]) {
            0.02
        } else if keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::ArrowUp]) {
            -0.02
        } else {
            0.
        };
        if delta != 0.
            && let Some(Node::Split {
                fraction,
                axis,
                first,
                second,
            }) = state.layout.snapshot().nodes.get(&divider.split)
        {
            let bounds = state
                .renderer
                .group(divider.split)
                .and_then(|root| rect(world, root))
                .ok_or("Divider bounds are unavailable")?;
            let a = state
                .renderer
                .minimum(world, state.layout.snapshot(), *first);
            let b = state
                .renderer
                .minimum(world, state.layout.snapshot(), *second);
            let size = (if *axis == Axis::Horizontal {
                bounds.width()
            } else {
                bounds.height()
            }) - state.renderer.divider_size(world, divider.split, *axis);
            let (min, max) = if *axis == Axis::Horizontal {
                (a.x / size, 1. - b.x / size)
            } else {
                (a.y / size, 1. - b.y / size)
            };
            if min < max {
                let fraction = (fraction + delta).clamp(min.max(0.001), max.min(0.999));
                apply(
                    world,
                    state,
                    Command::Resize {
                        split: divider.split,
                        fraction,
                    },
                )?;
            }
        }
        return Ok(());
    }
    let view = focus
        .and_then(|entity| world.get::<Control>(entity))
        .and_then(|control| match control {
            Control::Tab(view) => Some(*view),
            _ => None,
        });
    let Some(view) = view else {
        return Ok(());
    };
    let Some((group, index)) = state.layout.snapshot().location(view) else {
        return Ok(());
    };
    let Some(Node::Group { tabs, .. }) = state.layout.snapshot().nodes.get(&group) else {
        return Ok(());
    };
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let next = if keys.just_pressed(KeyCode::ArrowRight) {
        Some((index + 1) % tabs.len())
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        Some((index + tabs.len() - 1) % tabs.len())
    } else if keys.just_pressed(KeyCode::Home) {
        Some(0)
    } else if keys.just_pressed(KeyCode::End) {
        Some(tabs.len() - 1)
    } else {
        None
    };
    if let Some(next) = next {
        let view = tabs[next];
        apply(world, state, Command::Activate(view))?;
        if let Some(label) = state.renderer.label(view) {
            world
                .resource_mut::<bevy::input_focus::InputFocus>()
                .set(label, bevy::input_focus::FocusCause::Navigated);
        }
    }
    Ok(())
}
pub(super) fn cancel(world: &mut World, state: &mut EditorWorkspace) {
    super::feedback::clear(world, state);
    state.gesture = None;
    clear_measurement(world, state);
    state.rendered = None;
}
fn pointer(world: &World, state: &EditorWorkspace) -> Option<(HostId, Vec2)> {
    let sample = world.resource::<CursorSample>();
    let source = sample.window?;
    let window = world.get::<Window>(source)?;
    let local = sample.position;
    if let Some(host) = state
        .surfaces
        .iter()
        .find_map(|(id, surface)| (surface.window == source).then_some(*id))
        && Rect::from_corners(Vec2::ZERO, window.size()).contains(local)
    {
        return Some((host, local));
    }
    // Native coordinate conversion is a capability, never a Wayland assumption.
    bevy::winit::WINIT_WINDOWS.with_borrow(|native| {
        let source = native.get_window(source)?;
        let origin = source.inner_position().ok()?;
        let global = Vec2::new(origin.x as f32, origin.y as f32) + local * window.scale_factor();
        state.surfaces.iter().find_map(|(host, surface)| {
            let window = world.get::<Window>(surface.window)?;
            let native = native.get_window(surface.window)?;
            let origin = native.inner_position().ok()?;
            let point =
                (global - Vec2::new(origin.x as f32, origin.y as f32)) / window.scale_factor();
            Rect::from_corners(Vec2::ZERO, window.size())
                .contains(point)
                .then_some((*host, point))
        })
    })
}
fn measurement(
    world: &mut World,
    state: &mut EditorWorkspace,
    snapshot: &kedra_docking::snapshot::Snapshot,
    selection: Selection,
) -> Result<(), String> {
    let view = match selection {
        Selection::View(view) => view,
        Selection::Group(group) => match state.layout.snapshot().nodes.get(&group) {
            Some(Node::Group { active, .. }) => *active,
            _ => return Err("Drag group was retired".into()),
        },
    };
    let mut roots = BTreeMap::new();
    for (id, surface) in &state.surfaces {
        let root = world
            .spawn((
                UiNode::default(),
                bevy_flair::style::components::ClassList::new_with_classes(["dock-measure"]),
                Visibility::Hidden,
                ChildOf(surface.root),
            ))
            .id();
        roots.insert(*id, root);
    }
    let stash = world
        .spawn((
            UiNode::default(),
            bevy_flair::style::components::ClassList::new_with_classes(["dock-stash"]),
            ChildOf(state.stash),
        ))
        .id();
    let mut projection = Projection::default();
    let destinations = super::headers::measurement(world, state, snapshot, &roots)?;
    projection.set_header_destinations(world, destinations)?;
    projection.sync(
        world,
        snapshot,
        &roots,
        stash,
        state.skin.as_ref().ok_or("Docking skin absent")?,
        &state.titles,
        "measurement",
        4.,
    )?;
    projection.constrain(world, snapshot);
    // Move projected roots out of the hidden stash; its remaining content is empty.
    let target = snapshot.location(view).ok_or("Preview view is absent")?.0;
    let host = snapshot.host_of(target).ok_or("Preview host is absent")?;
    let overlay = world
        .spawn((
            UiNode::default(),
            bevy_flair::style::components::ClassList::new_with_classes(["dock-preview", "hidden"]),
            Pickable::IGNORE,
            GlobalZIndex(100),
            kedra_ui::ElementId::new("dock.preview"),
            ChildOf(state.surfaces[&host].root),
        ))
        .id();
    state.measurements = Some(Measurement {
        projection,
        roots,
        stash,
        overlay,
        snapshot: snapshot.clone(),
        target,
        ready: 0,
        bounds: None,
    });
    Ok(())
}
fn update_measurement(world: &mut World, state: &mut EditorWorkspace) -> Result<(), String> {
    let Some(measure) = state.measurements.as_mut() else {
        return Ok(());
    };
    measure.projection.constrain(world, &measure.snapshot);
    let measured = measured_bounds(world, measure);
    measure.ready = match measured {
        Some(bounds) if measure.bounds == Some(bounds) => measure.ready.saturating_add(1),
        Some(_) => 1,
        None => 0,
    };
    measure.bounds = measured;
    class(world, measure.overlay, "hidden", measure.ready < 3);
    if let Some((bounds, parent)) = measured {
        let origin = bounds.min - parent.min;
        geometry(
            world,
            measure.overlay,
            Geometry::Rectangle(Rect::from_corners(origin, origin + bounds.size())),
        )?;
    }
    Ok(())
}
fn measured_bounds(world: &World, measure: &Measurement) -> Option<(Rect, Rect)> {
    let host = measure.snapshot.host_of(measure.target)?;
    if measure.snapshot.hosts[&host]
        .maximized
        .is_some_and(|max| max != measure.target)
    {
        return None;
    }
    let entity = measure.projection.group(measure.target)?;
    if world.get::<UiNode>(entity)?.display == Display::None {
        return None;
    }
    let bounds = rect(world, entity)?;
    let parent = world.get::<ChildOf>(measure.overlay)?.parent();
    let parent = rect(world, parent)?;
    let minimum = measure
        .projection
        .minimum(world, &measure.snapshot, measure.target);
    (bounds.min.is_finite()
        && bounds.max.is_finite()
        && parent.min.is_finite()
        && parent.max.is_finite()
        && bounds.size().min_element() > 0.
        && bounds.width() + 0.5 >= minimum.x
        && bounds.height() + 0.5 >= minimum.y
        && parent.inflate(0.5).contains(bounds.min)
        && parent.inflate(0.5).contains(bounds.max))
    .then_some((bounds, parent))
}
fn clear_measurement(world: &mut World, state: &mut EditorWorkspace) {
    if let Some(measure) = state.measurements.take() {
        for root in measure.roots.values() {
            world.despawn(*root);
        }
        world.despawn(measure.stash);
        world.despawn(measure.overlay);
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod admission_tests;

#[cfg(test)]
#[path = "gesture_tests.rs"]
mod gesture_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activation_observer_is_nonexclusive_and_ignores_unrelated_buttons() {
        let mut app = App::new();
        app.add_observer(activate);
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().trigger(Activate { entity });
        app.update();
        assert!(!app.world().contains_resource::<EditorWorkspace>());
    }
}
