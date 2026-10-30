//! Native scroll-position updates for docking-owned rails and content surfaces.
use super::*;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use kedra_ui::docking::{TabRail, rect};

pub(super) fn wheel(mut events: MessageReader<MouseWheel>, mut commands: Commands) {
    // Native messages are collected before scheduling structural scroll updates.
    let events: Vec<_> = events.read().cloned().collect();
    if events.is_empty() {
        return;
    }
    commands.queue(move |world: &mut World| {
        if !world.contains_resource::<EditorWorkspace>() {
            return;
        }
        world.resource_scope(|world, state: Mut<EditorWorkspace>| {
            for event in events {
                let Some(host) = state
                    .surfaces
                    .iter()
                    .find_map(|(id, s)| (s.window == event.window).then_some(*id))
                else {
                    continue;
                };
                let Some(point) = world
                    .get_resource::<input::CursorSample>()
                    .filter(|sample| sample.window == Some(event.window))
                    .map(|sample| sample.position)
                    .or_else(|| {
                        world
                            .get::<Window>(event.window)
                            .and_then(Window::cursor_position)
                    })
                else {
                    continue;
                };
                let host_root = state.surfaces[&host].root;
                let horizontal_input = event.x != 0.
                    || world
                        .resource::<ButtonInput<KeyCode>>()
                        .any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
                let rail =
                    world
                        .query::<(Entity, &TabRail)>()
                        .iter(world)
                        .find_map(|(entity, rail)| {
                            (rail.host == host
                                && state.renderer.rail(rail.group) == Some(entity)
                                && rect(world, entity).is_some_and(|r| r.contains(point)))
                            .then_some(entity)
                        });
                let snapshot = state.layout.snapshot();
                let content = snapshot.nodes.iter().find_map(|(group, node)| {
                    let kedra_docking::model::Node::Group { active, .. } = node else {
                        return None;
                    };
                    if snapshot.host_of(*group) != Some(host)
                        || snapshot.hosts[&host]
                            .maximized
                            .is_some_and(|max| max != *group)
                    {
                        return None;
                    }
                    let root = state.renderer.content(*active)?;
                    if !is_under(world, root, host_root) {
                        return None;
                    }
                    let nested = world
                        .iter_entities()
                        .filter_map(|entity| {
                            let region = entity.get::<crate::asset_browser::ScrollRegion>()?;
                            (is_under(world, entity.id(), root)
                                && scrollable_at(world, entity.id(), point, horizontal_input))
                            .then_some((region.0, entity.id()))
                        })
                        .max_by_key(|(priority, _)| *priority)
                        .map(|(_, entity)| entity);
                    nested.or_else(|| {
                        scrollable_at(world, root, point, horizontal_input).then_some(root)
                    })
                });
                let popup = state.menu.iter().copied().find(|root| {
                    is_under(world, *root, host_root)
                        && scrollable_at(world, *root, point, horizontal_input)
                });
                let entity = popup
                    .or(rail.filter(|root| extent(world, *root).x > 0.).or(content))
                    .unwrap_or(state.surfaces[&host].root);
                {
                    let max = extent(world, entity);
                    let horizontal =
                        rail == Some(entity) || horizontal_input || (max.x > 0. && max.y == 0.);
                    let scale = if event.unit == MouseScrollUnit::Line {
                        20.
                    } else {
                        1.
                    };
                    let delta = if horizontal {
                        if event.x != 0. { event.x } else { event.y }
                    } else {
                        event.y
                    };
                    let mut scroll = world
                        .get::<ScrollPosition>(entity)
                        .cloned()
                        .unwrap_or_default();
                    if horizontal {
                        scroll.0.x = (scroll.0.x - delta * scale).clamp(0., max.x);
                    } else {
                        scroll.0.y = (scroll.0.y - delta * scale).clamp(0., max.y);
                    }
                    world.entity_mut(entity).insert(scroll);
                }
            }
        });
    });
}
fn scrollable_at(world: &World, entity: Entity, point: Vec2, horizontal: bool) -> bool {
    let max = extent(world, entity);
    (if horizontal {
        max.x > 0.
    } else {
        max.y > 0. || max.x > 0.
    }) && rect(world, entity).is_some_and(|r| r.contains(point))
}
fn is_under(world: &World, mut entity: Entity, root: Entity) -> bool {
    for _ in 0..128 {
        if entity == root {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
    false
}
fn extent(world: &World, entity: Entity) -> Vec2 {
    world
        .get::<ComputedNode>(entity)
        .map(|node| (node.content_size() - node.size()) * node.inverse_scale_factor())
        .unwrap_or(Vec2::ZERO)
        .max(Vec2::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kedra_docking::{command::Selection, model::View};
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn content(world: &mut World, parent: Entity, overflow: f32) -> Entity {
        world
            .spawn((
                Node::default(),
                ComputedNode {
                    size: Vec2::splat(100.),
                    content_size: Vec2::new(100., 100. + overflow),
                    inverse_scale_factor: 1.,
                    ..default()
                },
                UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::splat(50.))),
                ChildOf(parent),
            ))
            .id()
    }

    fn run_scroll(main_overflow: f32, popup: bool, scrollable_popup: bool) -> TestResult {
        run_scroll_overlay(main_overflow, popup, scrollable_popup, false)
    }
    fn run_scroll_overlay(
        main_overflow: f32,
        popup: bool,
        scrollable_popup: bool,
        overlay: bool,
    ) -> TestResult {
        let mut app = App::new();
        app.add_message::<MouseWheel>()
            .add_message::<bevy::window::CursorMoved>()
            .init_resource::<input::CursorSample>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, (input::cursor, wheel).chain());
        let world = app.world_mut();
        let window = world.spawn(Window::default()).id();
        let root = world.spawn(Node::default()).id();
        super::super::bind_root(world, window, root)?;
        let mut floating_window = Window::default();
        // Backend cursor state may lag the ordered Bevy pointer events.
        floating_window.set_cursor_position(Some(Vec2::splat(500.)));
        let floating_window = world.spawn(floating_window).id();
        let floating_root = world.spawn(Node::default()).id();
        let float = HostId::new();
        let ids: Vec<ViewId> = serde_json::from_value(serde_json::json!([
            "f0000000-0000-4000-8000-000000000001",
            "f0000000-0000-4000-8000-000000000002",
            "f0000000-0000-4000-8000-000000000003"
        ]))?;
        let main_content = content(world, root, main_overflow);
        let inactive_content = content(world, floating_root, 200.);
        let active_content = content(world, floating_root, 200.);
        let nested = overlay.then(|| {
            let viewport = content(world, active_content, 200.);
            world
                .entity_mut(viewport)
                .insert(crate::asset_browser::ScrollRegion(0));
            let tooltip = content(world, active_content, 200.);
            world
                .entity_mut(tooltip)
                .insert(crate::asset_browser::ScrollRegion(2));
            (viewport, tooltip)
        });
        let wrong_popup = content(world, root, 200.);
        let inert_popup = content(world, floating_root, 0.);
        let local_popup = content(world, floating_root, 200.);
        world.resource_scope(|_, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let host = state.host;
            for (index, id) in ids.iter().enumerate() {
                let destination = if index == 0 {
                    Destination::EmptyHost(host)
                } else {
                    let group = state
                        .layout
                        .snapshot()
                        .location(ids[0])
                        .ok_or("Missing group")?
                        .0;
                    Destination::Tabs { group, index }
                };
                state
                    .layout
                    .execute(Command::Open {
                        id: *id,
                        view: View {
                            kind: "test".into(),
                            restore_key: None,
                        },
                        destination,
                    })
                    .map_err(|e| e.to_string())?;
            }
            state
                .layout
                .execute(Command::Float {
                    selection: Selection::View(ids[1]),
                    host: float,
                })
                .map_err(|e| e.to_string())?;
            let group = state
                .layout
                .snapshot()
                .location(ids[1])
                .ok_or("Missing float")?
                .0;
            state
                .layout
                .execute(Command::Move {
                    selection: Selection::View(ids[2]),
                    destination: Destination::Tabs { group, index: 1 },
                })
                .map_err(|e| e.to_string())?;
            state.surfaces.insert(
                float,
                Surface {
                    window: floating_window,
                    root: floating_root,
                    owned: None,
                    staged: None,
                },
            );
            for (id, entity) in ids
                .iter()
                .zip([main_content, inactive_content, active_content])
            {
                state.renderer.register(*id, entity)?;
            }
            if popup {
                state.menu.extend([wrong_popup, inert_popup]);
            }
            if scrollable_popup {
                state.menu.push(local_popup);
            }
            Ok(())
        })?;
        world.write_message(bevy::window::CursorMoved {
            window: floating_window,
            position: Vec2::splat(50.),
            delta: None,
        });
        world.write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.,
            y: -1.,
            window: floating_window,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
        let target = if let Some((_, tooltip)) = nested {
            tooltip
        } else if scrollable_popup {
            local_popup
        } else {
            active_content
        };
        let untouched = if scrollable_popup {
            active_content
        } else {
            local_popup
        };
        assert_eq!(
            app.world().get::<ScrollPosition>(target).map(|s| s.0.y),
            Some(20.)
        );
        for entity in [
            main_content,
            inactive_content,
            wrong_popup,
            inert_popup,
            untouched,
            nested.map_or(root, |(viewport, _)| viewport),
        ] {
            assert_eq!(
                app.world()
                    .get::<ScrollPosition>(entity)
                    .map(|s| s.0.y)
                    .unwrap_or_default(),
                0.
            );
        }
        // A pointer sample in the floating host cannot authorize scrolling main.
        app.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.,
            y: -1.,
            window,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
        assert_eq!(
            app.world()
                .get::<ScrollPosition>(main_content)
                .map(|scroll| scroll.0.y)
                .unwrap_or_default(),
            0.
        );
        Ok(())
    }

    #[test]
    fn r14_scroll_is_scoped_to_event_host_and_active_content() -> TestResult {
        run_scroll(200., false, false)
    }

    #[test]
    fn r14_unscrollable_earlier_content_does_not_suppress_target() -> TestResult {
        run_scroll(0., false, false)
    }

    #[test]
    fn r14_other_host_and_unscrollable_popups_do_not_steal_wheel() -> TestResult {
        run_scroll(200., true, false)
    }

    #[test]
    fn r14_scrollable_popup_in_event_host_takes_priority() -> TestResult {
        run_scroll(200., true, true)
    }
    #[test]
    fn scrollable_browser_hint_takes_priority_over_the_covered_file_viewport() -> TestResult {
        run_scroll_overlay(200., false, false, true)
    }
}
