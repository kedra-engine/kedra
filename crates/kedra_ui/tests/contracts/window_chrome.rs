//! A non-editor consumer of reusable window-header bindings and native hit regions.
#![allow(clippy::unwrap_used, reason = "Isolated header contract assertions")]
use bevy::{camera::RenderTarget, math::Affine2, prelude::*, window::WindowRef};
use kedra_ui::window_chrome::{Header, HeaderRegion, RegionRole, hit};
use std::collections::BTreeMap;

fn surface(world: &mut World) -> (Entity, Entity) {
    let window = world.spawn(Window::default()).id();
    let camera = world
        .spawn((
            Camera::default(),
            RenderTarget::Window(WindowRef::Entity(window)),
        ))
        .id();
    let root = world
        .spawn((
            Node::default(),
            UiTargetCamera(camera),
            InheritedVisibility::VISIBLE,
        ))
        .id();
    (window, root)
}
fn node(world: &mut World, parent: Entity, center: Vec2, size: Vec2) -> Entity {
    world
        .spawn((
            Node::default(),
            InheritedVisibility::VISIBLE,
            ComputedNode {
                size,
                inverse_scale_factor: 1.,
                ..default()
            },
            UiGlobalTransform::from(Affine2::from_translation(center)),
            ChildOf(parent),
        ))
        .id()
}
#[test]
fn independent_consumer_attaches_and_detaches_without_editor() {
    let mut world = World::new();
    let (window, tree) = surface(&mut world);
    let root = node(&mut world, tree, Vec2::new(200., 16.), Vec2::new(400., 32.));
    let slot = node(&mut world, root, Vec2::new(100., 16.), Vec2::new(100., 32.));
    let content = world.spawn((Node::default(), ChildOf(tree))).id();
    let mut header = Header::bind(
        &mut world,
        window,
        root,
        BTreeMap::from([("tools".into(), slot)]),
    )
    .unwrap();
    header.attach(&mut world, "tools", content, tree).unwrap();
    assert_eq!(world.get::<ChildOf>(content).unwrap().parent(), slot);
    assert_eq!(header.detach(&mut world, "tools").unwrap(), Some(content));
    assert_eq!(world.get::<ChildOf>(content).unwrap().parent(), tree);
    header.attach(&mut world, "tools", content, tree).unwrap();
    header.unbind(&mut world).unwrap();
    world.despawn(root);
    assert!(world.entities().contains(content));
}
#[test]
fn rejected_bindings_preserve_attachments() {
    let mut world = World::new();
    let (window, tree) = surface(&mut world);
    let (_, other) = surface(&mut world);
    let root = node(&mut world, tree, Vec2::ZERO, Vec2::splat(32.));
    let slot = node(&mut world, root, Vec2::ZERO, Vec2::splat(32.));
    let content = world.spawn((Node::default(), ChildOf(tree))).id();
    let foreign = world.spawn((Node::default(), ChildOf(other))).id();
    let mut header = Header::bind(
        &mut world,
        window,
        root,
        BTreeMap::from([("tools".into(), slot)]),
    )
    .unwrap();
    assert!(header.attach(&mut world, "tools", foreign, tree).is_err());
    assert!(header.attach(&mut world, "missing", content, tree).is_err());
    assert!(header.attach(&mut world, "tools", tree, tree).is_err());
    assert_eq!(world.get::<ChildOf>(content).unwrap().parent(), tree);
    header.attach(&mut world, "tools", content, tree).unwrap();
    assert!(header.attach(&mut world, "tools", foreign, tree).is_err());
    assert_eq!(world.get::<ChildOf>(content).unwrap().parent(), slot);
}
#[test]
fn cycle_and_duplicate_window_rejected() {
    let mut world = World::new();
    let (window, tree) = surface(&mut world);
    let root = node(&mut world, tree, Vec2::ZERO, Vec2::splat(32.));
    let slot = node(&mut world, root, Vec2::ZERO, Vec2::splat(32.));
    assert!(
        Header::bind(
            &mut world,
            window,
            root,
            BTreeMap::from([("a".into(), slot), ("b".into(), slot)])
        )
        .is_err()
    );
    let mut header = Header::bind(&mut world, window, root, BTreeMap::new()).unwrap();
    assert!(Header::bind(&mut world, window, slot, BTreeMap::new()).is_err());
    header.unbind(&mut world).unwrap();
    assert!(Header::bind(&mut world, window, slot, BTreeMap::new()).is_ok());
}
#[test]
fn client_descendant_overrides_drag_even_when_disabled() {
    let mut world = World::new();
    let (window, tree) = surface(&mut world);
    let root = node(&mut world, tree, Vec2::new(200., 16.), Vec2::new(400., 32.));
    let _header = Header::bind(&mut world, window, root, BTreeMap::new()).unwrap();
    world.entity_mut(root).insert(HeaderRegion {
        header: root,
        role: RegionRole::Drag,
    });
    let button = node(&mut world, root, Vec2::new(50., 16.), Vec2::new(80., 26.));
    world
        .entity_mut(button)
        .insert((bevy::ui_widgets::Button, bevy::ui::InteractionDisabled));
    assert_eq!(
        hit(&world, window, Vec2::new(200., 16.)),
        Some(RegionRole::Drag)
    );
    assert_eq!(
        hit(&world, window, Vec2::new(50., 16.)),
        Some(RegionRole::Client)
    );
}
#[test]
fn hidden_clipped_stale_and_ambiguous_regions_do_not_drag() {
    let mut world = World::new();
    let (window, tree) = surface(&mut world);
    let root = node(&mut world, tree, Vec2::new(200., 16.), Vec2::new(400., 32.));
    let _header = Header::bind(&mut world, window, root, BTreeMap::new()).unwrap();
    world.entity_mut(root).insert(HeaderRegion {
        header: root,
        role: RegionRole::Drag,
    });
    assert_eq!(hit(&world, window, Vec2::splat(f32::NAN)), None);
    world.get_mut::<Node>(root).unwrap().display = Display::None;
    assert_eq!(hit(&world, window, Vec2::new(100., 16.)), None);
    world.get_mut::<Node>(root).unwrap().display = Display::Flex;
    world
        .entity_mut(root)
        .insert(CalculatedClip::default().with_rect(
            Rect::from_corners(Vec2::ZERO, Vec2::new(80., 32.)),
            &UiGlobalTransform::default(),
        ));
    assert_eq!(hit(&world, window, Vec2::new(100., 16.)), None);
    world.entity_mut(root).remove::<CalculatedClip>();
    let (_, other) = surface(&mut world);
    world.entity_mut(root).insert(ChildOf(other));
    assert_eq!(hit(&world, window, Vec2::new(100., 16.)), None);
}
#[test]
fn viewport_ui_scale_and_dpr_convert_once() {
    let mut world = World::new();
    world.insert_resource(UiScale(2.));
    let (window, tree) = surface(&mut world);
    let root = node(&mut world, tree, Vec2::new(100., 16.), Vec2::new(200., 32.));
    let _header = Header::bind(&mut world, window, root, BTreeMap::new()).unwrap();
    world
        .get_mut::<ComputedNode>(root)
        .unwrap()
        .inverse_scale_factor = 0.25;
    world.entity_mut(root).insert(HeaderRegion {
        header: root,
        role: RegionRole::Drag,
    });
    // Physical bounds [0, 200] × [0, 32], inverse layout scale 1/4,
    // UiScale=2: logical window bounds [0, 100] × [0, 16].
    assert_eq!(
        hit(&world, window, Vec2::new(50., 8.)),
        Some(RegionRole::Drag)
    );
    assert_eq!(hit(&world, window, Vec2::new(101., 8.)), None);
}

#[test]
fn modal_blocks_native_resize_outside_the_caption() {
    let mut world = World::new();
    let (window, tree) = surface(&mut world);
    let root = node(&mut world, tree, Vec2::new(200., 16.), Vec2::new(400., 32.));
    let _header = Header::bind(&mut world, window, root, BTreeMap::new()).unwrap();
    world.entity_mut(root).insert(HeaderRegion {
        header: root,
        role: RegionRole::Drag,
    });
    world.spawn((
        Node::default(),
        bevy::ui_widgets::ModalDialog,
        ChildOf(tree),
    ));
    assert_eq!(
        hit(&world, window, Vec2::new(2., 200.)),
        Some(RegionRole::Client)
    );
}

#[test]
fn detach_rejects_content_transferred_to_another_window() {
    let mut world = World::new();
    let (window, tree) = surface(&mut world);
    let (_, foreign) = surface(&mut world);
    let root = node(&mut world, tree, Vec2::ZERO, Vec2::splat(32.));
    let slot = node(&mut world, root, Vec2::ZERO, Vec2::splat(32.));
    let content = world.spawn((Node::default(), ChildOf(tree))).id();
    let mut header = Header::bind(
        &mut world,
        window,
        root,
        BTreeMap::from([("tools".into(), slot)]),
    )
    .unwrap();
    header.attach(&mut world, "tools", content, tree).unwrap();
    world.entity_mut(content).insert(ChildOf(foreign));
    assert!(header.detach(&mut world, "tools").is_err());
    assert_eq!(world.get::<ChildOf>(content).unwrap().parent(), foreign);
}

#[test]
fn caption_fitting_keeps_full_unicode_source_and_rejects_foreign_text() {
    let mut world = World::new();
    let (window, tree) = surface(&mut world);
    let root = node(&mut world, tree, Vec2::new(200., 16.), Vec2::new(400., 32.));
    let title = world.spawn((Text::new("before"), ChildOf(root))).id();
    let foreign = world.spawn(Text::new("foreign")).id();
    let header = Header::bind(&mut world, window, root, BTreeMap::new()).unwrap();
    header
        .set_title(&mut world, title, root, "Very long e\u{301}👩‍🚀 caption")
        .unwrap();
    assert_eq!(
        world.get::<Text>(title).unwrap().0,
        "Very long e\u{301}👩‍🚀 caption"
    );
    assert!(
        header
            .set_title(&mut world, foreign, root, "stolen")
            .is_err()
    );
    assert_eq!(world.get::<Text>(foreign).unwrap().0, "foreign");
}

#[test]
fn retired_handle_cannot_unbind_a_replacement_or_lose_its_borrowed_content() {
    let mut world = World::new();
    let (window, tree) = surface(&mut world);
    let root = node(&mut world, tree, Vec2::ZERO, Vec2::splat(32.));
    let slot = node(&mut world, root, Vec2::ZERO, Vec2::splat(32.));
    let slots = BTreeMap::from([("tools".into(), slot)]);
    let mut old = Header::bind(&mut world, window, root, slots.clone()).unwrap();
    old.unbind(&mut world).unwrap();
    let mut replacement = Header::bind(&mut world, window, root, slots).unwrap();
    let content = world.spawn((Node::default(), ChildOf(tree))).id();
    replacement
        .attach(&mut world, "tools", content, tree)
        .unwrap();
    assert!(old.unbind(&mut world).is_err());
    replacement.unbind(&mut world).unwrap();
    world.despawn(root);
    assert!(world.entities().contains(content));
    assert_eq!(world.get::<ChildOf>(content).unwrap().parent(), tree);
}

#[test]
fn foreign_camera_slots_are_rejected_before_bind_and_before_attachment() {
    let mut world = World::new();
    let (window, tree) = surface(&mut world);
    let (_, foreign) = surface(&mut world);
    let foreign_camera = world.get::<UiTargetCamera>(foreign).unwrap().0;
    let root = node(&mut world, tree, Vec2::ZERO, Vec2::splat(32.));
    let slot = node(&mut world, root, Vec2::ZERO, Vec2::splat(32.));
    let slots = BTreeMap::from([("tools".into(), slot)]);
    world
        .entity_mut(slot)
        .insert(UiTargetCamera(foreign_camera));
    assert!(Header::bind(&mut world, window, root, slots.clone()).is_err());
    world.entity_mut(slot).remove::<UiTargetCamera>();
    let mut header = Header::bind(&mut world, window, root, slots).unwrap();
    let content = world.spawn((Node::default(), ChildOf(tree))).id();
    world
        .entity_mut(slot)
        .insert(UiTargetCamera(foreign_camera));
    assert!(header.attach(&mut world, "tools", content, tree).is_err());
    assert_eq!(world.get::<ChildOf>(content).unwrap().parent(), tree);
}
