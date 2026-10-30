//! Scene View tool hits, bindings and explicit keyboard/projection commands.
//! Use the same rendered, clipped logical geometry as docking.
use super::*;
use crate::scene_view::orientation::Axis;

pub(super) fn align(
    world: &mut World,
    state: &EditorWorkspace,
    view: ViewId,
    axis: Axis,
) -> Result<(), String> {
    super::scene_input::cancel(world);
    if let Some(camera) = state
        .panels
        .scenes
        .get(&view)
        .and_then(|scene| scene.navigation(world))
    {
        crate::scene_view::orientation::align(world, camera, axis)?;
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Tool {
    Orbit,
    Axis(Axis),
    Block,
}
fn axis_at(world: &World, targets: &[(Axis, Entity)], point: Vec2) -> Option<(Tool, Entity)> {
    targets
        .iter()
        .filter(|(_, entity)| {
            hit::visible_at(world, *entity, point, 0.)
                && kedra_ui::docking::rect(world, *entity).is_some_and(|rect| {
                    let radius = rect.size() * 0.5;
                    ((point - rect.center()) / radius).length_squared() <= 1.
                })
        })
        .max_by_key(|(_, entity)| {
            world
                .get::<bevy::ui::ComputedStackIndex>(*entity)
                .map_or(0, |index| index.0)
        })
        .map(|(axis, entity)| (Tool::Axis(*axis), *entity))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn axis_hits_use_rendered_depth_and_geometry_not_the_next_animation_pose() {
        let mut world = World::new();
        let mut targets = Vec::new();
        for (axis, rank) in [(Axis::Z, 20), (Axis::NegZ, 10)] {
            let entity = world
                .spawn((
                    Node::default(),
                    ComputedNode {
                        size: Vec2::splat(21.),
                        inverse_scale_factor: 1.,
                        ..default()
                    },
                    UiGlobalTransform::default(),
                    Visibility::Inherited,
                    InheritedVisibility::VISIBLE,
                    bevy::ui::ComputedStackIndex(rank),
                ))
                .id();
            targets.push((axis, entity));
        }
        assert_eq!(
            axis_at(&world, &targets, Vec2::ZERO),
            Some((Tool::Axis(Axis::Z), targets[0].1))
        );
        assert_eq!(axis_at(&world, &targets, Vec2::splat(10.)), None);
        world.entity_mut(targets[0].1).insert(Visibility::Hidden);
        assert_eq!(
            axis_at(&world, &targets, Vec2::ZERO),
            Some((Tool::Axis(Axis::NegZ), targets[1].1))
        );
    }
}

pub(super) fn at(
    world: &World,
    scene: &crate::scene_view::View,
    point: Vec2,
) -> Option<(Tool, Entity)> {
    let targets = Axis::ALL
        .into_iter()
        .filter_map(|axis| scene.control(axis.name()).map(|entity| (axis, entity)))
        .collect::<Vec<_>>();
    if let Some(target) = axis_at(world, &targets, point) {
        return Some(target);
    }
    for (name, tool) in [
        ("orbit", Tool::Orbit),
        ("frame", Tool::Block),
        ("projection", Tool::Block),
        ("navigation", Tool::Block),
    ] {
        if let Some(entity) = scene.control(name)
            && hit::visible_at(world, entity, point, 0.)
        {
            return Some((tool, entity));
        }
    }
    None
}

/// Native Button Activate does not distinguish pointer from keyboard. Pointer
/// gestures are owned by scene_input; these buttons' Activate path is keyboard-only.
#[derive(Component)]
pub(super) struct DragControl;

pub(super) fn bind(world: &mut World, view: &UiView, id: ViewId) -> Result<(), String> {
    {
        let (name, label) = ("orbit", "Drag to orbit view");
        let entity = view.element(name).map_err(|e| e.to_string())?;
        world
            .entity_mut(entity)
            .insert((DragControl, kedra_ui::tooltip::Tooltip(label.into())));
        if let Some(mut node) = world.get_mut::<bevy::a11y::AccessibilityNode>(entity) {
            node.set_label(label);
        }
    }
    for axis in Axis::ALL {
        let entity = view.element(axis.name()).map_err(|e| e.to_string())?;
        world
            .entity_mut(entity)
            .insert((DragControl, input::Action::SceneAxis(id, axis)));
        if let Some(mut node) = world.get_mut::<bevy::a11y::AccessibilityNode>(entity) {
            node.set_label(format!("Align view to {:?}; repeat to flip", axis));
        }
    }
    Ok(())
}
