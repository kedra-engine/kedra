//! Window-owned adaptation of Bevy 0.20's primary-window-only input integration.
//! Native widgets, tab ordering, text edits and event propagation remain Bevy-owned.
mod ime;
mod navigation;
use bevy::{
    camera::RenderTarget,
    ecs::{message::MessageCursor, schedule::ScheduleCleanupPolicy},
    input::{InputSystems, keyboard::KeyboardInput, mouse::MouseWheel},
    input_focus::{
        FocusedInput, InputDispatchPlugin, InputFocus, InputFocusSystems, dispatch_focused_input,
    },
    prelude::*,
    ui_widgets::{ImeSystems, TextInputPlugin},
    window::{PrimaryWindow, WindowRef},
};

/// Adapts native focused input and IME to independently owned UI windows.
///
/// Install after Bevy's input-focus and text-input plugins. Each technical UI root
/// needs `UiTargetCamera` and `TabGroup`; its camera must target a live window.
/// This replaces only stock keyboard/wheel dispatch and primary-window IME systems
/// through Bevy's schedule-removal API. It retains native widget/IME observers,
/// gamepad dispatch and `TabNavigation`'s ordering/traversal. Hidden/disabled tab
/// indices are temporarily gated to -1 and restored when eligible again.
pub struct NativeInputPlugin;
impl Plugin for NativeInputPlugin {
    fn build(&self, app: &mut App) {
        assert!(
            app.is_plugin_added::<InputDispatchPlugin>()
                && app.is_plugin_added::<TextInputPlugin>(),
            "NativeInputPlugin requires Bevy InputDispatchPlugin and TextInputPlugin"
        );
        app.add_observer(navigation::tab)
            .add_observer(bevy::input_focus::tab_navigation::acquire_focus_tab_index);
        if !app.is_plugin_added::<kedra_text_input::TextSessionPlugin>() {
            app.add_plugins(kedra_text_input::TextSessionPlugin);
        }
        crate::fields::install_text_fields(app);
    }
    fn finish(&self, app: &mut App) {
        app.world_mut()
            .schedule_scope(PreUpdate, |world, schedule| {
                let policy = ScheduleCleanupPolicy::RemoveSystemsOnly;
                schedule
                    .remove_systems_in_set(dispatch_focused_input::<KeyboardInput>, world, policy)
                    .unwrap_or_else(|e| panic!("Cannot replace native keyboard dispatch: {e}"));
                schedule
                    .remove_systems_in_set(dispatch_focused_input::<MouseWheel>, world, policy)
                    .unwrap_or_else(|e| panic!("Cannot replace native wheel dispatch: {e}"));
                schedule
                    .remove_systems_in_set(ImeSystems::ToggleWindowIMEInput, world, policy)
                    .unwrap_or_else(|e| {
                        panic!("Cannot replace primary-window IME enablement: {e}")
                    });
                schedule
                    .remove_systems_in_set(ImeSystems::HandleEvents, world, policy)
                    .unwrap_or_else(|e| panic!("Cannot replace unscoped IME input: {e}"));
            });
        app.world_mut()
            .schedule_scope(PostUpdate, |world, schedule| {
                schedule
                    .remove_systems_in_set(
                        ImeSystems::UpdatePosition,
                        world,
                        ScheduleCleanupPolicy::RemoveSystemsOnly,
                    )
                    .unwrap_or_else(|e| panic!("Cannot replace primary-window IME position: {e}"));
            });
        app.add_systems(
            PreUpdate,
            navigation::gate
                .before(InputFocusSystems::Dispatch)
                .after(InputSystems),
        )
        .add_systems(
            PreUpdate,
            (keyboard, wheel)
                .chain()
                .in_set(InputFocusSystems::Dispatch)
                .after(InputSystems),
        )
        .add_systems(
            PreUpdate,
            ime::enable
                .in_set(ImeSystems::ToggleWindowIMEInput)
                .after(InputFocusSystems::Dispatch)
                .after(bevy::ui::UiSystems::Focus),
        )
        .add_systems(PreUpdate, ime::events.in_set(ImeSystems::HandleEvents))
        .add_systems(
            PostUpdate,
            ime::position
                .in_set(ImeSystems::UpdatePosition)
                .in_set(bevy::ui::UiSystems::PostLayout)
                .after(bevy::ui::widget::update_editable_text_layout),
        );
    }
}

// Explicit camera ancestry wins over renderer-derived components from a prior
// frame, so transferring a retained root cannot route input to its former host.
/// Resolve a retained UI node's live owning window through its target camera.
/// Returns `None` for retired nodes, inactive cameras or non-window targets.
/// This is presentation ownership only, not focus, modal or editing permission.
pub fn owner(world: &World, mut entity: Entity) -> Option<Entity> {
    let mut computed = None;
    let camera = loop {
        if world.get::<Window>(entity).is_some() {
            return Some(entity);
        }
        if let Some(target) = world.get::<UiTargetCamera>(entity) {
            break Some(target.0);
        }
        computed = computed.or_else(|| {
            world
                .get::<ComputedUiTargetCamera>(entity)
                .and_then(|target| target.get())
        });
        let Some(parent) = world.get::<ChildOf>(entity) else {
            break computed;
        };
        entity = parent.parent();
    }?;
    if world
        .get::<Camera>(camera)
        .is_none_or(|camera| !camera.is_active)
    {
        return None;
    }
    match world.get::<RenderTarget>(camera)? {
        RenderTarget::Window(WindowRef::Entity(window)) => {
            world.get::<Window>(*window).map(|_| *window)
        }
        RenderTarget::Window(WindowRef::Primary) => world
            .iter_entities()
            .find(|e| e.contains::<PrimaryWindow>())
            .map(|e| e.id()),
        _ => None,
    }
}

pub(crate) fn eligible(world: &World, mut entity: Entity) -> bool {
    loop {
        if world.get_entity(entity).is_err()
            || world
                .get::<Node>(entity)
                .is_some_and(|node| node.display == Display::None)
            || world.get::<Visibility>(entity) == Some(&Visibility::Hidden)
            || world
                .get::<InheritedVisibility>(entity)
                .is_some_and(|v| !v.get())
            || world.get::<bevy::ui::InteractionDisabled>(entity).is_some()
        {
            return false;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return true;
        };
        entity = parent.parent();
    }
}

// The same boundary applies to keyboard, wheel and IME, including the interval
// before Bevy's delayed modal focus handoff. This does not move focus itself.
fn focused(world: &mut World, window: Entity) -> Option<Entity> {
    let focus = world.resource::<InputFocus>().get()?;
    if world.get::<Window>(focus).is_some()
        || owner(world, focus) != Some(window)
        || !eligible(world, focus)
    {
        return None;
    }
    navigation::group(world, window)
        .is_none_or(|(root, modal)| !modal || navigation::under(world, focus, root))
        .then_some(focus)
}

fn target(world: &mut World, window: Entity, move_focus: bool) -> Option<Entity> {
    world.get::<Window>(window)?;
    if let Some(focus) = focused(world, window) {
        return Some(focus);
    }
    if move_focus {
        world.resource_mut::<InputFocus>().clear();
    }
    navigation::group(world, window)
        .map(|(root, _)| root)
        .or(Some(window))
}
fn keyboard(world: &mut World, mut cursor: Local<MessageCursor<KeyboardInput>>) {
    let events: Vec<_> = cursor
        .read(world.resource::<Messages<KeyboardInput>>())
        .cloned()
        .collect();
    for event in events {
        if let Some(target) = target(world, event.window, event.state.is_pressed()) {
            if crate::fields::field_keyboard(world, target, &event) {
                continue;
            }
            if kedra_text_input::handle_keyboard(world, target, &event) {
                continue;
            }
            let window = event.window;
            world.trigger(FocusedInput::new(target, event, window));
        }
    }
}
fn wheel(world: &mut World, mut cursor: Local<MessageCursor<MouseWheel>>) {
    let events: Vec<_> = cursor
        .read(world.resource::<Messages<MouseWheel>>())
        .cloned()
        .collect();
    for event in events {
        if let Some(target) = target(world, event.window, false) {
            let window = event.window;
            world.trigger(FocusedInput::new(target, event, window));
        }
    }
}

#[cfg(test)]
mod tests;
