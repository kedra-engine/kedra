//! Native adapter for shared window chrome; composition stays with the consumer.
mod input;
use bevy::{prelude::*, ui_widgets::Activate};
#[cfg(test)]
use input::{PointerAction, pointer_action, resize_edge};
use input::{input, native_state};

/// Native command, distinct from feature/document close approval or Save.
#[derive(Clone, Copy, Debug)]
pub enum ChromeAction {
    /// Ask the platform to minimize the bound window.
    Minimize,
    /// Ask the platform to toggle acknowledged maximize state.
    ToggleMaximize,
    /// Emit the existing close request; the lifecycle owner decides its effect.
    Close,
}

/// Attach to a native UI button. Activation never targets an implicit main window.
#[derive(Component, Clone, Copy, Debug)]
pub struct ChromeControl {
    /// Explicit live native window.
    pub window: Entity,
    /// Command supported by this control.
    pub action: ChromeAction,
}

/// Backend-acknowledged chrome state, owned by the desktop adapter on the window.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct NativeChromeState {
    /// Whether Winit has acknowledged this window.
    pub ready: bool,
    /// Actual backend focus.
    pub focused: bool,
    /// Actual backend maximize state, not an optimistic command result.
    pub maximized: bool,
    /// Actual native fullscreen state.
    pub fullscreen: bool,
}

/// Enable native chrome operations on a live window with a shared UI header.
/// Idempotent, does not reset acknowledged state. No window or UI is allocated.
pub fn bind(world: &mut World, window: Entity) -> Result<(), String> {
    if world.get::<Window>(window).is_none() {
        return Err("Chrome window was retired".into());
    }
    if world.get::<NativeChromeState>(window).is_none() {
        world
            .entity_mut(window)
            .insert(NativeChromeState::default());
    }
    Ok(())
}
pub(crate) fn install(app: &mut App) {
    app.add_message::<bevy::input::mouse::MouseButtonInput>()
        .add_observer(activate)
        .add_systems(Update, (native_state, input).chain());
}
fn activate(
    event: On<Activate>,
    controls: Query<(&ChromeControl, Has<bevy::ui::InteractionDisabled>)>,
    mut windows: Query<(&mut Window, &NativeChromeState)>,
    mut close: MessageWriter<bevy::window::WindowCloseRequested>,
) {
    let Ok((control, disabled)) = controls.get(event.entity) else {
        return;
    };
    if disabled {
        return;
    }
    let Ok((mut window, state)) = windows.get_mut(control.window) else {
        return;
    };
    match control.action {
        ChromeAction::Minimize if window.enabled_buttons.minimize => window.set_minimized(true),
        ChromeAction::ToggleMaximize
            if window.enabled_buttons.maximize && window.resizable && !state.fullscreen =>
        {
            window.set_maximized(!state.maximized)
        }
        ChromeAction::Close if window.enabled_buttons.close => {
            close.write(bevy::window::WindowCloseRequested {
                window: control.window,
            });
        }
        _ => {}
    }
}
#[cfg(test)]
mod tests;
