//! Integrated rails are client UI above the body, not native-drag background.
use super::*;
use crate::workspace::input::regression_tests::{box_at, setup};

#[test]
fn external_rail_hit_bounds_exclude_native_drag_spacer() -> Result<(), Box<dyn std::error::Error>> {
    let (mut app, a, _) = setup()?;
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let state = &mut *state;
            let group_id = state
                .layout
                .snapshot()
                .location(a)
                .ok_or("Missing group")?
                .0;
            let parent = state.surfaces[&state.host].root;
            let row = world.spawn((Node::default(), ChildOf(parent))).id();
            let menu = world.spawn((Node::default(), ChildOf(parent))).id();
            state.renderer.set_header_destinations(
                world,
                BTreeMap::from([(group_id, kedra_ui::docking::HeaderDestination { row, menu })]),
            )?;
            let hosts = state
                .surfaces
                .iter()
                .map(|(id, surface)| (*id, surface.root))
                .collect();
            state.renderer.sync(
                world,
                state.layout.snapshot(),
                &hosts,
                state.stash,
                state.skin.as_ref().ok_or("Missing skin")?,
                &state.titles,
                "dock",
                1.,
            )?;
            let group_entity = state.renderer.group(group_id).ok_or("Missing group root")?;
            let label_entity = state.renderer.label(a).ok_or("Missing label")?;
            let rail = state.renderer.rail(group_id).ok_or("Missing rail")?;
            for entity in world.query::<Entity>().iter(world).collect::<Vec<_>>() {
                world
                    .entity_mut(entity)
                    .insert(InheritedVisibility::VISIBLE);
            }
            box_at(
                world,
                group_entity,
                Vec2::new(200., 232.),
                Vec2::new(400., 400.),
            );
            box_at(world, row, Vec2::new(100., 16.), Vec2::new(200., 32.));
            box_at(world, rail, Vec2::new(100., 16.), Vec2::new(200., 32.));
            box_at(
                world,
                label_entity,
                Vec2::new(50., 16.),
                Vec2::new(100., 28.),
            );
            let binding = *world
                .get::<TabLabel>(label_entity)
                .ok_or("Missing label binding")?;
            assert!(label(
                world,
                state,
                label_entity,
                &binding,
                Vec2::new(50., 16.)
            ));
            assert!(!group(
                world,
                state,
                state.host,
                group_id,
                Vec2::new(300., 16.)
            ));
            Ok(())
        },
    )?;
    Ok(())
}
