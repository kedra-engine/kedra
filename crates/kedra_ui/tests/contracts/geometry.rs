//! Observation and docking use the same renderer-coordinate bounds.
use bevy::{math::Affine2, prelude::*};

#[test]
fn point_menu_uses_click_position_at_each_scale_and_rejects_unavailable_layout()
-> Result<(), String> {
    for scale in [1., 0.5] {
        let mut world = World::new();
        let parent = world
            .spawn((
                Node::default(),
                ComputedNode {
                    size: Vec2::new(400., 60.) / scale,
                    inverse_scale_factor: scale,
                    ..default()
                },
                UiGlobalTransform::from(Affine2::from_translation(Vec2::new(300., 150.) / scale)),
            ))
            .id();
        let point = kedra_ui::popup::attach_at(
            &mut world,
            parent,
            Vec2::new(170., 140.),
            "menu",
            "context",
        )?;
        let node = world
            .get::<Node>(point.root)
            .ok_or("position node missing")?;
        assert_eq!(node.left, Val::Px(70.));
        assert_eq!(node.top, Val::Px(20.));
        assert_eq!(
            world.get::<ChildOf>(point.menu).map(ChildOf::parent),
            Some(point.root)
        );
        assert!(world.get::<OverrideClip>(point.menu).is_some());
        let count = world.entities().count_spawned();
        assert!(
            kedra_ui::popup::attach_at(&mut world, parent, Vec2::NAN, "menu", "context").is_err()
        );
        assert_eq!(world.entities().count_spawned(), count);
        world.despawn(parent);
        assert!(!world.entities().contains(point.root));
        assert!(!world.entities().contains(point.menu));
        let count = world.entities().count_spawned();
        assert!(
            kedra_ui::popup::attach_at(&mut world, parent, Vec2::ZERO, "menu", "context").is_err()
        );
        assert_eq!(world.entities().count_spawned(), count);
    }
    Ok(())
}

#[test]
fn native_ui_04_bounds_transform_corners_before_converting_physical_to_logical()
-> Result<(), &'static str> {
    for scale in [1., 0.5, 1. / 3.] {
        let node = ComputedNode {
            size: Vec2::new(100., 20.),
            inverse_scale_factor: scale,
            ..default()
        };
        for (stretch, angle) in [
            (Vec2::ONE, 0.),
            (Vec2::new(2., 3.), 0.),
            (Vec2::splat(2.), std::f32::consts::FRAC_PI_2),
            (Vec2::new(-1.5, 0.75), 0.71),
        ] {
            let affine =
                Affine2::from_scale_angle_translation(stretch, angle, Vec2::new(300., 200.));
            let transform = UiGlobalTransform::from(affine);
            let points = [
                Vec2::new(-50., -10.),
                Vec2::new(-50., 10.),
                Vec2::new(50., -10.),
                Vec2::new(50., 10.),
            ]
            .map(|point| affine.transform_point2(point) * scale);
            let min = points
                .into_iter()
                .fold(Vec2::splat(f32::INFINITY), Vec2::min);
            let max = points
                .into_iter()
                .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max);
            let actual = kedra_ui::logical_rect(&node, &transform);
            for (got, expected) in actual.into_iter().zip([min.x, min.y, max.x, max.y]) {
                assert!(
                    (got - expected).abs() < 0.0001,
                    "got {actual:?}; expected {min:?}..{max:?}"
                );
            }
            let mut world = World::new();
            let entity = world.spawn((node, transform)).id();
            let rect = kedra_ui::docking::rect(&world, entity).ok_or("live layout")?;
            assert_eq!([rect.min.x, rect.min.y, rect.max.x, rect.max.y], actual);
            world.despawn(entity);
            assert!(kedra_ui::docking::rect(&world, entity).is_none());
        }
    }
    Ok(())
}
