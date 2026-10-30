//! Revision-pinned source admission for discrete native proposals.
use super::*;
use crate::documents::{Documents, inspection::Target};
use kedra_inspector::controls::{Control, ControlBinding, ControlEvent};
use kedra_scene::document::DocumentId;
use std::sync::Arc;

#[derive(Component, Clone)]
pub(crate) struct SourceControl {
    pub view: ViewId,
    pub(super) target: Target,
    pub(super) expected: BTreeMap<DocumentId, u64>,
    pub(super) component: String,
    pub(super) binding: Arc<ControlBinding>,
}
pub(super) fn install(app: &mut App) {
    app.add_observer(|event: On<ControlEvent>, mut commands: Commands| {
        let event = event.clone();
        commands.queue(move |world: &mut World| crate::workspace::property_control(world, event));
    });
}
impl Panel {
    pub(super) fn sync_controls(
        &self,
        world: &mut World,
        id: ViewId,
        documents: &Documents,
    ) -> Result<(), String> {
        let Some(target) = self.state.target.clone() else {
            return Ok(());
        };
        let controls: Vec<_> = world
            .query::<(Entity, &Control)>()
            .iter(world)
            .map(|(entity, control)| (entity, control.0.clone()))
            .collect();
        for (entity, binding) in controls {
            let ancestors: Vec<_> = world
                .query::<&ChildOf>()
                .query(world)
                .iter_ancestors(entity)
                .collect();
            let Some(component) = self
                .components
                .iter()
                .find_map(|(key, tree)| ancestors.contains(&tree.root()).then(|| key.clone()))
            else {
                continue;
            };
            // Never rebase an unchanged control's delayed proposal onto newer sources.
            if world
                .get::<SourceControl>(entity)
                .is_some_and(|old| Arc::ptr_eq(&old.binding, &binding))
            {
                continue;
            }
            world.entity_mut(entity).insert(SourceControl {
                view: id,
                target: target.clone(),
                expected: documents.revisions(),
                component,
                binding,
            });
        }
        Ok(())
    }
    pub(crate) fn property_control(
        &mut self,
        world: &mut World,
        documents: &mut Documents,
        event: ControlEvent,
    ) -> Result<(), String> {
        let Some(source) = world.get::<SourceControl>(event.entity).cloned() else {
            return Ok(());
        };
        if self.state.target.as_ref() != Some(&source.target)
            || !Arc::ptr_eq(&source.binding, &event.binding)
            || world
                .get::<Control>(event.entity)
                .is_none_or(|control| !Arc::ptr_eq(&control.0, &event.binding))
        {
            return Ok(());
        }
        let result = (|| {
            if crate::workspace::import_pending(world) {
                return Err(
                    "Finish the asset filesystem operation before editing properties.".into(),
                );
            }
            if self.has_drafts_except(
                world,
                kedra_inspector::collections::proposal_owner(world, event.entity),
            ) {
                return Err(
                    "Accept or cancel the property draft before choosing another value.".to_owned(),
                );
            }
            if world
                .get::<kedra_ui::fields::FieldAccess>(event.entity)
                .is_none_or(kedra_ui::fields::FieldAccess::is_read_only)
                || world
                    .get::<bevy::ui::InteractionDisabled>(event.entity)
                    .is_some()
            {
                return Err("This control no longer authorizes source editing.".into());
            }
            let registry = world.resource::<AppTypeRegistry>().clone();
            let change = documents.prepare_property(
                &source.target,
                &source.expected,
                &source.component,
                &[kedra_authoring::properties::PropertyEdit {
                    path: event.binding.path.clone(),
                    value: event.value.as_ref(),
                }],
                &registry.read(),
                world.resource::<kedra_scene::reflection::ComponentPolicies>(),
            )?;
            if !documents.change_current(&change) {
                return Err("Source changed while validating the choice.".into());
            }
            documents.publish_change(change);
            self.refresh_presentation();
            Ok(())
        })();
        kedra_inspector::collections::respond(world, &event, result.is_ok());
        let diagnostic = source.binding.diagnostic;
        if let Some(mut text) = world.get_mut::<Text>(diagnostic) {
            text.0 = result.as_ref().err().cloned().unwrap_or_default();
        }
        kedra_ui::docking::class(world, diagnostic, "hidden", result.is_ok());
        kedra_ui::docking::class(world, event.entity, "invalid", result.is_err());
        Ok(())
    }
}
