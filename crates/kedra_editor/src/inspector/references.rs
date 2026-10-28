//! Resource requests retain their source binding while native dependencies load.
//! Readiness grants no source authority; every publication rechecks the original
//! target/revisions/type/delivery and uses the existing source transaction path.
use super::discrete::SourceControl;
use super::*;
use crate::documents::{Documents, ReferenceRequest, ResourceContext};
use kedra_inspector::{controls::Control, resources::ReferenceEvent};
use std::sync::Arc;
mod drop;
#[derive(Component)]
struct PendingReference {
    event: ReferenceEvent,
    handle: bevy::asset::UntypedHandle,
    since: f64,
}
pub(super) fn install(app: &mut App) {
    drop::install(app);
    app.add_observer(|event: On<ReferenceEvent>, mut commands: Commands| {
        let event = event.clone();
        commands.queue(move |world: &mut World| crate::workspace::property_reference(world, event));
    });
    app.add_systems(Update, poll);
    app.add_observer(cancel);
}
pub(super) fn pending(world: &World, root: Entity) -> bool {
    let view = world.get::<Binding>(root).map(|binding| binding.view);
    world.iter_entities().any(|entity| {
        entity.contains::<PendingReference>()
            && entity
                .get::<SourceControl>()
                .is_some_and(|source| Some(source.view) == view)
    })
}
impl Panel {
    pub(crate) fn property_reference(
        &mut self,
        world: &mut World,
        documents: &mut Documents,
        event: ReferenceEvent,
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
            if self.has_drafts(world) {
                return Err(
                    "Correct or cancel pending property input before assigning a resource".into(),
                );
            }
            if world
                .get::<kedra_ui::fields::FieldAccess>(event.entity)
                .is_none_or(kedra_ui::fields::FieldAccess::is_read_only)
                || world
                    .get::<bevy::ui::InteractionDisabled>(event.entity)
                    .is_some()
            {
                return Err("This resource field no longer authorizes editing".into());
            }
            let root = world
                .resource::<crate::model::ProjectManager>()
                .project
                .as_ref()
                .ok_or("No project is open")?
                .assets
                .clone();
            let server = world.resource::<AssetServer>().clone();
            let registry = world.resource::<AppTypeRegistry>().clone();
            let types = registry.read();
            let policies = world
                .resource::<kedra_scene::reflection::ComponentPolicies>()
                .clone();
            let value = documents.reference_value(
                ReferenceRequest {
                    target: &source.target,
                    expected: &source.expected,
                    component: &source.component,
                    field: &event.binding.path,
                    path: event.path.as_deref(),
                },
                &types,
                &policies,
                ResourceContext {
                    root: &root,
                    delivery: world.get_resource::<kedra_authoring_assets::Delivery>(),
                    server: &server,
                },
            )?;
            let Some(value) = value else {
                self.refresh_presentation();
                return Ok(());
            };
            if let Some(handle) =
                kedra_inspector::resources::referenced_handle(value.as_ref(), &types)?
                && !server.is_loaded_with_dependencies(handle.id())
            {
                let since = world.resource::<Time>().elapsed_secs_f64();
                world.entity_mut(event.entity).insert((
                    PendingReference {
                        event: event.clone(),
                        handle,
                        since,
                    },
                    bevy::ui::InteractionDisabled,
                ));
                return Ok(());
            }
            if kedra_inspector::resources::asset_type(value.as_ref(), &types)?
                == std::any::TypeId::of::<bevy::world_serialization::WorldAsset>()
                && let Some(handle) =
                    kedra_inspector::resources::referenced_handle(value.as_ref(), &types)?
            {
                kedra_preview::models::validate(
                    world,
                    &handle.typed::<bevy::world_serialization::WorldAsset>(),
                )?;
            }
            let change = documents.prepare_property(
                &source.target,
                &source.expected,
                &source.component,
                &[kedra_authoring::properties::PropertyEdit {
                    path: event.binding.path.clone(),
                    value: value.as_ref(),
                }],
                &types,
                &policies,
            )?;
            if !documents.change_current(&change) {
                return Err("Resource source changed before publication".into());
            }
            documents.publish_change(change);
            kedra_inspector::collections::resource_inserted(world, event.entity);
            self.refresh_presentation();
            Ok(())
        })();
        feedback(world, &source, event.entity, result.err());
        Ok(())
    }
}
fn feedback(world: &mut World, source: &SourceControl, entity: Entity, error: Option<String>) {
    if let Some(mut text) = world.get_mut::<Text>(source.binding.diagnostic) {
        text.0 = error.clone().unwrap_or_default();
    }
    kedra_ui::docking::class(world, source.binding.diagnostic, "hidden", error.is_none());
    kedra_ui::docking::class(world, entity, "invalid", error.is_some());
}
fn poll(world: &mut World) {
    let server = world.resource::<AssetServer>().clone();
    let now = world.resource::<Time>().elapsed_secs_f64();
    let ready = world
        .query::<(Entity, &PendingReference)>()
        .iter(world)
        .filter_map(|(entity, pending)| {
            let failure = world
                .get::<Control>(entity)
                .is_none_or(|control| !Arc::ptr_eq(&control.0, &pending.event.binding))
                || matches!(
                    server.get_load_state(pending.handle.id()),
                    Some(bevy::asset::LoadState::Failed(_))
                )
                || matches!(
                    server.get_recursive_dependency_load_state(pending.handle.id()),
                    Some(bevy::asset::RecursiveDependencyLoadState::Failed(_))
                )
                || now - pending.since > 30.;
            (failure || server.is_loaded_with_dependencies(pending.handle.id())).then_some((
                entity,
                pending.event.clone(),
                failure,
            ))
        })
        .collect::<Vec<_>>();
    for (entity, event, failed) in ready {
        world
            .entity_mut(entity)
            .remove::<(PendingReference, bevy::ui::InteractionDisabled)>();
        if failed {
            if let Some(source) = world.get::<SourceControl>(entity).cloned() {
                feedback(world, &source, entity, Some("Resource loading failed or timed out; check the matching delivery and choose again".into()));
            }
        } else {
            crate::workspace::property_reference(world, event);
        }
    }
}
fn cancel(
    event: On<bevy::input_focus::FocusedInput<bevy::input::keyboard::KeyboardInput>>,
    mut commands: Commands,
) {
    if event.input.logical_key != bevy::input::keyboard::Key::Escape
        || !event.input.state.is_pressed()
        || event.input.repeat
    {
        return;
    }
    let window = event.input.window;
    commands.queue(move |world: &mut World| {
        let requests = world
            .query_filtered::<Entity, With<PendingReference>>()
            .iter(world)
            .filter(|entity| kedra_ui::native_input::owner(world, *entity) == Some(window))
            .collect::<Vec<_>>();
        for entity in requests {
            world
                .entity_mut(entity)
                .remove::<(PendingReference, bevy::ui::InteractionDisabled)>();
        }
    });
}
