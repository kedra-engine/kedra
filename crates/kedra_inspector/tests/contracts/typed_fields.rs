//! Exact native field proposals preserve values before host/source acknowledgment.
use bevy::{prelude::*, reflect::ParsedPath};
use kedra_inspector::{
    binding::{BindingToken, ValueBinding},
    fields::ReflectedField,
};
use kedra_ui::fields::FieldAccess;

fn binding(value: &dyn PartialReflect) -> ValueBinding<'_> {
    ValueBinding {
        token: BindingToken(1),
        value,
        label: "Value",
        access: &FieldAccess::Editable,
        owner_type: None,
        path: None,
        diagnostic: None,
        metadata: None,
    }
}
fn field(value: &dyn PartialReflect) -> Result<ReflectedField, String> {
    ReflectedField::bind(
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        binding(value),
        &ParsedPath::from(Vec::<bevy::reflect::Access<'static>>::new()),
    )
}

#[test]
fn numeric_proposals_preserve_native_width_and_reject_invalid_values() -> Result<(), String> {
    let exact = field(&u64::MAX)?;
    assert_eq!(exact.display(), u64::MAX.to_string());
    let proposal = exact.propose("18446744073709551614")?;
    assert_eq!(proposal.try_downcast_ref::<u64>(), Some(&(u64::MAX - 1)));
    for text in ["18446744073709551616", "-1", "1.0", ""] {
        assert!(exact.propose(text).is_err());
    }
    let small = field(&0_i8)?;
    assert_eq!(
        small.propose("-128")?.try_downcast_ref::<i8>(),
        Some(&i8::MIN)
    );
    for text in ["-129", "128"] {
        assert!(small.propose(text).is_err());
    }
    let number = field(&0.33333334_f32)?;
    assert_eq!(
        number.propose(&number.display())?.try_downcast_ref::<f32>(),
        Some(&0.33333334_f32)
    );
    for text in ["NaN", "inf", "-inf", "1e99", "-"] {
        assert!(number.propose(text).is_err());
    }
    assert!(field(&f32::NAN).is_err());
    assert_eq!(
        exact.display(),
        u64::MAX.to_string(),
        "Proposals never mutate the binding"
    );
    Ok(())
}

#[test]
fn vector_axis_proposals_replace_only_one_exact_axis_at_the_owning_field_path() -> Result<(), String>
{
    let value = Vec3::new(1., 0.33333334, -9.);
    let path = ParsedPath::parse("size").map_err(|e| e.to_string())?;
    let field = ReflectedField::vector_axis(
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        ValueBinding {
            path: Some(&path),
            ..binding(&value)
        },
        0,
    )?;
    let proposed = field.propose("4.5")?;
    assert_eq!(
        proposed.try_downcast_ref::<Vec3>(),
        Some(&Vec3::new(4.5, value.y, value.z))
    );
    assert_eq!(field.path, path);
    for invalid in ["NaN", "inf", "1e99"] {
        assert!(field.propose(invalid).is_err());
    }
    assert!(
        ReflectedField::vector_axis(Entity::PLACEHOLDER, Entity::PLACEHOLDER, binding(&value), 3)
            .is_err()
    );
    assert_eq!(value, Vec3::new(1., 0.33333334, -9.));
    Ok(())
}

#[test]
fn color_proposals_preserve_precision_representation_and_alpha_without_silent_clamping()
-> Result<(), String> {
    let path = ParsedPath::parse("color").map_err(|e| e.to_string())?;
    for color in [
        Color::srgba(0.1234567, 0.33333334, 0.9, 0.37),
        Color::linear_rgba(0.1234567, 0.33333334, 0.9, 0.37),
    ] {
        let field = ReflectedField::color(
            Entity::PLACEHOLDER,
            Entity::PLACEHOLDER,
            ValueBinding {
                path: Some(&path),
                ..binding(&color)
            },
        )?;
        assert!(field.display().starts_with('#') && !field.display()[1..].contains('#'));
        assert!(matches!(field.display().len(), 7 | 9));
        assert_eq!(
            field
                .propose(&field.display().to_ascii_lowercase())?
                .try_downcast_ref::<Color>(),
            Some(&color)
        );
        assert_eq!(
            field.propose(&field.display())?.try_downcast_ref::<Color>(),
            Some(&color)
        );
        let proposed = field.propose("#FF000080")?;
        let proposed = proposed.try_downcast_ref::<Color>().ok_or("Color")?;
        assert_eq!(
            std::mem::discriminant(proposed),
            std::mem::discriminant(&color)
        );
        let rgba = proposed.to_srgba();
        assert!((rgba.red - 1.).abs() < 0.000001);
        assert_eq!([rgba.green, rgba.blue, rgba.alpha], [0., 0., 128. / 255.]);
        for invalid in ["red", "#xyzxyz", "#FFF", "#FFFFFFFFF", "#💜0000"] {
            assert!(field.propose(invalid).is_err());
        }
        assert_eq!(
            field.propose(&field.display())?.try_downcast_ref::<Color>(),
            Some(&color)
        );
    }
    for color in [
        Color::linear_rgba(4., 0., 0., 1.),
        Color::srgba(f32::NAN, 0., 0., 1.),
    ] {
        assert!(
            ReflectedField::color(
                Entity::PLACEHOLDER,
                Entity::PLACEHOLDER,
                ValueBinding {
                    path: Some(&path),
                    ..binding(&color)
                }
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn unsupported_color_refresh_revokes_editing_without_losing_the_retained_value()
-> crate::support::TestResult {
    #[derive(Reflect)]
    struct Paint {
        color: Color,
    }
    let (mut app, parent, templates) = crate::support::fixture()?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    registry.write().register::<Paint>();
    let types = registry.read();
    let editors = kedra_inspector::builtin::editors()?;
    let mut budget = kedra_inspector::editors::RenderBudget::default();
    let mut context = kedra_inspector::editors::EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "color-contract",
    };
    let value = Paint {
        color: Color::srgb(0.25, 0.5, 0.75),
    };
    let mut tree = kedra_inspector::view::TreeView::mount(
        app.world_mut(),
        parent,
        &mut context,
        binding(&value),
        false,
    )?;
    let input = app
        .world_mut()
        .query::<(Entity, &ReflectedField)>()
        .iter(app.world())
        .find_map(|(entity, field)| {
            app.world()
                .get::<bevy::text::EditableText>(entity)
                .filter(|_| field.display().starts_with('#'))
                .map(|_| entity)
        })
        .ok_or("Color field")?;
    let before = app
        .world()
        .get::<bevy::text::EditableText>(input)
        .ok_or("Color text")?
        .value()
        .to_string();
    assert_eq!(before, "#4080BF");
    let unsupported = Paint {
        color: Color::linear_rgb(4., 0., 0.),
    };
    tree.update(app.world_mut(), &mut context, binding(&unsupported), false)?;
    assert!(
        app.world()
            .get::<FieldAccess>(input)
            .is_some_and(FieldAccess::is_read_only)
    );
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(input)
            .ok_or("Retained color text")?
            .value()
            .to_string(),
        before
    );
    tree.update(app.world_mut(), &mut context, binding(&value), false)?;
    assert_eq!(
        app.world().get::<FieldAccess>(input),
        Some(&FieldAccess::Editable)
    );
    tree.retire(app.world_mut());
    Ok(())
}

#[test]
fn rgba_channel_proposals_preserve_other_exact_channels_space_and_alpha() -> Result<(), String> {
    let path = ParsedPath::parse("color").map_err(|e| e.to_string())?;
    for color in [
        Color::srgba(0.1234567, 0.33333334, 0.8, 0.37),
        Color::linear_rgba(0.1234567, 0.33333334, 0.8, 0.37),
    ] {
        let current = ValueBinding {
            path: Some(&path),
            ..binding(&color)
        };
        for axis in 0..4 {
            let field = ReflectedField::color_channel(
                Entity::PLACEHOLDER,
                Entity::PLACEHOLDER,
                current,
                axis,
            )?;
            assert_eq!(
                field.propose(&field.display())?.try_downcast_ref::<Color>(),
                Some(&color)
            );
            let proposed = field.propose("0.25")?;
            let actual = *proposed.try_downcast_ref::<Color>().ok_or("Color")?;
            assert_eq!(
                std::mem::discriminant(&actual),
                std::mem::discriminant(&color)
            );
            let mut expected = [0.1234567, 0.33333334, 0.8, 0.37];
            expected[axis] = 0.25;
            let actual = if matches!(color, Color::LinearRgba(_)) {
                actual.to_linear().to_f32_array()
            } else {
                actual.to_srgba().to_f32_array()
            };
            assert_eq!(actual, expected);
            for invalid in ["NaN", "inf", "-0.1", "1.01", "-"] {
                assert!(field.propose(invalid).is_err());
            }
        }
        assert!(
            ReflectedField::color_channel(Entity::PLACEHOLDER, Entity::PLACEHOLDER, current, 4)
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn native_length_amounts_keep_units_and_reject_auto_and_nonfinite_drafts() -> Result<(), String> {
    let path = ParsedPath::parse("node.width").map_err(|e| e.to_string())?;
    for (value, expected) in [
        (Val::Px(0.33333334), Val::Px(-2.5)),
        (Val::Percent(25.), Val::Percent(-2.5)),
        (Val::Vw(12.), Val::Vw(-2.5)),
        (Val::Em(1.2), Val::Em(-2.5)),
        (Val::Rem(1.2), Val::Rem(-2.5)),
    ] {
        let field = ReflectedField::length(
            Entity::PLACEHOLDER,
            Entity::PLACEHOLDER,
            ValueBinding {
                path: Some(&path),
                ..binding(&value)
            },
        )?;
        assert_eq!(field.path, path);
        assert_eq!(
            field.propose(&field.display())?.try_downcast_ref::<Val>(),
            Some(&value)
        );
        assert_eq!(
            field.propose("-2.5")?.try_downcast_ref::<Val>(),
            Some(&expected)
        );
        for invalid in ["-", "NaN", "inf", "1e99"] {
            assert!(field.propose(invalid).is_err());
        }
    }
    let auto = ReflectedField::length(
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        ValueBinding {
            path: Some(&path),
            ..binding(&Val::Auto)
        },
    )?;
    assert_eq!(auto.display(), "Auto");
    assert!(auto.propose("2").is_err());
    assert!(
        ReflectedField::length(
            Entity::PLACEHOLDER,
            Entity::PLACEHOLDER,
            ValueBinding {
                path: Some(&path),
                ..binding(&Val::Px(f32::NAN))
            }
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn grid_nonzero_amounts_reject_zero_without_changing_the_native_baseline() -> Result<(), String> {
    let empty = ParsedPath::from(Vec::<bevy::reflect::Access<'static>>::new());
    let value = std::num::NonZeroU16::new(3).ok_or("Nonzero fixture")?;
    let field = ReflectedField::bind(
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        binding(&value),
        &empty,
    )?;
    assert_eq!(field.display(), "3");
    assert!(field.propose("0").is_err());
    assert!(field.propose("65536").is_err());
    assert_eq!(
        field
            .propose("4")?
            .try_downcast_ref::<std::num::NonZeroU16>()
            .map(|value| value.get()),
        Some(4)
    );
    assert_eq!(field.display(), "3");
    let native = Some(value);
    let registry = kedra_inspector::builtin::editors()?;
    let choices = registry
        .variant_choices(&native, &bevy::reflect::TypeRegistry::default())
        .ok_or("Native grid constructors")?;
    assert_eq!(
        Option::<std::num::NonZeroU16>::from_reflect(choices[0].1.as_ref()),
        Some(None)
    );
    assert_eq!(
        Option::<std::num::NonZeroU16>::from_reflect(choices[1].1.as_ref()),
        Some(native)
    );
    Ok(())
}
