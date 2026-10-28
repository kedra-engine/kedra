//! Native math/Transform presentations over the same immutable reflected bindings.
use crate::{binding::ValueBinding, editors::*};
use bevy::{
    prelude::*,
    reflect::{Access, ParsedPath},
};
use kedra_ui::{
    docking::class,
    fields::{PropertyRow, SectionView, VectorView},
};
use std::{any::TypeId, sync::Arc};
mod color;
mod layout;
mod length;
mod transform_fields;
pub use transform_fields::{TransformField, TransformPart};
pub(crate) fn install(app: &mut App) {
    color::install(app);
    crate::resources::install(app);
}

/// Construct the engine's native-type editors. Hosts may add explicit field/type
/// registrations afterward; duplicate keys still fail rather than replacing an editor.
/// Default/raw mode bypasses these presentations and exposes native reflected data.
pub fn editors() -> Result<EditorRegistry, String> {
    let mut registry = EditorRegistry::default();
    for ty in [
        TypeId::of::<Vec2>(),
        TypeId::of::<Vec3>(),
        TypeId::of::<Vec4>(),
        TypeId::of::<Quat>(),
    ] {
        registry.register_type(ty, Arc::new(MathEditor))?;
    }
    registry.register_type(TypeId::of::<Transform>(), Arc::new(TransformEditor))?;
    registry.register_type(TypeId::of::<Color>(), Arc::new(color::ColorEditor))?;
    registry.register_type(TypeId::of::<Val>(), Arc::new(length::LengthEditor))?;
    for ty in [
        TypeId::of::<Handle<Image>>(),
        TypeId::of::<Option<Handle<Image>>>(),
    ] {
        registry.register_type(ty, Arc::new(crate::resources::ReferenceEditor))?;
    }
    registry.register_type(
        TypeId::of::<kedra_ui::authored::components::UiClasses>(),
        Arc::new(crate::collections::CollectionEditor(
            crate::collections::Kind::Classes,
        )),
    )?;
    registry.register_type(
        TypeId::of::<BoxShadow>(),
        Arc::new(crate::collections::CollectionEditor(
            crate::collections::Kind::Shadows,
        )),
    )?;
    for ty in [
        TypeId::of::<Vec<String>>(),
        TypeId::of::<Vec<ShadowStyle>>(),
        TypeId::of::<Vec<kedra_ui::authored::components::UiImageDensity>>(),
        TypeId::of::<Vec<GridTrack>>(),
        TypeId::of::<Vec<RepeatedGridTrack>>(),
    ] {
        registry.register_type(
            ty,
            Arc::new(crate::collections::CollectionEditor(
                crate::collections::Kind::List,
            )),
        )?;
    }
    // Native grid placement uses optional nonzero line/span values. Zero is not a
    // constructible payload; explicit native defaults avoid guessing invalid Some(0).
    macro_rules! optional_grid {
        ($($ty:ty),* $(,)?) => { $(registry.register_variants(TypeId::of::<Option<$ty>>(), Arc::new(|value| {
            let current = Option::<$ty>::from_reflect(value).ok_or("Native optional grid value required")?;
            let some = current.or_else(|| <$ty>::new(1)).ok_or("Native grid default unavailable")?;
            Ok(vec![("None".into(), Arc::new(None::<$ty>)), ("Some".into(), Arc::new(Some(some)))])
        }))?;)* };
    }
    optional_grid!(std::num::NonZeroU16, std::num::NonZeroI16);
    registry.register_type(
        TypeId::of::<kedra_ui::authored::components::UiLayout>(),
        Arc::new(layout::LayoutEditor),
    )?;
    Ok(registry)
}

struct MathEditor;
struct MathView(VectorView, Option<Entity>);

impl ValueEditor for MathEditor {
    fn mount(
        &self,
        world: &mut World,
        parent: Entity,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<Box<dyn ValueEditorView>, String> {
        let values = math_values(binding.value)?;
        let diagnostic = PropertyRow::diagnostic_for(world, parent);
        let mut view = MathView(
            VectorView::mount(
                world,
                parent,
                context.templates,
                context.scope,
                values.len(),
            )
            .map_err(|e| e.to_string())?,
            diagnostic,
        );
        if let Err(error) = view.update(world, context, binding) {
            view.retire(world);
            return Err(error);
        }
        Ok(Box::new(view))
    }
}
impl ValueEditorView for MathView {
    fn root(&self) -> Entity {
        self.0.root()
    }
    fn update(
        &mut self,
        world: &mut World,
        _: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<(), String> {
        let values = math_values(binding.value)?;
        let description = if binding.value.try_downcast_ref::<Quat>().is_some() {
            "Rotation in degrees, native Bevy YXZ order. Stored value remains a quaternion; use default view for XYZW."
        } else {
            "Native vector axes; exact source precision is preserved."
        };
        let provenance = binding
            .metadata
            .and_then(|metadata| metadata.describe(binding.path))
            .unwrap_or_default();
        let description = format!("{description}\n{provenance}");
        self.0
            .update(world, &values, binding.label, &description, binding.access)
            .map_err(|e| e.to_string())?;
        if let Some(diagnostic) = self.1
            && binding.value.try_downcast_ref::<Vec3>().is_some()
            && binding.path.is_some_and(|path| !path.0.is_empty())
        {
            for (axis, input) in self.0.inputs().enumerate() {
                world
                    .entity_mut(input)
                    .insert(crate::fields::ReflectedField::vector_axis(
                        input, diagnostic, binding, axis,
                    )?);
            }
        }
        describe_axes(
            world,
            &self.0,
            binding,
            None,
            binding.label,
            &description,
            binding.value.try_downcast_ref::<Quat>().is_some(),
        )
    }
}

fn math_values(value: &dyn PartialReflect) -> Result<Vec<String>, String> {
    if let Some(value) = value.try_downcast_ref::<Vec2>() {
        return Ok(value.to_array().map(|v| v.to_string()).into());
    }
    if let Some(value) = value.try_downcast_ref::<Vec3>() {
        return Ok(value.to_array().map(|v| v.to_string()).into());
    }
    if let Some(value) = value.try_downcast_ref::<Vec4>() {
        return Ok(value.to_array().map(|v| v.to_string()).into());
    }
    if let Some(value) = value.try_downcast_ref::<Quat>() {
        return Ok(rotation_degrees(*value)?
            .to_array()
            .map(|v| v.to_string())
            .into());
    }
    Err("Math editor requires its registered native value type".into())
}

// Bevy's camera convention: yaw Y, pitch X, roll Z. Normalize only the display
// copy within the source's unit tolerance; never quantize or write back a source.
fn rotation_degrees(value: Quat) -> Result<Vec3, String> {
    if !value.is_finite() || (value.length_squared() - 1.).abs() > 0.001 {
        return Err(
            "Rotation requires a finite unit quaternion; use default view to inspect the source"
                .into(),
        );
    }
    let (y, x, z) = value.normalize().to_euler(EulerRot::YXZ);
    Ok(Vec3::from_array([x, y, z].map(|angle| {
        let degrees = angle.to_degrees();
        if degrees == 0. { 0. } else { degrees }
    })))
}

struct TransformEditor;
struct TransformView {
    section: SectionView,
    fields: Vec<(PropertyRow, VectorView)>,
}
impl ValueEditor for TransformEditor {
    fn search_terms(&self) -> &'static [&'static str] {
        &["Position", "Rotation", "Scale"]
    }
    fn layout(&self) -> EditorLayout {
        EditorLayout::Block
    }
    fn title(&self) -> Option<&'static str> {
        Some("Transform")
    }
    fn mount(
        &self,
        world: &mut World,
        parent: Entity,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<Box<dyn ValueEditorView>, String> {
        let section = SectionView::mount(world, parent, context.templates, context.scope)
            .map_err(|e| e.to_string())?;
        class(world, section.root(), "property-composition", true);
        let mut view = TransformView {
            section,
            fields: Vec::new(),
        };
        let result = (|| {
            for name in ["translation", "rotation", "scale"] {
                let scope = format!("{}/{name}", context.scope);
                let row = PropertyRow::mount(world, view.section.body(), context.templates, &scope)
                    .map_err(|e| e.to_string())?;
                let values = VectorView::mount(world, row.editor(), context.templates, &scope, 3)
                    .map_err(|e| e.to_string())?;
                view.fields.push((row, values));
            }
            view.update(world, context, binding)
        })();
        if let Err(error) = result {
            view.retire(world);
            return Err(error);
        }
        Ok(Box::new(view))
    }
}
impl ValueEditorView for TransformView {
    fn root(&self) -> Entity {
        self.section.root()
    }
    fn update(
        &mut self,
        world: &mut World,
        _: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<(), String> {
        let value = binding
            .value
            .try_downcast_ref::<Transform>()
            .ok_or("Transform editor requires native Transform")?;
        let rotation = rotation_degrees(value.rotation)?;
        for ((row, values), (field, name, data, description)) in self.fields.iter_mut().zip([
            (
                "translation",
                "Position",
                value.translation,
                "Native local translation",
            ),
            (
                "rotation",
                "Rotation",
                rotation,
                "Local rotation in degrees, native Bevy YXZ order; source stores a quaternion",
            ),
            ("scale", "Scale", value.scale, "Native local scale"),
        ]) {
            row.update(world, name, None).map_err(|e| e.to_string())?;
            values
                .update(
                    world,
                    &data.to_array().map(|v| v.to_string()),
                    name,
                    description,
                    binding.access,
                )
                .map_err(|e| e.to_string())?;
            describe_axes(
                world,
                values,
                binding,
                Some(field),
                name,
                description,
                field == "rotation",
            )?;
            if !binding.access.is_read_only() {
                for (axis, input) in values.inputs().enumerate() {
                    world.entity_mut(input).insert(TransformField {
                        owner: self.section.root(),
                        diagnostic: row.diagnostic(),
                        token: binding.token,
                        part: match field {
                            "translation" => TransformPart::Translation,
                            "rotation" => TransformPart::Rotation,
                            _ => TransformPart::Scale,
                        },
                        axis,
                        value: *value,
                    });
                }
            }
        }
        Ok(())
    }
}

// Euler axes are derived from the whole quaternion, not bindings to quaternion
// x/y/z. Ordinary vector axes retain their actual native field provenance.
fn describe_axes(
    world: &mut World,
    view: &VectorView,
    binding: ValueBinding<'_>,
    field: Option<&'static str>,
    label: &str,
    description: &str,
    derived: bool,
) -> Result<(), String> {
    let Some(metadata) = binding.metadata else {
        return Ok(());
    };
    let reason = binding.access.description();
    let dimensions = view.inputs().count();
    for (axis, name) in ["x", "y", "z", "w"]
        .into_iter()
        .take(dimensions)
        .enumerate()
    {
        let mut accesses = binding
            .path
            .into_iter()
            .flat_map(|path| path.0.iter())
            .map(|item| item.access.clone())
            .collect::<Vec<_>>();
        if let Some(field) = field {
            accesses.push(Access::Field(field.into()));
        }
        if !derived {
            accesses.push(Access::Field(name.into()));
        }
        let path = ParsedPath::from(accesses);
        let provenance = metadata.describe(Some(&path)).unwrap_or_default();
        view.describe_axis(
            world,
            axis,
            &format!("{label} {}", name.to_uppercase()),
            &format!("{description}\n{reason}\n{provenance}"),
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transform_axis_proposals_preserve_untouched_values_and_reject_nonfinite_text()
    -> Result<(), String> {
        let baseline = Transform {
            translation: Vec3::new(1., 2., 3.),
            rotation: Quat::from_euler(EulerRot::YXZ, 0.7, -0.5, 0.3),
            scale: Vec3::new(2., 3., 4.),
        };
        let mut field = TransformField {
            owner: Entity::PLACEHOLDER,
            diagnostic: Entity::PLACEHOLDER,
            token: crate::binding::BindingToken(1),
            part: TransformPart::Translation,
            axis: 0,
            value: baseline,
        };
        let proposed = field.propose("12.5")?;
        assert_eq!(proposed.translation, Vec3::new(12.5, 2., 3.));
        assert_eq!(proposed.rotation, baseline.rotation);
        assert_eq!(proposed.scale, baseline.scale);
        for text in ["-", "NaN", "inf", "1e99"] {
            assert!(field.propose(text).is_err());
        }
        field.part = TransformPart::Rotation;
        assert_eq!(
            field.propose(&field.display()?)?.rotation,
            baseline.rotation
        );
        let proposed = field.propose("30")?;
        assert_eq!(proposed.translation, baseline.translation);
        assert_eq!(proposed.scale, baseline.scale);
        assert!((proposed.rotation.length_squared() - 1.).abs() < 0.00001);
        Ok(())
    }
    #[test]
    fn angle_projection_preserves_native_orientation_without_changing_the_source()
    -> Result<(), String> {
        let near_gimbal = std::f32::consts::FRAC_PI_2 - 0.0001;
        for (y, x, z) in [
            (0., 0., 0.),
            (0.7, -0.5, 0.3),
            (-2.1, near_gimbal, -0.8),
            (1.2, -near_gimbal, 2.2),
        ] {
            let source = Quat::from_euler(EulerRot::YXZ, y, x, z);
            let before = source.to_array();
            let angles = rotation_degrees(source)?;
            let restored = Quat::from_euler(
                EulerRot::YXZ,
                angles.y.to_radians(),
                angles.x.to_radians(),
                angles.z.to_radians(),
            );
            assert!(source.dot(restored).abs() > 0.99999);
            assert_eq!(source.to_array(), before);
            let opposite = rotation_degrees(-source)?;
            let restored = Quat::from_euler(
                EulerRot::YXZ,
                opposite.y.to_radians(),
                opposite.x.to_radians(),
                opposite.z.to_radians(),
            );
            assert!(source.dot(restored).abs() > 0.99999);
        }
        for invalid in [
            Quat::from_xyzw(0., 0., 0., 0.),
            Quat::from_xyzw(f32::NAN, 0., 0., 1.),
        ] {
            assert!(rotation_degrees(invalid).is_err());
        }
        Ok(())
    }
}
