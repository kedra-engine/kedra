//! Inline native length amount/unit controls at one owning source path.
use super::*;
use crate::{controls::ChoiceView, fields::ReflectedField};
use kedra_ui::fields::{FieldAccess, ValueView};
pub(super) struct LengthEditor;
struct LengthView {
    section: SectionView,
    amount: ValueView,
    unit: ChoiceView,
    diagnostic: Entity,
}
impl ValueEditor for LengthEditor {
    fn mount(
        &self,
        world: &mut World,
        parent: Entity,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<Box<dyn ValueEditorView>, String> {
        let diagnostic =
            PropertyRow::diagnostic_for(world, parent).ok_or("Length requires a base-owned row")?;
        let section = SectionView::mount(world, parent, context.templates, context.scope)
            .map_err(|e| e.to_string())?;
        class(world, section.root(), "property-composition", true);
        class(world, section.body(), "property-length", true);
        let root = section.root();
        let result = (|| {
            let amount = ValueView::mount(
                world,
                section.body(),
                context.templates,
                &format!("{}/amount", context.scope),
            )
            .map_err(|e| e.to_string())?;
            let unit = ChoiceView::mount(
                world,
                section.body(),
                context,
                &format!("{}/unit", context.scope),
            )?;
            let mut view = LengthView {
                section,
                amount,
                unit,
                diagnostic,
            };
            view.update(world, context, binding)?;
            Ok(Box::new(view) as Box<dyn ValueEditorView>)
        })();
        if result.is_err()
            && let Ok(entity) = world.get_entity_mut(root)
        {
            entity.despawn();
        }
        result
    }
}
impl ValueEditorView for LengthView {
    fn root(&self) -> Entity {
        self.section.root()
    }
    fn update(
        &mut self,
        world: &mut World,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<(), String> {
        let value = *binding
            .value
            .try_downcast_ref::<Val>()
            .ok_or("Native Val required")?;
        let field = ReflectedField::length(self.amount.input(), self.diagnostic, binding)?;
        let unavailable = FieldAccess::ReadOnly {
            reason: "Auto has no numeric amount; choose a unit first.".into(),
        };
        self.amount
            .update(
                world,
                &field.display(),
                if value == Val::Auto {
                    &unavailable
                } else {
                    binding.access
                },
            )
            .map_err(|e| e.to_string())?;
        self.amount.describe(world, binding.label, "Native length amount; the selected unit is retained. Enter accepts; Escape cancels.").map_err(|e| e.to_string())?;
        world.entity_mut(self.amount.input()).insert(field);
        let choices = crate::controls::enum_choices(binding.value, context.types)
            .ok_or("Native length constructors unavailable")?;
        let label = match value {
            Val::Auto => "Auto",
            Val::Px(_) => "Px",
            Val::Percent(_) => "%",
            Val::Vw(_) => "Vw",
            Val::Vh(_) => "Vh",
            Val::VMin(_) => "VMin",
            Val::VMax(_) => "VMax",
            Val::Em(_) => "Em",
            Val::Rem(_) => "Rem",
        };
        self.unit
            .update(world, binding, self.diagnostic, label, choices)
    }
}
