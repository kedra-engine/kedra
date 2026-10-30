use super::*;
use bevy::{
    ecs::system::RunSystemOnce,
    render::sync_world::MainEntity,
    ui::ResolvedBorderRadius,
    ui_render::{ExtractedGlyph, ExtractedUiNode, NodeType},
};

fn clip(rect: Rect, transform: Affine2) -> CalculatedClip {
    let mut clip = CalculatedClip::default();
    clip.push_rect(rect, transform);
    clip
}

fn item(size: Vec2) -> ExtractedUiItem {
    ExtractedUiItem::Node {
        color: LinearRgba::WHITE,
        rect: Rect::from_center_size(Vec2::ZERO, size),
        atlas_scaling: None,
        flip_x: false,
        flip_y: false,
        border_radius: ResolvedBorderRadius::default(),
        border: bevy::sprite::BorderRect::ZERO,
        node_type: NodeType::Rect,
    }
}

#[test]
fn every_inherited_clip_and_actual_transformed_corner_must_contain_the_geometry() {
    let broad = Rect::from_center_size(Vec2::ZERO, Vec2::splat(100.));
    let mut inherited = clip(broad, Affine2::IDENTITY);
    assert!(contained(
        &item(Vec2::splat(20.)),
        Affine2::IDENTITY,
        &inherited
    ));
    inherited.push_rect(
        Rect::from_center_size(Vec2::ZERO, Vec2::splat(10.)),
        Affine2::IDENTITY,
    );
    assert!(!contained(
        &item(Vec2::splat(20.)),
        Affine2::IDENTITY,
        &inherited
    ));
    let transformed =
        Affine2::from_scale_angle_translation(Vec2::new(2., 0.5), 0.7, Vec2::new(35., 2.));
    assert!(contained(
        &item(Vec2::splat(8.)),
        transformed,
        &clip(broad, Affine2::IDENTITY)
    ));
    assert!(!contained(
        &item(Vec2::splat(25.)),
        transformed,
        &clip(broad, Affine2::IDENTITY)
    ));
    assert!(!contained(
        &item(Vec2::splat(8.)),
        Affine2::from_translation(Vec2::splat(f32::NAN)),
        &clip(broad, Affine2::IDENTITY)
    ));
}

#[test]
fn a_single_overhanging_glyph_preserves_the_entire_runs_clipping() {
    let bounds = clip(
        Rect::from_center_size(Vec2::ZERO, Vec2::splat(20.)),
        Affine2::IDENTITY,
    );
    let glyph = |x| ExtractedGlyph {
        color: LinearRgba::WHITE,
        translation: Vec2::new(x, 0.),
        // Nonzero atlas origin must not become a position in the UI.
        rect: Rect::from_corners(Vec2::splat(100.), Vec2::splat(104.)),
    };
    assert!(contained(
        &ExtractedUiItem::Glyphs {
            glyphs: vec![glyph(-3.), glyph(3.)]
        },
        Affine2::IDENTITY,
        &bounds
    ));
    assert!(!contained(
        &ExtractedUiItem::Glyphs {
            glyphs: vec![glyph(-3.), glyph(11.)]
        },
        Affine2::IDENTITY,
        &bounds
    ));
    assert!(!contained(
        &ExtractedUiItem::Glyphs { glyphs: vec![] },
        Affine2::IDENTITY,
        &bounds
    ));
}

#[test]
fn transformed_clip_regions_and_unbounded_axes_keep_bevys_containment_semantics() {
    let transform =
        Affine2::from_scale_angle_translation(Vec2::new(-1.5, 0.75), 0.6, Vec2::new(23.25, -10.5));
    let bounds = clip(
        Rect::from_center_size(Vec2::ZERO, Vec2::splat(20.)),
        transform.inverse(),
    );
    assert!(contained(&item(Vec2::splat(8.)), transform, &bounds));
    assert!(!contained(&item(Vec2::splat(24.)), transform, &bounds));
    let vertical = clip(
        Rect::from_corners(
            Vec2::new(f32::NEG_INFINITY, -10.),
            Vec2::new(f32::INFINITY, 10.),
        ),
        Affine2::IDENTITY,
    );
    assert!(contained(
        &item(Vec2::splat(8.)),
        Affine2::from_translation(Vec2::new(500., 0.)),
        &vertical
    ));
    assert!(!contained(
        &item(Vec2::splat(24.)),
        Affine2::IDENTITY,
        &vertical
    ));
}

#[test]
fn render_elision_keeps_main_world_clips_and_reconsiders_changed_geometry()
-> Result<(), Box<dyn std::error::Error>> {
    let original = clip(
        Rect::from_center_size(Vec2::ZERO, Vec2::splat(20.)),
        Affine2::IDENTITY,
    );
    let mut world = World::new();
    let main = world.spawn(original.clone()).id();
    let key = MainEntity::from(main);
    let render_entity = world.spawn_empty().id();
    let mut extracted = ExtractedUiNodes::default();
    let nodes = &mut extracted
        .uinodes
        .entry(key)
        .or_insert_with(|| (Entity::PLACEHOLDER, default()))
        .1;
    nodes.insert(
        render_entity,
        ExtractedUiNode {
            z_order: 0.,
            image: default(),
            clip: Some(original.clone()),
            item: item(Vec2::splat(10.)),
            transform: Affine2::IDENTITY,
        },
    );
    world.insert_resource(extracted);
    // Unchanged entries must not be visited again every frame.
    world
        .run_system_once(elide_contained_clips)
        .map_err(|e| format!("{e:?}"))?;
    assert!(
        world.resource::<ExtractedUiNodes>().uinodes[&key].1[&render_entity]
            .clip
            .is_some()
    );
    world.resource_mut::<ExtractedUiNodes>().changed.insert(key);
    world
        .run_system_once(elide_contained_clips)
        .map_err(|e| format!("{e:?}"))?;
    assert!(
        world.resource::<ExtractedUiNodes>().uinodes[&key].1[&render_entity]
            .clip
            .is_none()
    );
    assert_eq!(world.get::<CalculatedClip>(main), Some(&original));
    assert!(world.resource::<ExtractedUiNodes>().changed.contains(&key));
    // Bevy extraction replaces changed geometry and supplies its original clip.
    {
        let mut extracted = world.resource_mut::<ExtractedUiNodes>();
        let node = &mut extracted
            .uinodes
            .get_mut(&key)
            .ok_or("missing render group")?
            .1[&render_entity];
        node.transform = Affine2::from_translation(Vec2::new(8., 0.));
        node.clip = Some(original.clone());
    }
    world
        .run_system_once(elide_contained_clips)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        world.resource::<ExtractedUiNodes>().uinodes[&key].1[&render_entity].clip,
        Some(original)
    );
    // Fully hidden content never becomes unclipped.
    world
        .resource_mut::<ExtractedUiNodes>()
        .uinodes
        .get_mut(&key)
        .ok_or("missing group")?
        .1[&render_entity]
        .clip = Some(CalculatedClip::FullyClipped);
    world
        .run_system_once(elide_contained_clips)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        world.resource::<ExtractedUiNodes>().uinodes[&key].1[&render_entity].clip,
        Some(CalculatedClip::FullyClipped)
    );
    Ok(())
}
