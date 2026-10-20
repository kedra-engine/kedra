//! Production feedback targets, asset readiness and blocked-state behavior.
use super::*;
use crate::workspace::{
    input::regression_tests::{box_at, setup},
    test_support::{TestResult, sources, visible},
};

#[test]
fn compass_readiness_all_directions_and_blocked_selection_use_real_feedback() -> TestResult {
    let (mut app, a, _) = setup()?;
    let world = app.world_mut();
    sources(world)?;
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let group = state.layout.snapshot().location(a).ok_or("group")?.0;
            box_at(
                world,
                state.root,
                Vec2::new(400., 300.),
                Vec2::new(800., 600.),
            );
            box_at(
                world,
                state.renderer.group(group).ok_or("group root")?,
                Vec2::new(400., 300.),
                Vec2::new(800., 600.),
            );
            visible(world);
            let center = Vec2::new(400., 300.);
            let host = state.host;
            let pending = update(world, &mut state, Selection::View(a), host, center)?;
            assert!(
                pending.destination.is_none(),
                "Unloaded guide assets must not expose a compass destination"
            );
            let feedback = state.feedback.as_ref().ok_or("feedback")?;
            let handles: Vec<_> = feedback
                .guides
                .iter()
                .flat_map(|g| g.images.iter())
                .chain(feedback.ghost_images.iter())
                .flatten()
                .cloned()
                .collect();
            for handle in handles {
                world
                    .resource_mut::<Assets<Image>>()
                    .insert(handle.id(), Image::default())
                    .map_err(|e| e.to_string())?;
            }
            assert!(state.feedback.as_ref().is_some_and(|f| f.ready(world)));
            for index in 0..5 {
                let host = state.host;
                let target = update(
                    world,
                    &mut state,
                    Selection::View(a),
                    host,
                    guide_bounds(center, index).center(),
                )?;
                assert_eq!(target.destination, Some(destination(&state, group, index)));
                assert!(!target.insertion);
            }
            let host = state.host;
            let blocked = update(world, &mut state, Selection::Group(group), host, center)?;
            assert!(blocked.destination.is_none());
            let ghost = state.feedback.as_ref().ok_or("feedback")?.ghost.root;
            assert!(
                world
                    .get::<bevy_flair::style::components::ClassList>(ghost)
                    .is_some_and(|c| c.contains("blocked"))
            );
            let entities: Vec<_> = state
                .feedback
                .as_ref()
                .ok_or("feedback")?
                .guides
                .iter()
                .map(|g| g.root)
                .chain([ghost])
                .collect();
            clear(world, &mut state);
            assert!(state.feedback.is_none());
            assert!(entities.iter().all(|e| !world.entities().contains(*e)));
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn direct_edges_report_minimum_rejection_and_empty_host_destination() -> TestResult {
    let (mut app, a, b) = setup()?;
    let world = app.world_mut();
    sources(world)?;
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let group = state.layout.snapshot().location(a).ok_or("group")?.0;
            let root = state.renderer.group(group).ok_or("root")?;
            box_at(
                world,
                state.root,
                Vec2::new(400., 300.),
                Vec2::new(800., 600.),
            );
            box_at(world, root, Vec2::new(400., 300.), Vec2::new(800., 600.));
            world
                .get_mut::<bevy::ui::Node>(root)
                .ok_or("node")?
                .min_width = px(500.);
            let host = state.host;
            let target = update(
                world,
                &mut state,
                Selection::View(a),
                host,
                Vec2::new(799., 300.),
            )?;
            assert!(target.destination.is_none());
            let ghost = state.feedback.as_ref().ok_or("feedback")?.ghost.root;
            assert!(
                world
                    .get::<bevy_flair::style::components::ClassList>(ghost)
                    .is_some_and(|c| c.contains("blocked"))
            );
            let floating = HostId::new();
            state
                .layout
                .execute(Command::Float {
                    selection: Selection::Group(group),
                    host: floating,
                })
                .map_err(|e| e.to_string())?;
            // A blank main host remains an explicit destination, not an auto-float.
            let empty = update(
                world,
                &mut state,
                Selection::View(b),
                host,
                Vec2::new(100., 300.),
            )?;
            assert_eq!(empty.destination, Some(Destination::EmptyHost(host)));
            Ok(())
        },
    )?;
    Ok(())
}
