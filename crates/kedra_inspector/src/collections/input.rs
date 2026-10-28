//! Structural actions wait for native blur, then emit one immutable complete value.
use super::*;

#[derive(Message, Clone)]
pub(super) struct ActionRequest {
    entity: Entity,
    action: Action,
    binding: Arc<crate::controls::ControlBinding>,
}
pub(super) fn action(event: On<bevy::ui_widgets::Activate>, mut commands: Commands) {
    let entity = event.entity;
    commands.queue(move |world: &mut World| {
        let Some(action) = world.get::<Action>(entity).cloned() else {
            return;
        };
        if !writable(world, entity) {
            return;
        }
        let Some(binding) = world
            .get::<Control>(action.owner)
            .map(|control| control.0.clone())
        else {
            return;
        };
        let Some(mut state) = world.get_mut::<State>(action.owner) else {
            return;
        };
        if matches!(action.kind, Operation::Select(_)) && state.drag.is_some() {
            return;
        }
        if state.queued >= 128 {
            return;
        }
        state.queued += 1;
        world.write_message(ActionRequest {
            entity,
            action,
            binding,
        });
    });
}
pub(super) fn actions(mut requests: MessageReader<ActionRequest>, mut commands: Commands) {
    for request in requests.read().cloned() {
        commands.queue(move |world: &mut World| execute(world, request));
    }
}
fn execute(world: &mut World, request: ActionRequest) {
    let ActionRequest {
        entity,
        action,
        binding,
    } = request;
    if let Some(mut state) = world.get_mut::<State>(action.owner) {
        state.queued = state.queued.saturating_sub(1);
    }
    let result: Result<(), String> = (|| {
        let state = world
            .get::<State>(action.owner)
            .ok_or("Collection retired")?;
        if state.revision != action.revision
            || world.get::<Action>(entity).is_none()
            || world
                .get::<Control>(action.owner)
                .is_none_or(|control| !Arc::ptr_eq(&binding, &control.0))
        {
            return Err("Collection action is stale; try again after refresh".into());
        }
        if let Operation::Expand(id) = action.kind {
            if let Some(mut state) = world.get_mut::<State>(action.owner)
                && !state.expanded.remove(&id)
            {
                state.expanded.insert(id);
            }
            world.trigger(Changed);
            return Ok(());
        }
        if !world
            .get::<FieldAccess>(action.owner)
            .is_some_and(|access| !access.is_read_only())
        {
            return Err("This collection no longer authorizes editing".into());
        }
        if let Operation::Select(id) = action.kind {
            if !state.ids.contains(&id) {
                return Err("Collection item retired".into());
            }
            if let Some(mut state) = world.get_mut::<State>(action.owner) {
                state.selected = Some(id);
            }
            world.trigger(Changed);
            return Ok(());
        }
        if unsettled(world, state.area, None) || state.pending.is_some() || state.queued > 0 {
            return Err(
                "Correct or cancel pending item text before changing the collection".into(),
            );
        }
        let registry = world.resource::<AppTypeRegistry>().clone();
        let candidate = value::structural(state, action.kind, &registry.read())?;
        let mut ids = state.ids.clone();
        let mut selected = state.selected;
        let focus;
        let next_id = state.next_id;
        match action.kind {
            Operation::Add => {
                ids.push(next_id);
                selected = Some(next_id);
                focus = Some(interaction::Focus::Item(next_id));
            }
            Operation::Remove(index) => {
                let removed = ids.remove(index);
                if selected == Some(removed) {
                    selected = ids.get(index).or_else(|| ids.last()).copied();
                }
                focus = Some(selected.map_or(interaction::Focus::Add, interaction::Focus::Item));
            }
            Operation::Move(from, to) => {
                let id = ids.remove(from);
                ids.insert(to, id);
                focus = Some(interaction::Focus::Item(id));
            }
            Operation::Select(_) | Operation::Expand(_) => {
                return Err("Structural collection action required".into());
            }
        }
        let next = next_id
            .checked_add(1)
            .ok_or("Collection item identity exhausted")?;
        let value: Arc<dyn PartialReflect> = Arc::from(candidate);
        let before = presentation::Presentation::capture(state);
        if let Some(mut state) = world.get_mut::<State>(action.owner) {
            state.next_id = next;
            state.pending = Some(Proposal {
                before,
                binding: binding.clone(),
                value: value.clone(),
                ids,
                selected,
                focus,
            });
        }
        feedback(world, action.owner, None);
        world.trigger(ControlEvent {
            entity: action.owner,
            binding,
            value,
        });
        Ok(())
    })();
    if let Err(error) = result {
        feedback(world, action.owner, Some(error));
    }
    world.trigger(Changed);
}
pub(super) fn reorder(
    world: &mut World,
    entity: Entity,
    owner: Entity,
    revision: u64,
    binding: Arc<crate::controls::ControlBinding>,
    from: usize,
    to: usize,
) {
    if from == to {
        return;
    }
    if let Some(mut state) = world.get_mut::<State>(owner) {
        state.queued += 1;
    }
    world.write_message(ActionRequest {
        entity,
        action: Action {
            owner,
            revision,
            kind: Operation::Move(from, to),
        },
        binding,
    });
}
