//! Retained native widget states; no source mutation and no per-frame tree rebuilding.
use super::components::*;
use bevy::{prelude::*, text::TextReadWriteMode, ui::InteractionDisabled};

type EditableInputs<'w, 's> = Query<
    'w,
    's,
    (
        Has<InteractionDisabled>,
        Option<&'static crate::fields::FieldAccess>,
        &'static mut TextReadWriteMode,
    ),
    With<UiEditable>,
>;

pub(super) fn editing(mut inputs: EditableInputs<'_, '_>) {
    for (disabled, access, mut mode) in &mut inputs {
        let next = if disabled || access.is_some() {
            TextReadWriteMode::ReadOnly
        } else {
            TextReadWriteMode::Editable
        };
        if *mode != next {
            *mode = next;
        }
    }
}

pub(super) fn images(
    images: Res<Assets<Image>>,
    mut nodes: Query<(
        &UiImage,
        &ComputedUiRenderTargetInfo,
        &mut ImageNode,
        Option<&mut PassiveUi>,
    )>,
    mut failure: ResMut<UiFailure>,
) {
    for (source, target, mut node, mut passive) in &mut nodes {
        let mut report = |error: String| {
            if let Some(local) = passive.as_mut() {
                local.error = Some(error);
            } else {
                failure.0 = Some(error);
            }
        };
        let density = target.scale_factor();
        let Some(variant) = source.variants.iter().find(|v| v.density >= density) else {
            report(format!(
                "authored image has no variant for density {density}"
            ));
            continue;
        };
        if !images.contains(variant.image.id()) {
            report("authored image dependency disappeared".into());
            continue;
        }
        if node.image != variant.image {
            node.image = variant.image.clone();
        }
    }
}

pub(super) fn spin(
    time: Res<Time<Real>>,
    motion: Res<UiMotion>,
    mut nodes: Query<(&UiSpin, &mut UiTransform)>,
) {
    for (spin, mut transform) in &mut nodes {
        let radians = if motion.reduced || spin.paused {
            0.
        } else {
            (time.elapsed_secs() / spin.seconds).fract() * std::f32::consts::TAU
        };
        transform.rotation = Rot2::radians(radians);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn passive_image_failure_cannot_fail_the_serving_ui() {
        let mut world = World::new();
        world.init_resource::<Assets<Image>>();
        world.init_resource::<UiFailure>();
        let missing = Handle::<Image>::Uuid(uuid::Uuid::new_v4(), std::marker::PhantomData);
        let entity = world
            .spawn((
                UiImage {
                    variants: vec![UiImageDensity {
                        density: 1.,
                        image: missing.clone(),
                    }],
                    color: Color::WHITE,
                },
                ImageNode::new(missing),
                ComputedUiRenderTargetInfo::default(),
                PassiveUi::default(),
            ))
            .id();
        world
            .run_system_once(images)
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(
            world
                .get::<PassiveUi>(entity)
                .is_some_and(|local| local.error.is_some())
        );
        assert!(world.resource::<UiFailure>().0.is_none());
    }
    #[test]
    fn editing_disabled_mode_changes_preserve_native_value() {
        let mut world = World::new();
        let field = world
            .spawn((
                UiEditable {
                    autofocus: false,
                    select_all: false,
                },
                bevy::text::EditableText::new("Draft 日本語"),
                TextReadWriteMode::Editable,
            ))
            .id();
        for disabled in [true, true, false, false] {
            if disabled {
                world.entity_mut(field).insert(InteractionDisabled);
            } else {
                world.entity_mut(field).remove::<InteractionDisabled>();
            }
            world
                .run_system_once(editing)
                .unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(
                world.get::<TextReadWriteMode>(field),
                Some(&if disabled {
                    TextReadWriteMode::ReadOnly
                } else {
                    TextReadWriteMode::Editable
                })
            );
            assert_eq!(
                world
                    .get::<bevy::text::EditableText>(field)
                    .map(|text| text.value().to_string()),
                Some("Draft 日本語".into())
            );
        }
    }

    #[test]
    fn image_density_and_disappeared_assets_report_failure_without_replacing_last_image() {
        let mut world = World::new();
        world.init_resource::<Assets<Image>>();
        world.init_resource::<UiFailure>();
        let first = world.resource_mut::<Assets<Image>>().add(Image::default());
        let second = world.resource_mut::<Assets<Image>>().add(Image::default());
        let entity = world
            .spawn((
                UiImage {
                    variants: vec![UiImageDensity {
                        density: 1.,
                        image: second.clone(),
                    }],
                    color: Color::WHITE,
                },
                ImageNode::new(first.clone()),
                ComputedUiRenderTargetInfo::default(),
            ))
            .id();
        world
            .run_system_once(images)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            world.get::<ImageNode>(entity).map(|node| &node.image),
            Some(&second)
        );
        assert!(world.resource::<UiFailure>().0.is_none());
        // A valid authored variant no longer covers the current native target.
        world
            .get_mut::<UiImage>(entity)
            .unwrap_or_else(|| panic!("image source"))
            .variants[0]
            .density = 0.5;
        world
            .run_system_once(images)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(
            world
                .resource::<UiFailure>()
                .0
                .as_deref()
                .is_some_and(|e| e.contains("no variant"))
        );
        assert_eq!(
            world.get::<ImageNode>(entity).map(|node| &node.image),
            Some(&second)
        );
        world
            .get_mut::<UiImage>(entity)
            .unwrap_or_else(|| panic!("image source"))
            .variants[0]
            .density = 1.;
        world.resource_mut::<Assets<Image>>().remove(second.id());
        world
            .run_system_once(images)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            world.resource::<UiFailure>().0.as_deref(),
            Some("authored image dependency disappeared")
        );
        assert_eq!(
            world.get::<ImageNode>(entity).map(|node| &node.image),
            Some(&second)
        );
    }
    #[test]
    fn spinner_turns_on_real_time_and_respects_pause_and_reduced_motion() {
        let mut world = World::new();
        world.init_resource::<Time<Real>>();
        world.init_resource::<UiMotion>();
        let spinner = world
            .spawn((
                UiSpin {
                    seconds: 1.,
                    paused: false,
                },
                UiTransform::default(),
            ))
            .id();
        world
            .resource_mut::<Time<Real>>()
            .update_with_duration(std::time::Duration::ZERO);
        world
            .resource_mut::<Time<Real>>()
            .update_with_duration(std::time::Duration::from_millis(250));
        world
            .run_system_once(spin)
            .unwrap_or_else(|e| panic!("{e}"));
        let rotation = world
            .get::<UiTransform>(spinner)
            .map(|t| t.rotation.as_radians())
            .unwrap_or_default();
        assert!((rotation - std::f32::consts::FRAC_PI_2).abs() < 0.0001);
        world
            .get_mut::<UiSpin>(spinner)
            .unwrap_or_else(|| panic!("spinner removed"))
            .paused = true;
        world
            .run_system_once(spin)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            world.get::<UiTransform>(spinner).map(|t| t.rotation),
            Some(Rot2::IDENTITY)
        );
        world
            .get_mut::<UiSpin>(spinner)
            .unwrap_or_else(|| panic!("spinner removed"))
            .paused = false;
        world.resource_mut::<UiMotion>().reduced = true;
        world
            .run_system_once(spin)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            world.get::<UiTransform>(spinner).map(|t| t.rotation),
            Some(Rot2::IDENTITY)
        );
    }
}
