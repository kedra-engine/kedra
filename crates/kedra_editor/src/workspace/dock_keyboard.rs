//! Keyboard navigation and resize of focused docking controls.
use super::*;

pub(super) fn keyboard(world: &mut World, state: &mut EditorWorkspace) -> Result<(), String> {
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
