//! Subsystem-owned hints; native cadence remains an opt-in host policy.
use bevy::{input_focus::InputFocus, prelude::*};
use kedra_wakeup::WakeDemand;

pub(crate) fn caret(
    focus: Option<Res<InputFocus>>,
    fields: Query<(&bevy::text::EditableText, Option<&InheritedVisibility>)>,
    mut demand: ResMut<WakeDemand>,
) {
    if let Some(entity) = focus.as_deref().and_then(InputFocus::get)
        && let Ok((text, visible)) = fields.get(entity)
        && !text.cursor_blink_period.is_zero()
        && visible.is_none_or(|visible| visible.get())
    {
        // Bevy's cursor phase is private Local state. Keep its real implementation
        // serviced, rather than introduce a second blink clock that can diverge
        // on editing, layout, IME or focus resets. Blur/retirement ends this hint.
        demand.active();
    }
}

pub(crate) fn sources(
    sources: Option<Res<kedra_scene::assets::PrefabSources>>,
    mut demand: ResMut<WakeDemand>,
) {
    if sources
        .as_deref()
        .is_some_and(|sources| sources.error.is_none() && !sources.ready())
    {
        demand.active();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn only_a_live_visible_focused_blinking_field_demands_active_frames() -> Result<(), String> {
        let mut world = World::new();
        world.init_resource::<WakeDemand>();
        world.init_resource::<InputFocus>();
        let field = world
            .spawn((
                bevy::text::EditableText::default(),
                InheritedVisibility::VISIBLE,
            ))
            .id();
        world
            .resource_mut::<InputFocus>()
            .set(field, bevy::input_focus::FocusCause::Navigated);
        world.run_system_once(caret).map_err(|e| e.to_string())?;
        // Consume through the real host adapter in integration/native checks;
        // here observe that blur does not reproduce a request on a fresh frame.
        assert_ne!(world.resource::<WakeDemand>(), &WakeDemand::default());
        world.insert_resource(WakeDemand::default());
        world.entity_mut(field).insert(InheritedVisibility::HIDDEN);
        world.run_system_once(caret).map_err(|e| e.to_string())?;
        assert_eq!(world.resource::<WakeDemand>(), &WakeDemand::default());
        world.despawn(field);
        world.run_system_once(caret).map_err(|e| e.to_string())?;
        assert_eq!(world.resource::<WakeDemand>(), &WakeDemand::default());
        Ok(())
    }
}
