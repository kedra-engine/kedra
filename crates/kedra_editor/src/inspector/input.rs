//! Native events route through the existing workspace's visibility/lifetime authority.
use super::*;
use bevy::input_focus::FocusedInput;
pub(super) fn install(app: &mut App) {
    app.add_observer(activate).add_observer(keyboard);
}
fn activate(
    event: On<bevy::ui_widgets::Activate>,
    bindings: Query<&Binding>,
    disclosures: Query<&Disclosure>,
    mut commands: Commands,
) {
    if let Ok(binding) = bindings.get(event.entity) {
        let binding = binding.clone();
        commands.queue(move |world: &mut World| crate::workspace::inspector_action(world, binding));
    } else if let Ok(disclosure) = disclosures.get(event.entity) {
        let disclosure = disclosure.clone();
        commands.queue(move |world: &mut World| {
            crate::workspace::inspector_disclosure(world, disclosure)
        });
    }
}
fn keyboard(
    mut event: On<FocusedInput<bevy::input::keyboard::KeyboardInput>>,
    bindings: Query<&Binding>,
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    if !event.input.state.is_pressed() {
        return;
    }
    let Ok(binding) = bindings.get(event.event_target()) else {
        return;
    };
    let command = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    let action = if command && event.input.key_code == KeyCode::KeyF {
        Some(Action::Search)
    } else if event.input.key_code == KeyCode::Escape {
        Some(Action::CloseSearch)
    } else {
        None
    };
    if let Some(action) = action {
        let binding = Binding {
            view: binding.view,
            action,
        };
        event.propagate(false);
        commands.queue(move |world: &mut World| crate::workspace::inspector_action(world, binding));
    }
}
