//! Acknowledged floating creation and native-host retirement, never content rebuild.
use super::*;
use bevy::window::Monitor;
use kedra_desktop::auxiliary;
use kedra_docking::{command::Selection, model::Candidate};
use std::time::Duration;
const CREATION_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) struct Pending {
    candidate: Candidate,
    host: HostId,
    surface: Surface,
    started: Duration,
    focus: Option<ViewId>,
}
#[derive(Resource, Default)]
pub(super) struct CloseRequests(Vec<Entity>);
pub(super) fn requests(
    mut events: MessageReader<bevy::window::WindowCloseRequested>,
    mut queue: ResMut<CloseRequests>,
) {
    queue.0.extend(events.read().map(|e| e.window));
}

pub(super) fn float(
    world: &mut World,
    state: &mut EditorWorkspace,
    selection: Selection,
) -> Result<(), String> {
    if state.pending.is_some() {
        return Err("A floating host is already being prepared".into());
    }
    let host = HostId::new();
    let candidate = state
        .layout
        .prepare(Command::Float { selection, host })
        .map_err(|e| e.to_string())?;
    let title = input::selected_view(state, selection)
        .and_then(|view| state.titles.get(&view))
        .map(|panel| {
            format!(
                "{panel} — {}",
                world
                    .get::<Window>(state.window)
                    .map(|window| window.title.as_str())
                    .unwrap_or("Docking")
            )
        })
        .unwrap_or_else(|| "Docking".into());
    let surface = create(world, None, title)?;
    state.pending = Some(Pending {
        candidate,
        host,
        surface,
        started: world.resource::<Time<Real>>().elapsed(),
        focus: input::selected_view(state, selection),
    });
    Ok(())
}
fn create(
    world: &mut World,
    placement: Option<&storage::Placement>,
    title: String,
) -> Result<Surface, String> {
    let size = placement.map_or(UVec2::new(800, 560), |p| UVec2::new(p.width, p.height));
    let native = auxiliary::stage(world, title, size)?;
    let result = (|| {
        if let Some(placement) = placement {
            place(world, native.window, placement)?;
        }
        let frame = crate::views::mount_asset(
            world,
            native.root,
            "editor://prefabs/window-frame.prefab.ron",
            "floating-frame",
        )?;
        let caption = crate::views::mount_asset(
            world,
            frame.root,
            "editor://prefabs/titlebar.prefab.ron",
            "floating-caption",
        )?;
        crate::chrome::bind_window(world, &caption, native.window, Some(frame.root))?;
        let root = world
            .spawn((
                Node::default(),
                bevy_flair::style::components::ClassList::new_with_classes(["dock-host"]),
                ChildOf(frame.root),
            ))
            .id();
        Ok::<_, String>(Surface {
            window: native.window,
            root,
            owned: Some(native),
            staged: None,
        })
    })();
    if result.is_err() {
        auxiliary::retire(world, native);
    }
    result
}
pub(super) fn restore(
    world: &mut World,
    state: &mut EditorWorkspace,
    placements: &BTreeMap<HostId, storage::Placement>,
) -> Result<(), String> {
    if let Some(placement) = placements.get(&state.host) {
        place(world, state.window, placement)?;
    }
    let hosts: Vec<_> = state
        .layout
        .snapshot()
        .hosts
        .keys()
        .filter(|host| **host != state.host)
        .copied()
        .collect();
    for host in hosts {
        match create(
            world,
            placements.get(&host),
            "Docking — Kedra Editor".into(),
        ) {
            Ok(mut surface) => {
                surface.staged = Some(world.resource::<Time<Real>>().elapsed());
                state.surfaces.insert(host, surface);
            }
            Err(error) => {
                recover(state, host)?;
                notice(
                    world,
                    state,
                    &format!(
                        "Restored floating host failed: {error}. Its views were moved to the main window."
                    ),
                );
            }
        }
    }
    Ok(())
}
fn place(world: &mut World, window: Entity, placement: &storage::Placement) -> Result<(), String> {
    let mut monitors: Vec<_> = world
        .query::<(&Monitor, Has<bevy::window::PrimaryMonitor>)>()
        .iter(world)
        .map(|(monitor, primary)| (monitor.clone(), primary))
        .collect();
    monitors.sort_by_key(|(_, primary)| !primary);
    let origin = placement.position.map(IVec2::from_array);
    let monitor = origin
        .and_then(|origin| {
            monitors.iter().find(|(monitor, _)| {
                Rect::from_corners(
                    monitor.physical_position.as_vec2(),
                    monitor.physical_position.as_vec2() + monitor.physical_size().as_vec2(),
                )
                .contains(origin.as_vec2())
            })
        })
        .or_else(|| monitors.first());
    let (size, position) = if let Some((monitor, _)) = monitor {
        let scale = monitor.scale_factor as f32;
        let logical = monitor.physical_size().as_vec2() / scale.max(0.5);
        let size = UVec2::new(
            placement.width.min(logical.x as u32).max(240),
            placement.height.min(logical.y as u32).max(192),
        );
        let position = if let Some(origin) = origin {
            let max = monitor.physical_position + monitor.physical_size().as_ivec2()
                - (size.as_vec2() * scale).as_ivec2();
            WindowPosition::At(origin.clamp(
                monitor.physical_position,
                max.max(monitor.physical_position),
            ))
        } else {
            WindowPosition::Centered(MonitorSelection::Primary)
        };
        (size, position)
    } else {
        (
            UVec2::new(placement.width, placement.height),
            WindowPosition::Centered(MonitorSelection::Primary),
        )
    };
    auxiliary::restore_placement(world, window, size, position)
}
pub(super) fn poll(world: &mut World, state: &mut EditorWorkspace) -> Result<(), String> {
    poll_restored(world, state)?;
    if let Some(pending) = state.pending.take() {
        let native = pending
            .surface
            .owned
            .ok_or("Staged native ownership was lost")?;
        if auxiliary::ready(world, native) {
            if let Err(error) = state.layout.commit(pending.candidate) {
                auxiliary::retire(world, native);
                return Err(error.to_string());
            }
            state.surfaces.insert(pending.host, pending.surface);
            state.focused_group = pending.focus.and_then(|view| {
                state
                    .layout
                    .snapshot()
                    .location(view)
                    .map(|(group, _)| group)
            });
            redraw(world, state)?;
            auxiliary::show(world, native)?;
            state.preferences.dirty = true;
        } else if world
            .resource::<Time<Real>>()
            .elapsed()
            .saturating_sub(pending.started)
            > CREATION_TIMEOUT
        {
            auxiliary::retire(world, native);
            return Err("Floating window creation timed out; source content was kept".into());
        } else {
            state.pending = Some(pending);
        }
    }
    let closed = std::mem::take(&mut world.resource_mut::<CloseRequests>().0);
    for window in closed {
        if let Some(host) = state.surfaces.iter().find_map(|(host, surface)| {
            (surface.window == window && *host != state.host).then_some(*host)
        }) {
            // Current registered panels are tools. Their close policy is Hide;
            // document feature kinds must supply approval before using this path.
            apply(world, state, Command::HideHost(host))?;
        }
    }
    let empty: Vec<_> = state
        .layout
        .snapshot()
        .hosts
        .iter()
        .filter_map(|(id, host)| (*id != state.host && host.root.is_none()).then_some(*id))
        .collect();
    for host in empty {
        state
            .layout
            .execute(Command::RetireHost(host))
            .map_err(|e| e.to_string())?;
    }
    let retired: Vec<_> = state
        .surfaces
        .keys()
        .filter(|id| !state.layout.snapshot().hosts.contains_key(id))
        .copied()
        .collect();
    if !retired.is_empty() {
        redraw(world, state)?;
    }
    for host in retired {
        if let Some(surface) = state.surfaces.remove(&host)
            && let Some(native) = surface.owned
        {
            auxiliary::retire(world, native);
        }
    }
    for surface in state.surfaces.values() {
        if let Some(native) = surface.owned
            && auxiliary::ready(world, native)
        {
            auxiliary::show(world, native)?;
        }
    }
    Ok(())
}
pub(super) fn retire(world: &mut World, state: &mut EditorWorkspace) {
    if let Some(pending) = state.pending.take()
        && let Some(native) = pending.surface.owned
    {
        auxiliary::retire(world, native);
    }
    let hosts: Vec<_> = state
        .surfaces
        .keys()
        .filter(|id| **id != state.host)
        .copied()
        .collect();
    for host in hosts {
        if let Some(surface) = state.surfaces.remove(&host)
            && let Some(native) = surface.owned
        {
            auxiliary::retire(world, native);
        }
    }
}

// Reuse normal model moves in a private candidate workspace. No prefix of a
// recovery may publish, and valid sibling hosts/hidden restore records survive.
fn recover(state: &mut EditorWorkspace, host: HostId) -> Result<(), String> {
    let mut layout = state
        .layout
        .snapshot()
        .clone()
        .into_workspace()
        .map_err(|e| e.to_string())?;
    let groups: Vec<_> = layout
        .snapshot()
        .nodes
        .iter()
        .filter_map(|(id, node)| {
            (matches!(node, kedra_docking::model::Node::Group { .. })
                && layout.snapshot().host_of(*id) == Some(host))
            .then_some(*id)
        })
        .collect();
    for group in groups {
        let destination = layout
            .snapshot()
            .nodes
            .iter()
            .find_map(|(id, node)| {
                (matches!(node, kedra_docking::model::Node::Group { .. })
                    && layout.snapshot().host_of(*id) == Some(state.host)
                    && layout.snapshot().hosts[&state.host]
                        .maximized
                        .is_none_or(|max| max == *id))
                .then_some(Destination::Tabs {
                    group: *id,
                    index: 0,
                })
            })
            .unwrap_or(Destination::EmptyHost(state.host));
        layout
            .execute(Command::Move {
                selection: Selection::Group(group),
                destination,
            })
            .map_err(|e| e.to_string())?;
    }
    layout
        .execute(Command::RetireHost(host))
        .map_err(|e| e.to_string())?;
    state.layout = layout;
    state.rendered = None;
    state.preferences.dirty = true;
    Ok(())
}

fn poll_restored(world: &mut World, state: &mut EditorWorkspace) -> Result<(), String> {
    let staged: Vec<_> = state
        .surfaces
        .iter()
        .filter_map(|(host, surface)| surface.staged.map(|started| (*host, *surface, started)))
        .collect();
    for (host, surface, started) in staged {
        let timed_out = world
            .resource::<Time<Real>>()
            .elapsed()
            .saturating_sub(started)
            > CREATION_TIMEOUT;
        let ready = world.entities().contains(surface.root)
            && surface
                .owned
                .is_some_and(|native| auxiliary::ready(world, native));
        if ready && !timed_out {
            if let Some(surface) = state.surfaces.get_mut(&host) {
                surface.staged = None;
            }
            state.rendered = None;
            if let Err(error) = redraw(world, state) {
                // A failed projection is not publication. Keep its original
                // deadline so repeated projection errors cannot leave it hidden forever.
                if let Some(surface) = state.surfaces.get_mut(&host) {
                    surface.staged = Some(started);
                }
                return Err(error);
            }
        } else if timed_out {
            input::cancel(world, state);
            recover(state, host)?;
            // Content was stashed during staging. Redraw moves it before retiring
            // any native hierarchy, just as fresh Float/Redock does.
            redraw(world, state)?;
            state.surfaces.remove(&host);
            if let Some(native) = surface.owned {
                auxiliary::retire(world, native);
            }
            notice(
                world,
                state,
                "Restored floating window timed out; its views were moved to the main window.",
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
