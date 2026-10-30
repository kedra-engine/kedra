//! Source binding for native and scripted shared-field presentations.
use super::*;
use crate::documents::{Documents, PropertyChange, inspection::Target};
use kedra_authoring::workspace::EditGroup;
use kedra_inspector::builtin::{TransformField, TransformPart};
use kedra_inspector::fields::ReflectedField;
use kedra_scene::document::DocumentId;
use kedra_ui::{
    fields::{LiveTextField, TextField, TextFieldEvent, TextFieldOutcome},
    text_input::TextSession,
};

#[derive(Component, Clone)]
pub(crate) struct SourceField {
    pub view: ViewId,
    target: Target,
    expected: BTreeMap<DocumentId, u64>,
    epoch: u64,
    component: String,
    field: Field,
    error: Option<String>,
}
#[derive(Component, Clone)]
struct SourceGesture {
    session: u64,
    group: EditGroup,
}
#[derive(Clone)]
enum Field {
    Transform(TransformField),
    Reflected(ReflectedField),
}
impl Field {
    fn diagnostic(&self) -> Entity {
        match self {
            Self::Transform(field) => field.diagnostic,
            Self::Reflected(field) => field.diagnostic,
        }
    }
    fn display(&self) -> Result<String, String> {
        match self {
            Self::Transform(field) => field.display(),
            Self::Reflected(field) => Ok(field.display()),
        }
    }
    fn propose(
        &self,
        text: &str,
    ) -> Result<(bevy::reflect::ParsedPath, Box<dyn PartialReflect>), String> {
        match self {
            Self::Reflected(field) => Ok((field.path.clone(), field.propose(text)?)),
            Self::Transform(field) => {
                let transform = field.propose(text)?;
                let (path, value): (&str, Box<dyn PartialReflect>) = match field.part {
                    TransformPart::Translation => ("translation", Box::new(transform.translation)),
                    TransformPart::Rotation => ("rotation", Box::new(transform.rotation)),
                    TransformPart::Scale => ("scale", Box::new(transform.scale)),
                };
                Ok((
                    bevy::reflect::ParsedPath::parse(path).map_err(|error| error.to_string())?,
                    value,
                ))
            }
        }
    }
}
pub(super) fn install(app: &mut App) {
    app.add_observer(|event: On<TextFieldEvent>, mut commands: Commands| {
        let event = event.clone();
        commands.queue(move |world: &mut World| crate::workspace::property_input(world, event));
    });
}
impl Panel {
    pub(crate) fn end_edit_groups(&self, world: &mut World) {
        for input in &self.inputs {
            if let Ok(mut entity) = world.get_entity_mut(*input) {
                entity.remove::<SourceGesture>();
            }
        }
    }
    pub(crate) fn authorizes_input(&self, world: &World, input: Entity) -> bool {
        self.inputs.contains(&input)
            && world
                .get::<SourceField>(input)
                .is_some_and(|binding| self.state.target.as_ref() == Some(&binding.target))
    }
    pub(super) fn native_work_pending(&self, world: &World) -> bool {
        self.inputs.iter().any(|input| {
            world
                .get::<bevy::text::EditableText>(*input)
                .is_some_and(|text| !text.pending_edits.is_empty() || text.pending_paste.is_some())
                || world
                    .get::<TextSession>(*input)
                    .is_some_and(TextSession::pending)
        })
    }
    pub(crate) fn has_drafts(&self, world: &World) -> bool {
        self.has_drafts_except(world, None)
    }
    pub(super) fn has_drafts_except(&self, world: &World, except: Option<Entity>) -> bool {
        self.inputs
            .iter()
            .any(|input| TextField::unsettled(world, *input))
            || kedra_inspector::collections::unsettled(world, self.root(), except)
            || super::references::pending(world, self.root())
    }
    pub(super) fn sync_fields(
        &mut self,
        world: &mut World,
        id: ViewId,
        documents: &Documents,
    ) -> Result<(), String> {
        let Some(target) = self.state.target.clone() else {
            return Ok(());
        };
        let mut fields: Vec<_> = world
            .query::<(Entity, &TransformField)>()
            .iter(world)
            .map(|(input, field)| (input, Field::Transform(field.clone())))
            .collect();
        let reflected: Vec<_> = world
            .query::<(Entity, &ReflectedField)>()
            .iter(world)
            .filter(|(input, field)| {
                field.owner == *input || kedra_inspector_luau::ready(world, field)
            })
            .map(|(input, field)| (input, Field::Reflected(field.clone())))
            .collect();
        fields.extend(reflected);
        for (input, field) in fields {
            let ancestors: Vec<_> = world
                .query::<&ChildOf>()
                .query(world)
                .iter_ancestors(input)
                .collect();
            let Some(component) = self
                .components
                .iter()
                .find_map(|(key, tree)| ancestors.contains(&tree.root()).then(|| key.clone()))
            else {
                continue;
            };
            self.inputs.insert(input);
            let old = world.get::<SourceField>(input).cloned();
            if old.is_none() {
                let native = world
                    .get::<bevy::text::EditableText>(input)
                    .ok_or("Property input disappeared")?;
                let session = TextSession::attach(
                    native,
                    if matches!(field, Field::Transform(_)) {
                        64
                    } else {
                        4096
                    },
                )
                .map_err(|e| format!("Property input admission failed: {e:?}"))?;
                world
                    .entity_mut(input)
                    .insert((session, bevy::ui::InteractionDisabled));
            }
            if world.get::<TextField>(input).is_none() {
                let ready = world
                    .get::<bevy::text::EditableText>(input)
                    .zip(world.get::<TextSession>(input))
                    .is_some_and(|(text, session)| {
                        text.pending_edits.is_empty() && !session.pending() && !text.is_composing()
                    });
                if ready {
                    world
                        .entity_mut(input)
                        .remove::<bevy::ui::InteractionDisabled>();
                    TextField::attach(world, input, self.edit_epoch)
                        .map_err(|e| format!("Property input binding failed: {e:?}"))?;
                    world
                        .entity_mut(input)
                        .remove::<SourceGesture>()
                        .insert(LiveTextField::default());
                }
            } else if old.as_ref().is_some_and(|old| old.epoch != self.edit_epoch) {
                // The acknowledged native string may be a valid noncanonical
                // number ("1."). Echoing our own source update must not rewrite it
                // while the user is still typing. Foreign revisions use source text.
                let own_update = old
                    .as_ref()
                    .is_some_and(|old| old.expected == documents.revisions());
                let display = if own_update {
                    world
                        .get::<TextField>(input)
                        .ok_or("Property input disappeared")?
                        .accepted()
                        .to_owned()
                } else {
                    world.entity_mut(input).remove::<SourceGesture>();
                    field.display()?
                };
                TextField::refresh(world, input, &display, self.edit_epoch)
                    .map_err(|e| format!("Property refresh refused: {e:?}"))?;
            }
            let error = old.and_then(|old| old.error);
            world.entity_mut(input).insert(SourceField {
                view: id,
                target: target.clone(),
                expected: documents.revisions(),
                epoch: self.edit_epoch,
                component,
                field,
                error,
            });
        }
        self.inputs
            .retain(|input| world.get::<SourceField>(*input).is_some());
        let errors: Vec<_> = self
            .inputs
            .iter()
            .filter_map(|input| {
                world
                    .get::<SourceField>(*input)
                    .filter(|binding| binding.error.is_some())
                    .map(|_| *input)
            })
            .collect();
        for input in errors {
            feedback(world, input, None, false);
        }
        self.sync_controls(world, id, documents)?;
        Ok(())
    }
    pub(crate) fn property_input(
        &mut self,
        world: &mut World,
        documents: &mut Documents,
        event: TextFieldEvent,
    ) -> Result<(), String> {
        let Some(binding) = world.get::<SourceField>(event.entity).cloned() else {
            return Ok(());
        };
        if self.state.target.as_ref() != Some(&binding.target)
            || !self.inputs.contains(&event.entity)
        {
            return Ok(());
        }
        match event.outcome {
            TextFieldOutcome::Requested(request) => {
                let result: Result<(), String> = (|| {
                    if crate::workspace::import_pending(world) {
                        return Err(
                            "Finish the asset filesystem operation before editing properties."
                                .into(),
                        );
                    }
                    if request.baseline() != binding.epoch
                        || !TextField::is_current(world, event.entity, &request)
                    {
                        return Err(
                            "The property draft is stale. Press Escape and try again.".into()
                        );
                    }
                    if world
                        .get::<kedra_ui::fields::FieldAccess>(event.entity)
                        .is_none_or(kedra_ui::fields::FieldAccess::is_read_only)
                    {
                        return Err(
                            "This presentation no longer authorizes editing. Use default view."
                                .into(),
                        );
                    }
                    let (path, value) = binding.field.propose(request.value())?;
                    let registry = world.resource::<AppTypeRegistry>().clone();
                    let types = registry.read();
                    let policies = world.resource::<kedra_scene::reflection::ComponentPolicies>();
                    let edit = kedra_authoring::properties::PropertyEdit {
                        path,
                        value: value.as_ref(),
                    };
                    let mut gesture = match event.edit_session {
                        Some(session) => Some(match world.get::<SourceGesture>(event.entity) {
                            Some(old) if old.session == session => old.clone(),
                            _ => SourceGesture {
                                session,
                                group: documents.edit_group()?,
                            },
                        }),
                        None => None,
                    };
                    let change = documents.prepare_property_change(
                        PropertyChange {
                            target: &binding.target,
                            expected: &binding.expected,
                            component: &binding.component,
                            edits: &[edit],
                        },
                        gesture.as_mut().map(|gesture| &mut gesture.group),
                        &types,
                        policies,
                    )?;
                    if !documents.change_current(&change) {
                        return Err("Source changed while validating the property.".into());
                    }
                    // All source/compiler work is isolated and complete. Native acknowledgment
                    // can still refuse; publication afterward is an infallible synchronous swap.
                    TextField::accept(world, event.entity, &request, binding.epoch)
                        .map_err(|e| format!("Cannot acknowledge property: {e:?}"))?;
                    documents.publish_change(change);
                    if let Some(gesture) = gesture {
                        world.entity_mut(event.entity).insert(gesture);
                    }
                    if let Some(mut current) = world.get_mut::<SourceField>(event.entity) {
                        current.expected = documents.revisions();
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    let _ = TextField::reject(world, event.entity, &request);
                    feedback(world, event.entity, Some(error), true);
                } else {
                    feedback(world, event.entity, None, true);
                }
            }
            TextFieldOutcome::Cancelled => {
                world.entity_mut(event.entity).remove::<SourceGesture>();
                feedback(world, event.entity, None, true);
            }
            TextFieldOutcome::Refused(_) => feedback(
                world,
                event.entity,
                Some("The property draft was not accepted. Correct it or press Escape.".into()),
                true,
            ),
            // The workspace owns cross-panel history routing and draft guards.
            TextFieldOutcome::HistoryRequested { .. } => {}
        }
        Ok(())
    }
}
fn feedback(world: &mut World, input: Entity, message: Option<String>, replace: bool) {
    if replace && let Some(mut binding) = world.get_mut::<SourceField>(input) {
        binding.error = message;
    }
    let Some(binding) = world.get::<SourceField>(input).cloned() else {
        return;
    };
    if let Some(root) = world.get::<ChildOf>(input).map(ChildOf::parent) {
        kedra_ui::docking::class(world, root, "invalid", binding.error.is_some());
    }
    let message = world
        .query::<&SourceField>()
        .iter(world)
        .filter(|other| other.field.diagnostic() == binding.field.diagnostic())
        .find_map(|other| other.error.clone());
    if let Some(mut text) = world.get_mut::<Text>(binding.field.diagnostic()) {
        text.0 = message.clone().unwrap_or_default();
    }
    kedra_ui::docking::class(
        world,
        binding.field.diagnostic(),
        "hidden",
        message.is_none(),
    );
}
