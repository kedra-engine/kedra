//! Compact native color value, reusing the base-owned label and shared value well.
use super::*;
use crate::controls::ChoiceView;
use kedra_ui::fields::ValueView;

pub(super) struct ColorEditor;
struct ColorView {
    section: SectionView,
    value: ValueView,
    diagnostic: Entity,
    channels: Vec<(PropertyRow, ValueView)>,
    choices: Vec<(PropertyRow, ChoiceView)>,
}
#[derive(Component)]
struct Expansion(Entity);
pub(super) fn install(app: &mut App) {
    app.add_observer(
        |event: On<bevy::ui_widgets::Activate>,
         expansions: Query<&Expansion>,
         mut commands: Commands| {
            if let Ok(expansion) = expansions.get(event.entity) {
                let palette = expansion.0;
                commands.queue(move |world: &mut World| {
                    let hidden = world
                        .get::<Visibility>(palette)
                        .is_none_or(|visibility| *visibility != Visibility::Hidden);
                    if let Ok(mut entity) = world.get_entity_mut(palette) {
                        entity.insert(if hidden {
                            Visibility::Hidden
                        } else {
                            Visibility::Inherited
                        });
                    }
                    class(world, palette, "hidden", hidden);
                });
            }
        },
    );
}
impl ValueEditor for ColorEditor {
    fn mount(
        &self,
        world: &mut World,
        parent: Entity,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<Box<dyn ValueEditorView>, String> {
        let diagnostic = PropertyRow::diagnostic_for(world, parent)
            .ok_or("Color requires a base-owned property row")?;
        let section = SectionView::mount(world, parent, context.templates, context.scope)
            .map_err(|e| e.to_string())?;
        class(world, section.root(), "property-composition", true);
        let root = section.root();
        let result = (|| {
            let value = ValueView::mount(world, section.body(), context.templates, context.scope)
                .map_err(|e| e.to_string())?;
            let scope = format!("{}/palette", context.scope);
            let palette = SectionView::mount(world, section.body(), context.templates, &scope)
                .map_err(|e| e.to_string())?;
            class(world, palette.root(), "property-composition", true);
            class(world, palette.root(), "hidden", true);
            world.entity_mut(palette.root()).insert(Visibility::Hidden);
            if let Some(swatch) = value.swatch() {
                world.entity_mut(swatch).insert((
                    bevy::ui_widgets::Button,
                    bevy::input_focus::tab_navigation::TabIndex(0),
                    bevy::picking::hover::Hovered::default(),
                    Expansion(palette.root()),
                    kedra_ui::tooltip::Tooltip("Open color palette and exact RGBA channels".into()),
                ));
                if let Some(mut node) = world.get_mut::<bevy::a11y::AccessibilityNode>(swatch) {
                    node.set_label("Open color palette");
                }
            }
            let mut channels = Vec::new();
            for name in ["red", "green", "blue", "alpha"] {
                let scope = format!("{scope}/{name}");
                let row = PropertyRow::mount(world, palette.body(), context.templates, &scope)
                    .map_err(|e| e.to_string())?;
                let value = ValueView::mount(world, row.editor(), context.templates, &scope)
                    .map_err(|e| e.to_string())?;
                channels.push((row, value));
            }
            let mut choices = Vec::new();
            for name in ["space", "presets"] {
                let scope = format!("{scope}/{name}");
                let row = PropertyRow::mount(world, palette.body(), context.templates, &scope)
                    .map_err(|e| e.to_string())?;
                let value = ChoiceView::mount(world, row.editor(), context, &scope)?;
                choices.push((row, value));
            }
            Ok(ColorView {
                section,
                value,
                diagnostic,
                channels,
                choices,
            })
        })();
        let mut view = match result {
            Ok(view) => view,
            Err(error) => {
                if let Ok(entity) = world.get_entity_mut(root) {
                    entity.despawn();
                }
                return Err(error);
            }
        };
        if let Err(error) = view.update(world, context, binding) {
            view.retire(world);
            return Err(error);
        }
        Ok(Box::new(view))
    }
}
impl ValueEditorView for ColorView {
    fn root(&self) -> Entity {
        self.section.root()
    }
    fn update(
        &mut self,
        world: &mut World,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<(), String> {
        let controls = self.channels.len() + self.choices.len();
        if context.budget.reserve(controls) != controls {
            return Err("Color palette exceeds the shared Inspector control budget".into());
        }
        let color = *binding
            .value
            .try_downcast_ref::<Color>()
            .ok_or("Native Color required")?;
        let field =
            crate::fields::ReflectedField::color(self.value.input(), self.diagnostic, binding)?;
        self.value
            .color_sample(world, color)
            .map_err(|e| e.to_string())?;
        self.value
            .update(world, &field.display(), binding.access)
            .map_err(|e| e.to_string())?;
        let space = if matches!(color, Color::LinearRgba(_)) {
            "linear RGBA"
        } else {
            "sRGB RGBA"
        };
        self.value.describe(world, binding.label, &format!("Hex input is sRGB #RRGGBB or #RRGGBBAA (alpha). Stored representation: {space}. Enter accepts; Escape cancels. Unchanged input preserves exact source precision.")).map_err(|e| e.to_string())?;
        world.entity_mut(self.value.input()).insert(field);
        for (axis, ((row, value), label)) in self
            .channels
            .iter_mut()
            .zip(["R", "G", "B", "Alpha"])
            .enumerate()
        {
            row.update(world, label, None).map_err(|e| e.to_string())?;
            let field = crate::fields::ReflectedField::color_channel(
                value.input(),
                self.diagnostic,
                binding,
                axis,
            )?;
            value
                .update(world, &field.display(), binding.access)
                .map_err(|e| e.to_string())?;
            value
                .describe(
                    world,
                    label,
                    "Exact native channel in 0..=1; Enter accepts, Escape cancels",
                )
                .map_err(|e| e.to_string())?;
            world.entity_mut(value.input()).insert(field);
        }
        let (row, control) = &mut self.choices[0];
        row.update(world, "Space", None)
            .map_err(|e| e.to_string())?;
        control.update(
            world,
            binding,
            self.diagnostic,
            if matches!(color, Color::LinearRgba(_)) {
                "Linear RGBA"
            } else {
                "sRGB RGBA"
            },
            vec![
                ("sRGB RGBA".into(), Arc::new(Color::Srgba(color.to_srgba()))),
                (
                    "Linear RGBA".into(),
                    Arc::new(Color::LinearRgba(color.to_linear())),
                ),
            ],
        )?;
        let (row, control) = &mut self.choices[1];
        row.update(world, "Palette", None)
            .map_err(|e| e.to_string())?;
        let colors = [
            ("White", [1., 1., 1.]),
            ("Black", [0., 0., 0.]),
            ("Gray", [0.5, 0.5, 0.5]),
            ("Red", [1., 0., 0.]),
            ("Orange", [1., 0.5, 0.]),
            ("Yellow", [1., 1., 0.]),
            ("Green", [0., 1., 0.]),
            ("Cyan", [0., 1., 1.]),
            ("Blue", [0., 0., 1.]),
            ("Purple", [0.5, 0., 1.]),
        ];
        let alpha = color.to_srgba().alpha;
        control.update(
            world,
            binding,
            self.diagnostic,
            "Choose color",
            colors
                .into_iter()
                .map(|(name, [r, g, b])| {
                    let selected = Color::srgba(r, g, b, alpha);
                    (
                        name.into(),
                        Arc::new(if matches!(color, Color::LinearRgba(_)) {
                            Color::LinearRgba(selected.to_linear())
                        } else {
                            selected
                        }) as Arc<dyn PartialReflect>,
                    )
                })
                .collect(),
        )?;
        Ok(())
    }
}
