//! Value-free optional host attachments. Source history is owned by the consumer;
//! these logical collection mappings are restored only for an explicit operation ID.
use super::*;
use std::collections::BTreeMap;

/// Opaque item mapping/selection/disclosure, with no source values or ECS handles.
/// The host must associate it with its own exact target, field and history identity.
#[derive(Clone)]
pub struct Presentation {
    ids: Vec<u64>,
    selected: Option<u64>,
    expanded: std::collections::BTreeSet<u64>,
    focus: Option<interaction::Focus>,
}
impl Presentation {
    pub(super) fn capture(state: &State) -> Self {
        Self {
            ids: state.ids.clone(),
            selected: state.selected,
            expanded: state.expanded.clone(),
            focus: state.focus.or(Some(
                state
                    .selected
                    .map_or(interaction::Focus::Add, interaction::Focus::Item),
            )),
        }
    }
    pub(super) fn proposed(state: &State, proposal: &Proposal) -> Self {
        let mut expanded = state.expanded.clone();
        if let Some(interaction::Focus::Item(id)) = proposal.focus {
            expanded.insert(id);
        }
        Self {
            ids: proposal.ids.clone(),
            selected: proposal.selected,
            expanded,
            focus: proposal.focus,
        }
    }
    pub(super) fn apply(self, state: &mut State, count: usize) {
        if self.ids.len() != count {
            return;
        }
        state.next_id = state.next_id.max(
            self.ids
                .iter()
                .copied()
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        );
        state.ids = self.ids;
        state.selected = self.selected;
        state.expanded = self.expanded;
        state.focus = self.focus;
        state.focus_ready = false;
    }
}
/// Capture by component-relative native field path. A pending structural proposal
/// supplies its exact before/after mapping; it never publishes or grants source access.
pub fn capture(world: &World, root: Entity, proposed: bool) -> BTreeMap<String, Presentation> {
    let focused = world
        .get_resource::<bevy::input_focus::InputFocus>()
        .and_then(|focus| focus.get());
    let focused_owner = focused.and_then(|mut entity| {
        loop {
            if world.get::<State>(entity).is_some() {
                break Some(entity);
            }
            entity = world.get::<ChildOf>(entity)?.parent();
        }
    });
    world
        .iter_entities()
        .filter(|entity| within(world, entity.id(), root))
        .filter_map(|entity| {
            let state = entity.get::<State>()?;
            let control = entity.get::<Control>()?;
            let mut snapshot = match &state.pending {
                Some(pending) if proposed => Presentation::proposed(state, pending),
                Some(pending) => pending.before.clone(),
                None => Presentation::capture(state),
            };
            if focused_owner != Some(entity.id()) {
                snapshot.focus = None;
            }
            Some((control.0.path.to_string(), snapshot))
        })
        .collect()
}
/// Whether a focused control belongs to a retained collection. Its item focus is
/// restored through the collection mapping rather than a stale indexed field ID.
pub fn owns_focus(world: &World, focused: Entity) -> bool {
    world
        .iter_entities()
        .any(|entity| entity.contains::<State>() && within(world, focused, entity.id()))
}
/// Queue explicit mappings for the next source refresh. Mismatched counts are
/// ignored; missing/closed views never prevent source Undo. `focus` must reflect
/// the host's active-view eligibility, not merely the existence of retained controls.
pub fn restore(
    world: &mut World,
    root: Entity,
    snapshots: &BTreeMap<String, Presentation>,
    focus: bool,
) {
    let owners: Vec<_> = world
        .iter_entities()
        .filter(|entity| entity.contains::<State>() && within(world, entity.id(), root))
        .filter_map(|entity| {
            snapshots
                .get(&entity.get::<Control>()?.0.path.to_string())
                .cloned()
                .map(|value| (entity.id(), value))
        })
        .collect();
    for (owner, mut value) in owners {
        if !focus {
            value.focus = None;
        }
        if let Some(mut state) = world.get_mut::<State>(owner) {
            state.restoration = Some(value);
        }
    }
}
