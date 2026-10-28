//! Optional logical presentation attachments, never a source journal or value cache.
use super::*;
use kedra_inspector::collections::presentation::{self, Presentation};

#[derive(Clone)]
pub(crate) struct Context {
    target: Option<crate::documents::inspection::Target>,
    asset: Option<kedra_editor_context::AssetRef>,
    collections: BTreeMap<String, BTreeMap<String, Presentation>>,
    focused: Option<String>,
    pub had_focus: bool,
}
pub(super) struct Pending {
    context: Context,
    focus: bool,
    queued: bool,
    attempts: u8,
}
fn within(world: &World, mut entity: Entity, root: Entity) -> bool {
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
impl Panel {
    pub(super) fn prepare_history(&mut self, world: &mut World, id: ViewId) {
        if self.pending_history.as_ref().is_some_and(|pending| {
            !pending.queued
                && pending
                    .context
                    .collections
                    .iter()
                    .all(|(component, values)| {
                        values.is_empty() || self.components.contains_key(component)
                    })
        }) {
            self.history_tick(world, id);
        }
    }
    pub(super) fn history_waiting(&self) -> bool {
        self.pending_history
            .as_ref()
            .is_some_and(|pending| !pending.queued)
    }
    pub(crate) fn history_context(&self, world: &World, id: ViewId, proposed: bool) -> Context {
        let focus = world
            .get_resource::<bevy::input_focus::InputFocus>()
            .and_then(|focus| focus.get())
            .filter(|entity| within(world, *entity, self.root()));
        let prefix = format!("inspector/{id:?}/{}/", self.state.token);
        let focused = focus
            .filter(|entity| !presentation::owns_focus(world, *entity))
            .and_then(|entity| world.get::<kedra_ui::ElementId>(entity))
            .and_then(|id| id.0.strip_prefix(&prefix))
            .map(str::to_owned);
        Context {
            target: self.state.target.clone(),
            asset: self.state.asset.clone(),
            focused,
            had_focus: focus.is_some(),
            collections: self
                .components
                .iter()
                .map(|(key, tree)| {
                    (
                        key.clone(),
                        presentation::capture(world, tree.root(), proposed),
                    )
                })
                .collect(),
        }
    }
    pub(crate) fn restore_history_context(&mut self, context: Context, focus: bool) {
        self.pending_history = Some(Pending {
            context,
            focus,
            queued: false,
            attempts: 0,
        });
        self.dirty = true;
    }
    pub(super) fn history_tick(&mut self, world: &mut World, id: ViewId) {
        let Some(mut pending) = self.pending_history.take() else {
            return;
        };
        if self.state.target != pending.context.target || self.state.asset != pending.context.asset
        {
            return;
        }
        if !pending.queued {
            for (component, values) in &pending.context.collections {
                if let Some(tree) = self.components.get(component) {
                    presentation::restore(
                        world,
                        tree.root(),
                        values,
                        pending.focus && pending.context.had_focus,
                    );
                }
            }
            pending.queued = true;
            self.dirty = true;
            self.pending_history = Some(pending);
            return;
        }
        if !pending.focus {
            return;
        }
        if let Some(path) = &pending.context.focused {
            let full = format!("inspector/{id:?}/{}/{path}", self.state.token);
            let target = world.iter_entities().find_map(|entity| {
                (entity
                    .get::<kedra_ui::ElementId>()
                    .is_some_and(|id| id.0 == full)
                    && entity.contains::<kedra_ui::fields::TextField>()
                    && !entity.contains::<bevy::ui::InteractionDisabled>())
                .then_some(entity.id())
            });
            if let Some(target) = target {
                if let Some(mut focus) = world.get_resource_mut::<bevy::input_focus::InputFocus>() {
                    focus.set(target, bevy::input_focus::FocusCause::Navigated);
                }
            } else if pending.attempts < 8 {
                pending.attempts += 1;
                self.dirty = true;
                self.pending_history = Some(pending);
            }
        }
    }
}
