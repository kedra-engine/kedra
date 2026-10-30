//! The existing environment asset is inspected as one reflected source root.
use super::*;
use crate::documents::Documents;
use kedra_editor_context::AssetRef;
use kedra_inspector::{
    binding::{BindingToken, ValueBinding},
    controls::{Control, ControlEvent},
    editors::{EditorContext, RenderBudget},
    fields::ReflectedField,
};
use kedra_ui::{
    docking::class,
    fields::{FieldAccess, TextField, TextFieldEvent, TextFieldOutcome},
    text_input::TextSession,
};

#[derive(Component, Clone)]
pub(crate) struct Binding {
    pub view: ViewId,
    asset: AssetRef,
    revision: u64,
    field: Option<ReflectedField>,
    control: Option<std::sync::Arc<kedra_inspector::controls::ControlBinding>>,
}
impl Panel {
    pub(super) fn environment_assignment(
        &mut self,
        world: &mut World,
        id: ViewId,
        types: &bevy::reflect::TypeRegistry,
        choices: Vec<String>,
    ) -> Result<(), String> {
        if !self.dirty && self.components.contains_key("environment-assignment") {
            return Ok(());
        }
        struct Choices(Vec<String>);
        impl kedra_inspector::binding::ValueMetadata for Choices {
            fn describe(&self, _: Option<&bevy::reflect::ParsedPath>) -> Option<String> {
                None
            }
            fn asset_choices(&self, _: std::any::TypeId) -> Vec<String> {
                self.0.clone()
            }
        }
        let metadata = Choices(choices);
        let value: Option<Handle<kedra_scene::environment::Environment>> = None;
        let scope = format!(
            "inspector/{id:?}/{}/environment-assignment",
            self.state.token
        );
        let mut budget = RenderBudget::default();
        let mut context = EditorContext {
            templates: &self.templates,
            types,
            editors: &self.editors,
            budget: &mut budget,
            scope: &scope,
        };
        let access = if self.state.raw {
            FieldAccess::ReadOnly {
                reason: "Use default view to assign an environment".into(),
            }
        } else {
            FieldAccess::Editable
        };
        let binding = ValueBinding {
            token: BindingToken(self.state.token),
            value: &value,
            label: "Environment",
            access: &access,
            owner_type: None,
            path: None,
            diagnostic: None,
            metadata: Some(&metadata),
        };
        if let Some(tree) = self.components.get_mut("environment-assignment") {
            tree.update(world, &mut context, binding, self.state.raw)?;
        } else {
            let tree = TreeView::mount_filtered(
                world,
                self.element("entries")?,
                &mut context,
                binding,
                self.state.raw,
                &self.filter,
            )?;
            self.components
                .insert("environment-assignment".into(), tree);
        }
        Ok(())
    }
    pub(super) fn environment_tick(
        &mut self,
        world: &mut World,
        id: ViewId,
        documents: &mut Documents,
        asset: &AssetRef,
    ) -> Result<(), String> {
        let (value, revision) = match documents.inspect_environment(asset) {
            Ok(value) => value,
            Err(error) => {
                self.text(world, "message", &error)?;
                class(world, self.element("message")?, "hidden", false);
                class(world, self.element("entries")?, "hidden", true);
                return Ok(());
            }
        };
        class(world, self.element("entries")?, "hidden", false);
        let path = documents.environment_path(asset)?;
        let name = path.rsplit('/').next().unwrap_or(&path);
        self.text(
            world,
            "name",
            &format!(
                "{name}{}",
                if documents.environments.dirty(&path) {
                    " *"
                } else {
                    ""
                }
            ),
        )?;
        let registry = world.resource::<AppTypeRegistry>().clone();
        let types = registry.read();
        let scope = format!("inspector/{id:?}/{}/environment", self.state.token);
        let mut budget = RenderBudget::default();
        let mut context = EditorContext {
            templates: &self.templates,
            types: &types,
            editors: &self.editors,
            budget: &mut budget,
            scope: &scope,
        };
        let access = if self.state.raw {
            FieldAccess::ReadOnly {
                reason: "Use default view to edit source fields".into(),
            }
        } else {
            FieldAccess::Editable
        };
        let binding = ValueBinding {
            token: BindingToken(self.state.token),
            value: &value,
            label: "Environment",
            access: &access,
            owner_type: Some(std::any::TypeId::of::<kedra_scene::environment::Environment>()),
            path: None,
            diagnostic: None,
            metadata: None,
        };
        if self.dirty || self.edit_epoch != revision {
            if let Some(tree) = self.components.get_mut("environment") {
                tree.filter(&self.filter);
                tree.update(world, &mut context, binding, self.state.raw)?;
            } else {
                let tree = TreeView::mount_filtered(
                    world,
                    self.element("entries")?,
                    &mut context,
                    binding,
                    self.state.raw,
                    &self.filter,
                )?;
                self.components.insert("environment".into(), tree);
            }
            self.edit_epoch = revision;
        }
        let root = self.components["environment"].root();
        let fields: Vec<_> = world
            .query::<(Entity, &ReflectedField)>()
            .iter(world)
            .map(|(entity, field)| (entity, field.clone()))
            .collect();
        for (entity, field) in fields {
            if !world
                .query::<&ChildOf>()
                .query(world)
                .iter_ancestors(entity)
                .any(|e| e == root)
            {
                continue;
            }
            self.inputs.insert(entity);
            if world.get::<TextSession>(entity).is_none() {
                let session = TextSession::attach(
                    world
                        .get::<bevy::text::EditableText>(entity)
                        .ok_or("Environment field retired")?,
                    4096,
                )
                .map_err(|e| format!("Cannot bind environment field: {e:?}"))?;
                world
                    .entity_mut(entity)
                    .insert((session, bevy::ui::InteractionDisabled));
            }
            let ready = world
                .get::<TextSession>(entity)
                .is_some_and(|session| !session.pending())
                && world
                    .get::<bevy::text::EditableText>(entity)
                    .is_some_and(|text| {
                        text.pending_edits.is_empty()
                            && text.pending_paste.is_none()
                            && !text.is_composing()
                    });
            if !ready {
                continue;
            }
            if world.get::<TextField>(entity).is_none() {
                world
                    .entity_mut(entity)
                    .remove::<bevy::ui::InteractionDisabled>();
                TextField::attach(world, entity, revision)
                    .map_err(|e| format!("Cannot bind environment draft: {e:?}"))?;
            } else if world
                .get::<Binding>(entity)
                .is_some_and(|old| old.revision != revision)
            {
                TextField::refresh(world, entity, &field.display(), revision)
                    .map_err(|e| format!("Cannot refresh environment draft: {e:?}"))?;
            }
            world.entity_mut(entity).insert(Binding {
                view: id,
                asset: asset.clone(),
                revision,
                field: Some(field),
                control: None,
            });
        }
        let controls: Vec<_> = world
            .query::<(Entity, &Control)>()
            .iter(world)
            .map(|(e, c)| (e, c.0.clone()))
            .collect();
        for (entity, control) in controls {
            if !world
                .query::<&ChildOf>()
                .query(world)
                .iter_ancestors(entity)
                .any(|e| e == root)
            {
                continue;
            }
            world.entity_mut(entity).insert(Binding {
                view: id,
                asset: asset.clone(),
                revision,
                field: None,
                control: Some(control),
            });
        }
        self.inputs
            .retain(|entity| world.get::<Binding>(*entity).is_some());
        self.dirty = false;
        Ok(())
    }
    pub(crate) fn environment_input(
        &mut self,
        world: &mut World,
        documents: &mut Documents,
        event: TextFieldEvent,
    ) -> Result<(), String> {
        let Some(binding) = world.get::<Binding>(event.entity).cloned() else {
            return Ok(());
        };
        if self.state.asset.as_ref() != Some(&binding.asset) || !self.inputs.contains(&event.entity)
        {
            return Ok(());
        }
        let Some(field) = &binding.field else {
            return Ok(());
        };
        match event.outcome {
            TextFieldOutcome::Requested(request) => {
                let result = (|| {
                    if crate::workspace::import_pending(world)
                        || request.baseline() != binding.revision
                        || !TextField::is_current(world, event.entity, &request)
                        || world
                            .get::<FieldAccess>(event.entity)
                            .is_none_or(FieldAccess::is_read_only)
                        || documents.save_busy()
                        || documents.save_pending()
                        || documents.creating()
                        || documents.opening()
                    {
                        return Err("Environment field is stale or unavailable".into());
                    }
                    let path = documents.environment_path(&binding.asset)?;
                    let proposal = field.propose(request.value())?;
                    let mut candidate = documents.environments.clone();
                    candidate.edit(&path, binding.revision, &field.path, proposal.as_ref())?;
                    TextField::accept(world, event.entity, &request, binding.revision)
                        .map_err(|e| format!("Cannot accept environment field: {e:?}"))?;
                    documents.environments = candidate;
                    Ok(())
                })();
                if result.is_err() {
                    let _ = TextField::reject(world, event.entity, &request);
                }
                feedback(world, field.diagnostic, result);
            }
            TextFieldOutcome::Cancelled => feedback(world, field.diagnostic, Ok(())),
            TextFieldOutcome::HistoryRequested { redo } if !self.has_drafts(world) => {
                let path = documents.environment_path(&binding.asset)?;
                documents.environments.history(&path, redo)?;
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn environment_control(
        &mut self,
        world: &mut World,
        documents: &mut Documents,
        event: ControlEvent,
    ) {
        let Some(binding) = world.get::<Binding>(event.entity).cloned() else {
            return;
        };
        if self.state.asset.as_ref() != Some(&binding.asset)
            || binding
                .control
                .as_ref()
                .is_none_or(|old| !std::sync::Arc::ptr_eq(old, &event.binding))
        {
            return;
        }
        let result = if self.has_drafts(world)
            || crate::workspace::import_pending(world)
            || world
                .get::<FieldAccess>(event.entity)
                .is_none_or(FieldAccess::is_read_only)
        {
            Err("Accept or cancel the current draft before choosing another value".into())
        } else {
            documents.edit_environment(
                &binding.asset,
                binding.revision,
                &event.binding.path,
                event.value.as_ref(),
            )
        };
        feedback(world, event.binding.diagnostic, result);
    }
}
fn feedback(world: &mut World, diagnostic: Entity, result: Result<(), String>) {
    if let Some(mut text) = world.get_mut::<Text>(diagnostic) {
        text.0 = result.as_ref().err().cloned().unwrap_or_default();
    }
    class(world, diagnostic, "hidden", result.is_ok());
}
