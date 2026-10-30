//! Single-owner source gesture: projection preview, guarded one-shot publication.
use super::*;
use crate::documents::inspection::Target;
use kedra_scene::document::DocumentId;
mod overlay;
mod tools;
use tools::Constraint;
pub(super) use tools::Mode;

#[cfg(debug_assertions)]
pub(super) fn diagnostics(world: &World) -> serde_json::Value {
    let state = world.get_resource::<State>();
    serde_json::json!({"enabled":state.is_some_and(|s|s.mode.is_some()),
        "mode":state.and_then(|s|s.mode).map(Mode::name),
        "snap":state.is_some_and(|s|s.snap), "active":active(world),
        "handles":overlay::diagnostics(world)})
}

#[derive(Resource, Default)]
struct State {
    mode: Option<Mode>,
    snap: bool,
    drag: Option<Drag>,
}
struct Drag {
    target: Target,
    revisions: BTreeMap<DocumentId, u64>,
    view: ViewId,
    window: Entity,
    camera: Entity,
    entity: Entity,
    layout: u64,
    area: Rect,
    baseline: Transform,
    candidate: Transform,
    parent: Mat4,
    constraint: Constraint,
    mode: Mode,
    axis: usize,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>();
}
pub(super) fn shortcut(world: &mut World, event: &bevy::input::keyboard::KeyboardInput) -> bool {
    let mode = match event.key_code {
        KeyCode::KeyW => Mode::Move,
        KeyCode::KeyE => Mode::Rotate,
        KeyCode::KeyR => Mode::Scale,
        _ => return false,
    };
    if !event.state.is_pressed()
        || event.repeat
        || active(world)
        || world.resource::<ButtonInput<KeyCode>>().any_pressed([
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::AltLeft,
            KeyCode::AltRight,
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
        ])
    {
        return false;
    }
    select(world, Some(mode));
    true
}
pub(super) fn press(
    world: &mut World,
    workspace: &EditorWorkspace,
    view: ViewId,
    window: Entity,
    point: Vec2,
) -> bool {
    match begin(world, workspace, view, window, point) {
        Ok(false) => false,
        Ok(true) => {
            if let Some(mut focus) = world.get_resource_mut::<bevy::input_focus::InputFocus>() {
                focus.set(
                    workspace.panels.scenes[&view].focus(),
                    bevy::input_focus::FocusCause::Pressed,
                );
            }
            true
        }
        Err(error) => {
            notice(world, workspace, &error);
            true
        }
    }
}
pub(super) fn active(world: &World) -> bool {
    world
        .get_resource::<State>()
        .is_some_and(|state| state.drag.is_some())
}
pub(super) fn select(world: &mut World, mode: Option<Mode>) {
    cancel(world);
    world.init_resource::<State>();
    world.resource_mut::<State>().mode = mode;
}
pub(super) fn toggle_snap(world: &mut World) {
    cancel(world);
    world.init_resource::<State>();
    let mut state = world.resource_mut::<State>();
    state.snap = !state.snap;
}
pub(super) fn cancel(world: &mut World) {
    let drag = world
        .get_resource_mut::<State>()
        .and_then(|mut state| state.drag.take());
    if let Some(drag) = drag {
        restore(world, &drag);
    }
    overlay::clear(world);
}
fn restore(world: &mut World, drag: &Drag) {
    if let Some(mut transform) = world.get_mut::<Transform>(drag.entity)
        && *transform == drag.candidate
    {
        *transform = drag.baseline;
    }
}
fn selected(state: &EditorWorkspace) -> Option<(ViewId, Target)> {
    let document = state.documents.active?;
    let opened = state.documents.opened.get(&document)?;
    let selected = state.documents.selected_nodes(document);
    if selected.len() != 1 {
        return None;
    }
    Some((
        opened.view,
        state.documents.target(document, selected[0].clone())?,
    ))
}
pub(super) fn begin(
    world: &mut World,
    workspace: &EditorWorkspace,
    view: ViewId,
    window: Entity,
    point: Vec2,
) -> Result<bool, String> {
    let Some(state) = world.get_resource::<State>() else {
        return Ok(false);
    };
    let snap = state.snap;
    let Some(mode) = state.mode else {
        return Ok(false);
    };
    if state.drag.is_some()
        || settlement::source_edits(world, workspace)
        || crate::drag::active(world).is_some()
    {
        return Ok(false);
    }
    let Some((selected_view, target)) = selected(workspace).filter(|(v, _)| *v == view) else {
        return Ok(false);
    };
    let scene = &workspace.panels.scenes[&selected_view];
    let Some((entity, baseline, parent, layer)) = scene.manipulation_target(world, &target.node)
    else {
        return Ok(false);
    };
    let (camera, area) = scene_input::eligible(world, workspace, view, window)
        .ok_or("Transform viewport unavailable")?;
    let Some(geometry) = overlay::geometry(world, camera, area, (baseline, parent, mode), layer)
    else {
        return Ok(false);
    };
    let Some(axis) = geometry.hit(point) else {
        return Ok(false);
    };
    // Admission uses the same reflected property policy as Inspector before preview starts.
    prepare(
        world,
        workspace,
        &target,
        &workspace.documents.revisions(),
        baseline,
        mode,
    )?;
    let constraint = geometry
        .begin(
            baseline,
            parent,
            axis,
            scene.manipulation_ray(world, point)?,
            snap,
        )
        .map_err(str::to_owned)?;
    world.resource_mut::<State>().drag = Some(Drag {
        target,
        revisions: workspace.documents.revisions(),
        view,
        window,
        camera,
        entity,
        layout: workspace.layout.revision(),
        area,
        baseline,
        candidate: baseline,
        parent,
        constraint,
        mode,
        axis,
    });
    Ok(true)
}
fn prepare(
    world: &World,
    workspace: &EditorWorkspace,
    target: &Target,
    revisions: &BTreeMap<DocumentId, u64>,
    candidate: Transform,
    mode: Mode,
) -> Result<crate::documents::PreparedChange, String> {
    let registry = world.resource::<AppTypeRegistry>().clone();
    let (field, value): (&str, &dyn bevy::reflect::PartialReflect) = match mode {
        Mode::Move => ("translation", &candidate.translation),
        Mode::Rotate => ("rotation", &candidate.rotation),
        Mode::Scale => ("scale", &candidate.scale),
    };
    workspace.documents.prepare_property(
        target,
        revisions,
        "scene.transform",
        &[kedra_authoring::properties::PropertyEdit {
            path: bevy::reflect::ParsedPath::parse(field).map_err(|e| e.to_string())?,
            value,
        }],
        &registry.read(),
        world.resource::<kedra_scene::reflection::ComponentPolicies>(),
    )
}
fn valid(world: &World, workspace: &EditorWorkspace, drag: &Drag) -> bool {
    selected(workspace).is_some_and(|(view, target)| view == drag.view && target == drag.target)
        && workspace.documents.revisions() == drag.revisions
        && workspace.layout.revision() == drag.layout
        && scene_input::eligible(world, workspace, drag.view, drag.window)
            .is_some_and(|(camera, area)| camera == drag.camera && area == drag.area)
        && world
            .get_resource::<bevy::input_focus::InputFocus>()
            .and_then(|f| f.get())
            == workspace.panels.scenes.get(&drag.view).map(|s| s.focus())
        && workspace
            .panels
            .scenes
            .get(&drag.view)
            .and_then(|s| s.manipulation_target(world, &drag.target.node))
            .is_some_and(|(entity, local, parent, _)| {
                entity == drag.entity && parent == drag.parent && local == drag.candidate
            })
}
pub(super) fn maintain(
    world: &mut World,
    workspace: &EditorWorkspace,
    cancelled: &std::collections::HashSet<Entity>,
) {
    let invalid = world
        .get_resource::<State>()
        .and_then(|s| s.drag.as_ref())
        .is_some_and(|d| cancelled.contains(&d.window) || !valid(world, workspace, d));
    if invalid {
        cancel(world);
    }
}
pub(super) fn release(
    world: &mut World,
    workspace: &mut EditorWorkspace,
    window: Entity,
) -> Result<bool, String> {
    if !world
        .get_resource::<State>()
        .and_then(|s| s.drag.as_ref())
        .is_some_and(|d| d.window == window)
    {
        return Ok(false);
    }
    update(world, workspace);
    let Some(drag) = world.resource_mut::<State>().drag.take() else {
        return Ok(true);
    };
    restore(world, &drag);
    if drag.candidate == drag.baseline {
        return Ok(true);
    }
    let context = history_context::capture(world, workspace);
    let change = prepare(
        world,
        workspace,
        &drag.target,
        &drag.revisions,
        drag.candidate,
        drag.mode,
    )?;
    if !workspace.documents.change_current(&change) {
        return Err("Transform source changed before release".into());
    }
    workspace.documents.publish_change(change);
    history_context::accepted(world, workspace, context);
    Ok(true)
}
fn update(world: &mut World, workspace: &EditorWorkspace) {
    if !active(world) {
        return;
    }
    world.resource_scope(|world, mut state: Mut<State>| {
        let Some(drag) = state.drag.as_mut() else {
            return;
        };
        // State is removed during resource_scope; validation never depends on its presence.
        if !valid(world, workspace, drag) {
            let drag = state.drag.take();
            if let Some(drag) = drag {
                restore(world, &drag);
            }
            return;
        }
        let sample = world.resource::<input::CursorSample>();
        if sample.window != Some(drag.window) {
            return;
        }
        let candidate = workspace.panels.scenes[&drag.view]
            .manipulation_ray(world, sample.position)
            .and_then(|ray| drag.constraint.update(ray).map_err(str::to_owned));
        if let Ok(candidate) = candidate {
            if let Some(mut transform) = world.get_mut::<Transform>(drag.entity) {
                *transform = candidate;
            }
            drag.candidate = candidate;
        }
    });
}
pub(super) fn tick(world: &mut World, workspace: &EditorWorkspace) {
    update(world, workspace);
    let Some(state) = world.get_resource::<State>() else {
        return;
    };
    let (mode, snap, axis) = (state.mode, state.snap, state.drag.as_ref().map(|d| d.axis));
    for scene in workspace.panels.scenes.values() {
        for (name, selected) in [
            ("hand", mode.is_none()),
            ("move", mode == Some(Mode::Move)),
            ("rotate", mode == Some(Mode::Rotate)),
            ("scale", mode == Some(Mode::Scale)),
            ("snap", snap),
        ] {
            if let Some(entity) = scene.control(name) {
                kedra_ui::docking::class(world, entity, "selected", selected);
            }
        }
    }
    let geometry = mode
        .is_some()
        .then(|| selected(workspace))
        .flatten()
        .and_then(|(view, target)| {
            let scene = workspace.panels.scenes.get(&view)?;
            let window = kedra_ui::native_input::owner(world, scene.focus())?;
            let (camera, area) = scene_input::eligible(world, workspace, view, window)?;
            let (_, mut local, parent, layer) = scene.manipulation_target(world, &target.node)?;
            if let Some(drag) = world.get_resource::<State>().and_then(|s| s.drag.as_ref())
                && drag.mode == Mode::Rotate
            {
                local = drag.baseline;
            }
            let geometry = overlay::geometry(world, camera, area, (local, parent, mode?), layer)?;
            let sample = world.resource::<input::CursorSample>();
            let hover = (sample.window == Some(window)
                && area.contains(sample.position)
                && scene_tools::at(world, scene, sample.position).is_none())
            .then(|| geometry.hit(sample.position))
            .flatten();
            Some((geometry, axis.or(hover)))
        });
    if let Some((geometry, axis)) = geometry {
        overlay::draw(world, geometry, axis);
    } else {
        overlay::clear(world);
    }
}
