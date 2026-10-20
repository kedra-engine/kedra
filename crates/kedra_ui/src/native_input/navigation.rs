use super::*;
use bevy::{
    ecs::system::SystemState,
    input_focus::{
        FocusCause, InputFocusVisible,
        tab_navigation::{NavAction, TabGroup, TabIndex, TabNavigation},
    },
};

#[derive(Component)]
struct Gated {
    desired: TabIndex,
    written: TabIndex,
}

pub(super) fn gate(world: &mut World) {
    let indices: Vec<_> = world
        .query::<(Entity, &TabIndex, Option<&Gated>)>()
        .iter(world)
        .map(|(entity, index, gate)| {
            let desired = gate
                .filter(|g| g.written == *index)
                .map_or(*index, |g| g.desired);
            (entity, desired)
        })
        .collect();
    for (entity, desired) in indices {
        let written = if eligible(world, entity) && owner(world, entity).is_some() {
            desired
        } else {
            TabIndex(-1)
        };
        if world.get::<TabIndex>(entity) != Some(&written) {
            world.entity_mut(entity).insert(written);
        }
        world.entity_mut(entity).insert(Gated { desired, written });
    }
}
pub(super) fn under(world: &World, mut entity: Entity, root: Entity) -> bool {
    loop {
        if entity == root {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
}
pub(super) fn group(world: &mut World, window: Entity) -> Option<(Entity, bool)> {
    let groups: Vec<_> = world
        .query::<(Entity, &TabGroup)>()
        .iter(world)
        .map(|(e, g)| (e, *g))
        .collect();
    groups
        .into_iter()
        .filter(|(e, _)| owner(world, *e) == Some(window) && eligible(world, *e))
        .filter(|(e, g)| g.modal || world.get::<ChildOf>(*e).is_none())
        .max_by_key(|(e, g)| (g.modal, g.order, *e))
        .map(|(e, g)| (e, g.modal))
}

// Handle at the technical root, before bubbling reaches any Startup-only stock
// window observer. This works for every replacement/auxiliary root automatically.
type NativeRoots<'w, 's> =
    Query<'w, 's, (), (With<TabGroup>, With<UiTargetCamera>, Without<ChildOf>)>;
pub(super) fn tab(
    mut event: On<FocusedInput<KeyboardInput>>,
    roots: NativeRoots<'_, '_>,
    mut commands: Commands,
) {
    if event.input.key_code != KeyCode::Tab
        || !event.input.state.is_pressed()
        || event.input.repeat
        || !roots.contains(event.event_target())
    {
        return;
    }
    event.propagate(false);
    let window = event.input.window;
    commands.queue(move |world: &mut World| navigate(world, window));
}
fn navigate(world: &mut World, window: Entity) {
    let Some((root, modal)) = group(world, window) else {
        return;
    };
    let focus = world
        .resource::<InputFocus>()
        .get()
        .filter(|e| owner(world, *e) == Some(window) && (!modal || under(world, *e, root)));
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let direction = if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        NavAction::Previous
    } else {
        NavAction::Next
    };
    // Let Bevy enumerate all nonmodal groups (including groups below ordinary
    // wrappers), sort their order/indices and wrap. Its query is global, so hide
    // only foreign/ineligible indices for the duration of this synchronous call.
    // Restore the exact values before publishing focus; no hierarchy is rebuilt.
    let excluded: Vec<_> = world
        .query::<(Entity, &TabIndex)>()
        .iter(world)
        .filter(|(entity, index)| {
            index.0 >= 0 && (owner(world, *entity) != Some(window) || !eligible(world, *entity))
        })
        .map(|(entity, index)| (entity, *index))
        .collect();
    for (entity, _) in &excluded {
        world.entity_mut(*entity).insert(TabIndex(-1));
    }
    let mut state = SystemState::<TabNavigation>::new(world);
    let result = state.get(world).ok().and_then(|nav| {
        nav.navigate_internal(
            focus,
            direction,
            Some((root, &TabGroup { modal, order: 0 })),
        )
        .ok()
    });
    for (entity, index) in excluded {
        world.entity_mut(entity).insert(index);
    }
    if let Some(next) = result {
        world
            .resource_mut::<InputFocus>()
            .set(next, FocusCause::Navigated);
        world.resource_mut::<InputFocusVisible>().0 = true;
    }
}
