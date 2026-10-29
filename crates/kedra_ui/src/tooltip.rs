//! Shared, non-interactive hover hints over native UI layout and pointer state.
use bevy::{
    picking::{
        hover::Hovered,
        pointer::{PointerId, PointerLocation},
    },
    prelude::*,
};
use bevy_flair::style::components::ClassList;

/// Attach a plain-text hover hint to a native UI node.
///
/// Install [`TooltipPlugin`] once. The enclosing stylesheet must style `.tooltip`
/// including an explicit font; no font/theme is supplied.
/// Empty text disables the hint. A hint never takes focus or pointer input.
/// The source owns its bubble: retirement, hiding, or leaving the source closes it.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
#[require(Hovered)]
pub struct Tooltip(pub String);

/// Hover delay, pointer placement and disposable bubble lifetime for [`Tooltip`].
/// Requires native UI picking/layout and `Time<Real>`; creates no window or camera.
pub struct TooltipPlugin;
impl Plugin for TooltipPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Active>()
            .init_resource::<kedra_wakeup::WakeDemand>()
            .add_systems(Update, update);
    }
}

#[derive(Resource, Default)]
struct Active {
    source: Option<Entity>,
    text: String,
    since: std::time::Duration,
    bubble: Option<Entity>,
}
const DELAY: std::time::Duration = std::time::Duration::from_millis(350);
const GAP: f32 = 12.;
const MARGIN: f32 = 4.;

fn placement(cursor: Vec2, size: Vec2, viewport: Vec2) -> Vec2 {
    let max = (viewport - size - Vec2::splat(MARGIN)).max(Vec2::splat(MARGIN));
    let mut position = cursor + Vec2::splat(GAP);
    if position.y > max.y {
        position.y = cursor.y - size.y - GAP;
    }
    position.clamp(Vec2::splat(MARGIN), max)
}

fn update(world: &mut World) {
    let now = world.resource::<Time<Real>>().elapsed();
    let pressed = world
        .get_resource::<ButtonInput<MouseButton>>()
        .is_some_and(|buttons| buttons.get_just_pressed().next().is_some());
    // Use the same native pointer location as picking (including injected input),
    // rather than Window's hardware-only cursor cache.
    let location = world
        .query::<(&PointerId, &PointerLocation)>()
        .iter(world)
        .find_map(|(id, pointer)| {
            (*id == PointerId::Mouse)
                .then(|| pointer.location().cloned())
                .flatten()
        });
    let primary = world
        .query_filtered::<Entity, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next();
    let scale = world.get_resource::<UiScale>().map_or(1., |scale| scale.0);
    let candidate = world
        .query::<(
            Entity,
            &Tooltip,
            &Hovered,
            &ComputedUiTargetCamera,
            &ComputedUiRenderTargetInfo,
            &InheritedVisibility,
        )>()
        .iter(world)
        .filter(|(_, hint, hover, _, _, visible)| hover.0 && visible.get() && !hint.0.is_empty())
        .filter_map(|(entity, hint, _, target_camera, target, _)| {
            let location = location.as_ref()?;
            let camera_entity = target_camera.get()?;
            let camera = world.get::<Camera>(camera_entity)?;
            let render_target = world.get::<bevy::camera::RenderTarget>(camera_entity)?;
            if render_target.normalize(primary).as_ref() != Some(&location.target) {
                return None;
            }
            let rect = camera.logical_viewport_rect()?;
            if !rect.contains(location.position) {
                return None;
            }
            let viewport = target.logical_size();
            (viewport.min_element() > 2. * MARGIN).then(|| {
                (
                    entity,
                    hint.0.clone(),
                    (location.position - rect.min) / scale,
                    viewport,
                )
            })
        })
        .next();
    present(world, candidate, now, pressed);
}

fn present(
    world: &mut World,
    candidate: Option<(Entity, String, Vec2, Vec2)>,
    now: std::time::Duration,
    pressed: bool,
) {
    world.resource_scope(|world, mut active: Mut<Active>| {
        let source = candidate.as_ref().map(|value| value.0);
        let text = candidate.as_ref().map_or("", |value| value.1.as_str());
        if source != active.source || text != active.text || pressed {
            if let Some(bubble) = active.bubble.take()
                && let Ok(entity) = world.get_entity_mut(bubble)
            {
                entity.despawn();
            }
            active.source = source;
            active.text = text.into();
            active.since = now;
        }
        let Some((source, text, cursor, viewport)) = candidate else {
            return;
        };
        if now.saturating_sub(active.since) < DELAY {
            // One subsystem integration covers every Tooltip, including changed
            // targets/text and retirement. No control owns a wake call or timer.
            if let Some(time) = world.get_resource::<Time<Real>>().copied()
                && let Some(mut demand) = world.get_resource_mut::<kedra_wakeup::WakeDemand>()
            {
                demand.at_real(&time, active.since + DELAY);
            }
            return;
        }
        let bubble = if let Some(bubble) = active
            .bubble
            .filter(|entity| world.entities().contains(*entity))
        {
            bubble
        } else {
            let id = world.get::<crate::ElementId>(source).map(|id| id.0.clone());
            let bubble = world
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        max_width: px(320),
                        max_height: px((viewport.y - 2. * MARGIN).max(1.)),
                        ..default()
                    },
                    ClassList::new("tooltip"),
                    Text::new(text),
                    TextLayout::linebreak(bevy::text::LineBreak::WordOrCharacter),
                    bevy::ui::FixedNode,
                    GlobalZIndex(300),
                    bevy::picking::Pickable::IGNORE,
                    ChildOf(source),
                ))
                .id();
            if let Some(id) = id {
                world
                    .entity_mut(bubble)
                    .insert(crate::ElementId::new(format!("{id}/tooltip")));
            }
            active.bubble = Some(bubble);
            bubble
        };
        let size = world
            .get::<ComputedNode>(bubble)
            .map_or(Vec2::ZERO, |node| node.size() * node.inverse_scale_factor());
        let position = placement(cursor, size, viewport);
        if let Some(mut node) = world.get_mut::<Node>(bubble) {
            let next = (
                px(position.x),
                px(position.y),
                px(320_f32.min((viewport.x - 2. * MARGIN).max(1.))),
                px((viewport.y - 2. * MARGIN).max(1.)),
            );
            if (node.left, node.top, node.max_width, node.max_height) != next {
                (node.left, node.top, node.max_width, node.max_height) = next;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_hints_follow_pointer_and_flip_at_window_edges() {
        let viewport = Vec2::new(800., 600.);
        let size = Vec2::new(120., 28.);
        assert_eq!(
            placement(Vec2::new(200., 300.), size, viewport),
            Vec2::new(212., 312.)
        );
        let edge = placement(Vec2::new(795., 595.), size, viewport);
        assert!(edge.x + size.x <= 796. && edge.y + size.y < 595.);
        assert!(placement(Vec2::ZERO, size, viewport).min_element() >= MARGIN);
    }

    #[test]
    fn shared_lifecycle_delays_closes_replaces_and_reclaims_noninteractive_hints()
    -> Result<(), &'static str> {
        let mut world = World::new();
        world.init_resource::<Active>();
        world.init_resource::<Time<Real>>();
        let origin = world.resource::<Time<Real>>().startup() + std::time::Duration::from_secs(5);
        world
            .resource_mut::<Time<Real>>()
            .update_with_instant(origin);
        world.init_resource::<kedra_wakeup::WakeDemand>();
        let source = world.spawn(Node::default()).id();
        let candidate = Some((
            source,
            "Straße.txt".into(),
            Vec2::new(100., 200.),
            Vec2::new(800., 600.),
        ));
        present(
            &mut world,
            candidate.clone(),
            std::time::Duration::ZERO,
            false,
        );
        assert!(world.resource::<Active>().bubble.is_none());
        let mut expected = kedra_wakeup::WakeDemand::default();
        expected.at(origin + DELAY);
        assert_eq!(world.resource::<kedra_wakeup::WakeDemand>(), &expected);
        world.insert_resource(kedra_wakeup::WakeDemand::default());
        present(&mut world, candidate.clone(), DELAY, false);
        assert_eq!(
            world.resource::<kedra_wakeup::WakeDemand>(),
            &kedra_wakeup::WakeDemand::default()
        );
        let bubble = world.resource::<Active>().bubble.ok_or("Delayed hint")?;
        assert!(
            world.get::<bevy::ui::FixedNode>(bubble).is_some(),
            "Native fixed layout escapes source clipping/transforms"
        );
        let node = world.get::<Node>(bubble).ok_or("Bubble layout")?;
        assert_eq!((node.left, node.top), (px(112), px(212)));
        assert_eq!(
            (node.width, node.right, node.bottom),
            (Val::Auto, Val::Auto, Val::Auto),
            "Never stretch a hint across its source/panel"
        );
        assert_eq!(node.max_width, px(320));
        world.clear_trackers();
        present(&mut world, candidate.clone(), DELAY, false);
        assert!(
            !world
                .entity(bubble)
                .get_ref::<Node>()
                .ok_or("Bubble layout")?
                .is_changed()
        );
        assert_eq!(world.get::<Text>(bubble).ok_or("Text")?.0, "Straße.txt");
        let pick = world
            .get::<bevy::picking::Pickable>(bubble)
            .ok_or("Pointer passthrough")?;
        assert!(!pick.is_hoverable && !pick.should_block_lower);
        assert!(
            world
                .get::<bevy::input_focus::tab_navigation::TabIndex>(bubble)
                .is_none()
        );
        present(&mut world, None, DELAY, false);
        assert_eq!(
            world.resource::<kedra_wakeup::WakeDemand>(),
            &kedra_wakeup::WakeDemand::default()
        );
        assert!(!world.entities().contains(bubble));
        present(&mut world, candidate.clone(), DELAY, false);
        present(&mut world, candidate.clone(), DELAY * 2, false);
        let bubble = world.resource::<Active>().bubble.ok_or("Reopened")?;
        present(&mut world, candidate.clone(), DELAY * 2, true);
        assert!(
            !world.entities().contains(bubble),
            "Click closes and restarts dwell"
        );
        present(&mut world, candidate, DELAY * 3, false);
        let bubble = world.resource::<Active>().bubble.ok_or("New bubble")?;
        world.despawn(source);
        assert!(!world.entities().contains(bubble));
        present(&mut world, None, DELAY * 3, false);
        assert!(world.resource::<Active>().bubble.is_none());
        Ok(())
    }
}
