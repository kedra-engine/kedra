//! Typed discrete proposals over immutable bindings. Hosts retain source authority.
use crate::{
    binding::{BindingToken, ValueBinding},
    editors::EditorContext,
};
use bevy::prelude::*;
use kedra_ui::fields::{CommandView, FieldAccess, FieldTemplates};
use std::sync::Arc;
mod variants;
pub use variants::enum_choices;

/// Finite native bool/unit-enum proposals. Payload variants require constructors
/// and are deliberately not guessed from display text.
pub fn finite_choices(binding: ValueBinding<'_>) -> Option<Vec<(String, Arc<dyn PartialReflect>)>> {
    let labels = if binding.value.try_downcast_ref::<bool>().is_some() {
        vec!["false".to_owned(), "true".to_owned()]
    } else if let Some(bevy::reflect::TypeInfo::Enum(schema)) =
        binding.value.get_represented_type_info()
    {
        if !schema
            .iter()
            .all(|variant| matches!(variant, bevy::reflect::enums::VariantInfo::Unit(_)))
        {
            return None;
        }
        schema
            .iter()
            .map(|variant| variant.name().to_owned())
            .collect()
    } else {
        return None;
    };
    let empty = bevy::reflect::ParsedPath::from(Vec::<bevy::reflect::Access<'static>>::new());
    let field = crate::fields::ReflectedField::bind(
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        binding,
        &empty,
    )
    .ok()?;
    labels
        .into_iter()
        .map(|label| {
            field
                .propose(&label)
                .ok()
                .map(|value| (label, Arc::from(value)))
        })
        .collect()
}

/// Immutable proof of the exact retained control/binding which produced a proposal.
/// A refresh replaces this identity; delayed popup actions cannot borrow a new target.
pub struct ControlBinding {
    /// Consumer scope identity, not a source identity.
    pub token: BindingToken,
    /// Native path relative to the inspected component; empty means component root.
    pub path: bevy::reflect::ParsedPath,
    /// Shared property diagnostic target.
    pub diagnostic: Entity,
}
/// Presentation-only proposal endpoint. A host must additionally bind source revisions.
#[derive(Component, Clone)]
pub struct Control(pub Arc<ControlBinding>);
/// An exact native value request, never acknowledgment or permission to publish.
#[derive(EntityEvent, Clone)]
pub struct ControlEvent {
    /// Original retained control, not the currently focused replacement.
    pub entity: Entity,
    /// Original immutable binding identity.
    pub binding: Arc<ControlBinding>,
    /// Complete native proposed value. No reflection borrow escapes the callback.
    pub value: Arc<dyn PartialReflect>,
}
/// Native finite value choices, rendered by the shared authored command/menu kit.
pub struct ChoiceView {
    command: CommandView,
    templates: FieldTemplates,
    scope: String,
}
#[derive(Component, Clone)]
struct Choices {
    binding: Arc<ControlBinding>,
    values: Vec<(String, Arc<dyn PartialReflect>)>,
    templates: FieldTemplates,
    scope: String,
}
#[derive(Component, Clone)]
struct Item {
    owner: Entity,
    binding: Arc<ControlBinding>,
    value: Arc<dyn PartialReflect>,
    popup: Entity,
}
#[derive(Component)]
struct ChoicePopup;

impl ChoiceView {
    /// Mount a retained command in a normal property slot, reclaiming failed templates.
    pub fn mount(
        world: &mut World,
        parent: Entity,
        context: &EditorContext<'_>,
        scope: &str,
    ) -> Result<Self, String> {
        let command = CommandView::mount(world, parent, context.templates, scope)
            .map_err(|e| e.to_string())?;
        kedra_ui::docking::class(world, command.root(), "property-choice", true);
        command.choice_markers(world);
        Ok(Self {
            command,
            templates: context.templates.clone(),
            scope: scope.into(),
        })
    }
    /// Retained native control identity.
    pub fn root(&self) -> Entity {
        self.command.root()
    }
    /// Supply valid typed choices. Construction/validation is the caller's responsibility.
    /// A fresh immutable identity revokes menus made against a preceding value/revision.
    pub fn update(
        &self,
        world: &mut World,
        binding: ValueBinding<'_>,
        diagnostic: Entity,
        label: &str,
        values: Vec<(String, Arc<dyn PartialReflect>)>,
    ) -> Result<(), String> {
        if values.len() > 128 {
            return Err("Choice menu exceeds 128 variants".into());
        }
        let current = Arc::new(ControlBinding {
            token: binding.token,
            path: binding.path.cloned().unwrap_or_else(|| {
                bevy::reflect::ParsedPath::from(Vec::<bevy::reflect::Access<'static>>::new())
            }),
            diagnostic,
        });
        self.command
            .update(
                world,
                if binding.value.try_downcast_ref::<bool>().is_some() {
                    ""
                } else {
                    label
                },
                !binding.access.is_read_only(),
            )
            .map_err(|e| e.to_string())?;
        world.entity_mut(self.root()).insert((
            Control(current.clone()),
            binding.access.clone(),
            Choices {
                binding: current,
                values,
                templates: self.templates.clone(),
                scope: self.scope.clone(),
            },
        ));
        Ok(())
    }
    /// Use a controlled native Checkbox; visual state changes only after host refresh.
    pub fn checkbox(&self, world: &mut World, checked: bool, label: &str) {
        if world
            .get::<bevy::ui_widgets::Checkbox>(self.root())
            .is_none()
        {
            world
                .entity_mut(self.root())
                .remove::<(bevy::ui_widgets::Button, bevy::a11y::AccessibilityNode)>()
                .insert(bevy::ui_widgets::Checkbox);
        }
        if let Some(mut node) = world.get_mut::<bevy::a11y::AccessibilityNode>(self.root()) {
            node.set_label(label);
        }
        if checked {
            world.entity_mut(self.root()).insert(bevy::ui::Checked);
        } else {
            world.entity_mut(self.root()).remove::<bevy::ui::Checked>();
        }
        kedra_ui::docking::class(world, self.root(), "property-choice", false);
        kedra_ui::docking::class(world, self.root(), "property-checkbox", true);
        kedra_ui::docking::class(world, self.root(), "checked", checked);
    }
    /// Release only this command/menu subtree.
    pub fn retire(&self, world: &mut World) {
        self.command.retire(world);
    }
}

/// Install native Inspector control routing once per UI host. No source adapter or
/// project execution is installed. Native widget plugins remain the host's responsibility.
pub struct InspectorControlsPlugin;
impl Plugin for InspectorControlsPlugin {
    fn build(&self, app: &mut App) {
        crate::builtin::install(app);
        crate::collections::install(app);
        app.add_observer(activate)
            .add_observer(checkbox)
            .add_observer(menu_event);
    }
}
fn eligible(world: &World, owner: Entity, binding: &Arc<ControlBinding>) -> bool {
    world
        .get::<Control>(owner)
        .is_some_and(|current| Arc::ptr_eq(&current.0, binding))
        && world
            .get::<FieldAccess>(owner)
            .is_some_and(|access| !access.is_read_only())
        && world.get::<bevy::ui::InteractionDisabled>(owner).is_none()
}
fn activate(event: On<bevy::ui_widgets::Activate>, mut commands: Commands) {
    let entity = event.entity;
    commands.queue(move |world: &mut World| {
        if let Some(item) = world.get::<Item>(entity).cloned() {
            if eligible(world, item.owner, &item.binding) {
                world.trigger(ControlEvent {
                    entity: item.owner,
                    binding: item.binding,
                    value: item.value,
                });
            }
            if let Ok(popup) = world.get_entity_mut(item.popup) {
                popup.despawn();
            }
            return;
        }
        let Some(choices) = world.get::<Choices>(entity).cloned() else {
            return;
        };
        if !eligible(world, entity, &choices.binding) {
            return;
        }
        let prior = world
            .get::<Children>(entity)
            .map(|children| {
                children
                    .iter()
                    .filter(|child| world.get::<ChoicePopup>(*child).is_some())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !prior.is_empty() {
            for popup in prior {
                world.despawn(popup);
            }
            return;
        }
        let Ok(popup) = kedra_ui::popup::attach(
            world,
            entity,
            "property-menu",
            &format!("{}/menu", choices.scope),
        ) else {
            return;
        };
        world.entity_mut(popup).insert(ChoicePopup);
        for (index, (label, value)) in choices.values.into_iter().enumerate() {
            let scope = format!("{}/menu/{index}", choices.scope);
            let Ok(view) = CommandView::mount(world, popup, &choices.templates, &scope) else {
                world.despawn(popup);
                return;
            };
            if view.update(world, &label, true).is_err() {
                world.despawn(popup);
                return;
            }
            if let Some(color) = value.try_downcast_ref::<Color>() {
                view.color_marker(world, *color);
            }
            world
                .entity_mut(view.root())
                .remove::<(bevy::ui_widgets::Button, bevy::a11y::AccessibilityNode)>()
                .insert((
                    bevy::ui_widgets::MenuItem,
                    bevy::input_focus::tab_navigation::TabIndex(index as i32),
                    Item {
                        owner: entity,
                        binding: choices.binding.clone(),
                        value,
                        popup,
                    },
                ));
        }
    });
}
fn checkbox(
    event: On<bevy::ui_widgets::ValueChange<bool>>,
    controls: Query<&Control, With<bevy::ui_widgets::Checkbox>>,
    mut commands: Commands,
) {
    let Ok(control) = controls.get(event.source) else {
        return;
    };
    let (entity, binding, value) = (event.source, control.0.clone(), event.value);
    commands.queue(move |world: &mut World| {
        if eligible(world, entity, &binding) {
            world.trigger(ControlEvent {
                entity,
                binding,
                value: Arc::new(value),
            });
        }
    });
}
fn menu_event(
    event: On<bevy::ui_widgets::MenuEvent>,
    popups: Query<(), With<ChoicePopup>>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    let Some(popup) = std::iter::once(event.source)
        .chain(parents.iter_ancestors(event.source))
        .find(|entity| popups.contains(*entity))
    else {
        return;
    };
    let action = event.action;
    let owner = parents.get(popup).ok().map(ChildOf::parent);
    commands.queue(move |world: &mut World| match action {
        bevy::ui_widgets::MenuAction::FocusRoot => {
            if let Some(owner) = owner.filter(|owner| world.entities().contains(*owner))
                && let Some(mut focus) = world.get_resource_mut::<bevy::input_focus::InputFocus>()
            {
                focus.set(owner, bevy::input_focus::FocusCause::Navigated);
            }
        }
        bevy::ui_widgets::MenuAction::CloseAll => {
            if let Ok(root) = world.get_entity_mut(popup) {
                root.despawn();
            }
        }
        _ => {}
    });
}
