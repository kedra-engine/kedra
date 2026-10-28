//! Typed asset-reference presentation and bounded native picking. Hosts supply
//! exact logical paths and remain the sole source/type/delivery authority.
use crate::{
    binding::ValueBinding,
    controls::{Control, ControlBinding},
    editors::*,
};
use bevy::{
    asset::{AssetPath, UntypedHandle},
    prelude::*,
    reflect::TypeRegistry,
};
use kedra_assets::references::inspect as native;
use kedra_ui::fields::{CommandView, FieldAccess, FieldTemplates, SectionView, ValueView};
use std::{any::TypeId, sync::Arc};
mod picker;

/// A logical resource proposal under an original immutable native binding. `None`
/// requests Clear and is legal only for optional native fields. This event neither
/// loads assets nor acknowledges a source edit; consumers revalidate everything.
#[derive(EntityEvent, Clone)]
pub struct ReferenceEvent {
    /// Retained picker/drop endpoint which owns the original source binding.
    pub entity: Entity,
    /// Original full native field/path identity; never the currently selected row.
    pub binding: Arc<ControlBinding>,
    /// Exact source-qualified logical path/subasset, or optional Clear.
    pub path: Option<String>,
}
/// A native reference well over `Handle<T>` or `Option<Handle<T>>`. Register exact
/// native types with the existing editor registry; raw/default mode stays generic.
pub struct ReferenceEditor;
struct ReferenceView {
    root: SectionView,
    value: ValueView,
    picker: CommandView,
    templates: FieldTemplates,
    scope: String,
    diagnostic: Entity,
}
#[derive(Component, Clone)]
struct Reference {
    binding: Arc<ControlBinding>,
    choices: Vec<String>,
    optional: bool,
    templates: FieldTemplates,
    scope: String,
}
/// Native resource drop surface. A host may offer a logical Asset Browser path;
/// membership here controls preview styling only, not source publication rights.
#[derive(Component, Clone)]
pub struct ReferenceTarget {
    /// Source-bound retained picker endpoint used for a completed drop.
    pub endpoint: Entity,
    /// Original immutable binding; later field refreshes revoke it.
    pub binding: Arc<ControlBinding>,
    choices: Vec<String>,
}
impl ReferenceTarget {
    /// Whether this snapshot advertises an exact compatible logical path.
    pub fn accepts(&self, world: &World, path: &str) -> bool {
        self.drop_path(world, path).is_some()
    }
    /// Resolve an exact advertised resource, or a bare parent with exactly one
    /// advertised native subasset. Ambiguous parents require the searchable picker;
    /// never invent a Scene0 label. This is presentation admission, not source authority.
    pub fn drop_path(&self, world: &World, path: &str) -> Option<String> {
        eligible(world, self.endpoint, &self.binding)
            .then(|| drop_choice(&self.choices, path))
            .flatten()
    }
}
fn drop_choice(choices: &[String], path: &str) -> Option<String> {
    if let Some(exact) = choices.iter().find(|candidate| candidate.as_str() == path) {
        return Some(exact.clone());
    }
    let parent = AssetPath::try_parse(path).ok()?;
    if parent.label().is_some() {
        return None;
    }
    let mut matching = choices.iter().filter(|candidate| {
        AssetPath::try_parse(candidate).is_ok_and(|candidate| {
            candidate.label().is_some() && candidate.without_label() == parent
        })
    });
    let only = matching.next()?;
    matching.next().is_none().then(|| only.clone())
}
#[cfg(test)]
mod drop_tests;
/// Native asset type for a handle/optional handle or the explicitly supported
/// image-density-list constructor. Returns an error for unrelated shapes; it
/// does not infer a type from a filename or create a handle.
pub fn asset_type(value: &dyn PartialReflect, types: &TypeRegistry) -> Result<TypeId, String> {
    if value
        .try_downcast_ref::<Vec<kedra_ui::authored::components::UiImageDensity>>()
        .is_some()
    {
        return Ok(TypeId::of::<Image>());
    }
    Ok(native(value, types)?.handle.asset_type_id())
}
/// Current native dependency, or `None` for an empty optional reference/list. For
/// the image-list constructor, returns the final inserted image's dependency.
/// Hosts use this for readiness only; it does not prove source membership.
pub fn referenced_handle(
    value: &dyn PartialReflect,
    types: &TypeRegistry,
) -> Result<Option<UntypedHandle>, String> {
    if let Some(items) =
        value.try_downcast_ref::<Vec<kedra_ui::authored::components::UiImageDensity>>()
    {
        return Ok(items.last().map(|item| item.image.clone().untyped()));
    }
    Ok(native(value, types)?.current)
}

pub(crate) fn image_insertion_picker(
    world: &mut World,
    entity: Entity,
    binding: Arc<ControlBinding>,
    choices: Vec<String>,
    templates: &FieldTemplates,
    scope: &str,
) {
    world.entity_mut(entity).insert((
        Control(binding.clone()),
        Reference {
            binding,
            choices,
            optional: false,
            templates: templates.clone(),
            scope: scope.into(),
        },
    ));
}
pub(crate) fn clear_insertion_picker(world: &mut World, entity: Entity) {
    world.entity_mut(entity).remove::<(Control, Reference)>();
}
impl ValueEditor for ReferenceEditor {
    fn search_terms(&self) -> &'static [&'static str] {
        &["Asset", "Resource"]
    }
    fn mount(
        &self,
        world: &mut World,
        parent: Entity,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<Box<dyn ValueEditorView>, String> {
        let diagnostic = kedra_ui::fields::PropertyRow::diagnostic_for(world, parent)
            .ok_or("Resource reference needs an owning property row")?;
        let root = SectionView::mount(world, parent, context.templates, context.scope)
            .map_err(|e| e.to_string())?;
        let entity = root.root();
        kedra_ui::docking::class(world, entity, "property-composition", true);
        kedra_ui::docking::class(world, root.body(), "property-reference", true);
        let result = (|| {
            let value = ValueView::mount(
                world,
                root.body(),
                context.templates,
                &format!("{}/path", context.scope),
            )
            .map_err(|e| e.to_string())?;
            let picker = CommandView::mount(
                world,
                root.body(),
                context.templates,
                &format!("{}/picker", context.scope),
            )
            .map_err(|e| e.to_string())?;
            picker.choice_markers(world);
            kedra_ui::docking::class(world, picker.root(), "property-reference-picker", true);
            let mut view = ReferenceView {
                root,
                value,
                picker,
                templates: context.templates.clone(),
                scope: context.scope.into(),
                diagnostic,
            };
            view.update(world, context, binding)?;
            Ok(Box::new(view) as Box<dyn ValueEditorView>)
        })();
        if result.is_err()
            && let Ok(root) = world.get_entity_mut(entity)
        {
            root.despawn();
        }
        result
    }
}
impl ValueEditorView for ReferenceView {
    fn root(&self) -> Entity {
        self.root.root()
    }
    fn update(
        &mut self,
        world: &mut World,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<(), String> {
        let native = native(binding.value, context.types)?;
        let path = native.current.as_ref().and_then(|handle| {
            binding
                .metadata
                .and_then(|metadata| metadata.asset_path(handle))
        });
        let icon = if native.handle.asset_type_id() == TypeId::of::<Image>() {
            kedra_ui::fields::AssetIcon::Image
        } else {
            binding
                .metadata
                .map_or(kedra_ui::fields::AssetIcon::File, |metadata| {
                    metadata.asset_icon(native.handle.asset_type_id())
                })
        };
        self.value.asset_icon(world, icon);
        let display = path.as_deref().unwrap_or(if native.current.is_none() {
            "None"
        } else {
            "Unresolved resource"
        });
        self.value
            .update(
                world,
                display,
                &FieldAccess::ReadOnly {
                    reason: "Resource reference".into(),
                },
            )
            .map_err(|e| e.to_string())?;
        let choices = binding
            .metadata
            .map(|metadata| metadata.asset_choices(native.handle.asset_type_id()))
            .unwrap_or_default();
        let enabled =
            !binding.access.is_read_only() && (!choices.is_empty() || native.optional.is_some());
        self.picker
            .update(world, "Choose resource", enabled)
            .map_err(|e| e.to_string())?;
        // The owning property row supplies source diagnostics; the well itself
        // remains a native read-only path surface, never an editable UUID buffer.
        let diagnostic = self.diagnostic;
        let control = Arc::new(ControlBinding {
            token: binding.token,
            path: binding.path.cloned().unwrap_or_else(|| {
                bevy::reflect::ParsedPath::from(Vec::<bevy::reflect::Access<'static>>::new())
            }),
            diagnostic,
        });
        world.entity_mut(self.picker.root()).insert((
            binding.access.clone(),
            Control(control.clone()),
            Reference {
                binding: control,
                choices,
                optional: native.optional.is_some(),
                templates: self.templates.clone(),
                scope: self.scope.clone(),
            },
        ));
        let reference = world
            .get::<Reference>(self.picker.root())
            .ok_or("Resource field retired")?;
        let target = ReferenceTarget {
            endpoint: self.picker.root(),
            binding: reference.binding.clone(),
            choices: reference.choices.clone(),
        };
        world.entity_mut(self.root.root()).insert(target);
        kedra_ui::docking::class(world, self.root.root(), "property-reference-target", true);
        Ok(())
    }
}
fn eligible(world: &World, entity: Entity, binding: &Arc<ControlBinding>) -> bool {
    world
        .get::<Control>(entity)
        .is_some_and(|control| Arc::ptr_eq(&control.0, binding))
        && world
            .get::<FieldAccess>(entity)
            .is_some_and(|access| !access.is_read_only())
        && world.get::<bevy::ui::InteractionDisabled>(entity).is_none()
}
pub(crate) fn install(app: &mut App) {
    picker::install(app);
}
