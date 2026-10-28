//! Dock input routing and candidate admission with synthetic geometry, not rendered layout.
use super::super::test_support::{box_at, setup};
use super::*;
use kedra_docking::model::Edge;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn activation_observer_is_nonexclusive_and_ignores_unrelated_buttons() {
    let mut app = App::new();
    app.add_observer(activate);
    let entity = app.world_mut().spawn_empty().id();
    app.world_mut().trigger(Activate { entity });
    app.update();
    assert!(!app.world().contains_resource::<EditorWorkspace>());
}

#[test]
fn missing_measurements_never_become_ready() -> TestResult {
    let (mut app, _, b) = setup()?;
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let snapshot = state.layout.snapshot().clone();
            measurement(world, &mut state, &snapshot, Selection::View(b))?;
            for _ in 0..5 {
                update_measurement(world, &mut state)?;
            }
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(0));
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn maximized_edge_candidate_is_not_admissible() -> TestResult {
    let (mut app, a, b) = setup()?;
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let group = state
                .layout
                .snapshot()
                .location(a)
                .ok_or("Missing group")?
                .0;
            let host = state.host;
            state
                .layout
                .execute(Command::Maximize {
                    host,
                    group: Some(group),
                })
                .map_err(|e| e.to_string())?;
            let root = state.renderer.group(group).ok_or("Missing chrome")?;
            box_at(world, root, Vec2::splat(400.), Vec2::splat(800.));
            box_at(world, state.root, Vec2::splat(400.), Vec2::splat(800.));
            let before = state.layout.snapshot().clone();
            let destination = Destination::Edge {
                target: group,
                edge: Edge::Right,
                fraction: 0.5,
            };
            assert!(!super::super::feedback::admissible(
                world,
                &state,
                Selection::View(b),
                destination
            ));
            assert_eq!(state.layout.snapshot(), &before);
            state
                .layout
                .execute(Command::Maximize { host, group: None })
                .map_err(|e| e.to_string())?;
            assert!(super::super::feedback::admissible(
                world,
                &state,
                Selection::View(b),
                destination
            ));
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn ready_measurement_is_revoked_when_geometry_changes_or_retires() -> TestResult {
    let (mut app, _, b) = setup()?;
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let snapshot = state.layout.snapshot().clone();
            measurement(world, &mut state, &snapshot, Selection::View(b))?;
            let measure = state.measurements.as_ref().ok_or("Missing measurement")?;
            let target = measure
                .projection
                .group(measure.target)
                .ok_or("Missing target")?;
            box_at(world, state.root, Vec2::splat(400.), Vec2::splat(800.));
            box_at(world, target, Vec2::splat(200.), Vec2::splat(400.));
            for _ in 0..3 {
                update_measurement(world, &mut state)?;
            }
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(3));
            // Current measurements, not an earlier settled rectangle, gate release.
            box_at(world, target, Vec2::splat(250.), Vec2::splat(400.));
            update_measurement(world, &mut state)?;
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(1));
            box_at(world, target, Vec2::splat(200.), Vec2::ZERO);
            update_measurement(world, &mut state)?;
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(0));
            box_at(world, target, Vec2::splat(900.), Vec2::splat(400.));
            update_measurement(world, &mut state)?;
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(0));
            world.despawn(target);
            update_measurement(world, &mut state)?;
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(0));
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn invisible_maximized_candidate_rejects_even_stale_positive_geometry() -> TestResult {
    let (mut app, a, b) = setup()?;
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let group = state
                .layout
                .snapshot()
                .location(a)
                .ok_or("Missing group")?
                .0;
            let host = state.host;
            state
                .layout
                .execute(Command::Maximize {
                    host,
                    group: Some(group),
                })
                .map_err(|e| e.to_string())?;
            let before = state.layout.snapshot().clone();
            let candidate = state
                .layout
                .prepare(Command::Move {
                    selection: Selection::View(b),
                    destination: Destination::Edge {
                        target: group,
                        edge: Edge::Right,
                        fraction: 0.5,
                    },
                })
                .map_err(|e| e.to_string())?;
            measurement(world, &mut state, candidate.snapshot(), Selection::View(b))?;
            let measure = state.measurements.as_ref().ok_or("Missing measurement")?;
            let target = measure
                .projection
                .group(measure.target)
                .ok_or("Missing target")?;
            box_at(world, state.root, Vec2::splat(400.), Vec2::splat(800.));
            box_at(world, target, Vec2::splat(200.), Vec2::splat(400.));
            for _ in 0..5 {
                update_measurement(world, &mut state)?;
            }
            assert_eq!(state.measurements.as_ref().map(|m| m.ready), Some(0));
            assert_eq!(state.layout.snapshot(), &before);
            Ok(())
        },
    )?;
    Ok(())
}
