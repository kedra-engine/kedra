//! Regression boundary: selection must add presentation, never source styles.
use super::*;
use bevy::{gizmos::GizmoAsset, picking::Pickable};
use kedra_scene::document::{NodeAddress, NodeId};

fn native_app() -> (App, Entity) {
    use bevy::{
        asset::AssetPlugin,
        camera::{ComputedCameraValues, RenderTargetInfo},
    };
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::text::TextPlugin,
        bevy::ui::UiPlugin,
        bevy::transform::TransformPlugin,
    ));
    app.init_resource::<Assets<Image>>();
    app.init_resource::<Assets<TextureAtlasLayout>>();
    app.init_resource::<Assets<GizmoAsset>>();
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>();
    app.add_plugins(bevy::camera::visibility::VisibilityPlugin);
    ui_marker::install(&mut app);
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            Camera {
                computed: ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::new(1280, 720),
                        scale_factor: 1.,
                    }),
                    ..default()
                },
                ..default()
            },
        ))
        .id();
    (app, camera)
}

fn frame(world: &mut World) -> Result<Entity, String> {
    Ok(world
        .query::<(Entity, &BorderColor, &Pickable)>()
        .iter(world)
        .find(|(_, color, _)| color.top == Color::srgb_u8(177, 210, 255))
        .ok_or("Selection frame missing")?
        .0)
}

fn assert_frame_above(world: &World, overlay: Entity, source: Entity) -> Result<(), String> {
    let index = |entity| {
        world
            .get::<bevy::ui::ComputedStackIndex>(entity)
            .map(|index| index.0)
            .ok_or("Native stack index missing".to_owned())
    };
    assert!(
        index(overlay)? > index(source)?,
        "Authored Z order occluded the selection frame"
    );
    Ok(())
}

#[test]
fn ui_frame_is_above_authored_global_max_local_one() -> Result<(), String> {
    let (mut app, camera) = native_app();
    let source = app
        .world_mut()
        .spawn((
            Node {
                width: px(160),
                height: px(80),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            UiTargetCamera(camera),
            GlobalZIndex(i32::MAX),
            ZIndex(1),
        ))
        .id();
    let address: NodeAddress = NodeId::new().into();
    let mut projection = projection::Projection::pending(1);
    projection.nodes.insert(address.clone(), source);
    let mut marker = selection::Marker::default();
    marker.update(app.world_mut(), &projection, Some(&address))?;
    app.world_mut().run_schedule(PostUpdate);
    let overlay = frame(app.world_mut())?;
    assert_frame_above(app.world(), overlay, source)?;
    assert_eq!(
        app.world().get::<GlobalZIndex>(source),
        Some(&GlobalZIndex(i32::MAX))
    );
    assert_eq!(app.world().get::<ZIndex>(source), Some(&ZIndex(1)));
    Ok(())
}

#[test]
fn ui_frame_is_above_equal_z_sentinels_after_native_history_and_layout_updates()
-> Result<(), String> {
    let (mut app, camera) = native_app();
    let source = app
        .world_mut()
        .spawn((
            Node {
                width: px(160),
                height: px(80),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            UiTargetCamera(camera),
            GlobalZIndex(i32::MAX),
            ZIndex(0),
        ))
        .id();
    let address: NodeAddress = NodeId::new().into();
    let mut projection = projection::Projection::pending(1);
    projection.nodes.insert(address.clone(), source);
    let mut marker = selection::Marker::default();
    marker.update(app.world_mut(), &projection, Some(&address))?;
    app.world_mut().run_schedule(PostUpdate);
    let overlay = frame(app.world_mut())?;
    // Equal authored sentinels can be reordered by native change/history fields.
    // Also exercise both local MAX sentinels, so adding another Z sentinel is not a fix.
    for local in [0, i32::MAX, i32::MAX, 0] {
        app.world_mut().entity_mut(overlay).insert(ZIndex(local));
        app.world_mut().run_schedule(PostUpdate);
        app.world_mut()
            .entity_mut(source)
            .insert((GlobalZIndex(i32::MAX), ZIndex(local)));
        app.world_mut()
            .get_mut::<Node>(source)
            .ok_or("Source missing")?
            .width = px(240);
        app.world_mut().run_schedule(PostUpdate);
        assert_frame_above(app.world(), overlay, source)?;
        assert_eq!(
            app.world()
                .get::<ComputedNode>(overlay)
                .ok_or("Frame bounds missing")?
                .size,
            Vec2::new(240., 80.)
        );
        assert_eq!(
            app.world().get::<GlobalZIndex>(source),
            Some(&GlobalZIndex(i32::MAX))
        );
        assert_eq!(app.world().get::<ZIndex>(source), Some(&ZIndex(local)));
    }
    Ok(())
}

// Removing post-layout synchronization breaks this test even with a marker entity.
#[test]
fn ui_frame_tracks_native_nested_layout_affine_clip_and_cleanup_without_reselection()
-> Result<(), String> {
    for mixed in [false, true] {
        let (mut app, camera) = native_app();
        if mixed {
            app.world_mut().spawn(Camera3d::default());
        }
        let parent = app
            .world_mut()
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(100),
                    top: px(60),
                    width: px(400),
                    height: px(300),
                    overflow: Overflow::clip(),
                    ..default()
                },
                UiTargetCamera(camera),
            ))
            .id();
        let source = app
            .world_mut()
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(20),
                    top: px(30),
                    width: px(160),
                    height: px(80),
                    ..default()
                },
                UiTargetCamera(camera),
                ChildOf(parent),
            ))
            .id();
        let address: NodeAddress = NodeId::new().into();
        let mut projection = projection::Projection::pending(1);
        projection.nodes.insert(address.clone(), source);
        let mut marker = selection::Marker::default();
        marker.update(app.world_mut(), &projection, Some(&address))?;
        app.world_mut().run_schedule(PostUpdate);
        let overlay = frame(app.world_mut())?;
        assert!(
            app.world()
                .get::<bevy::ui::ComputedStackIndex>(overlay)
                .ok_or("Frame stack missing")?
                .0
                > app
                    .world()
                    .get::<bevy::ui::ComputedStackIndex>(source)
                    .ok_or("Source stack missing")?
                    .0,
            "The native border must render above the passive source"
        );
        assert_eq!(
            app.world()
                .get::<ComputedNode>(overlay)
                .ok_or("Frame bounds missing")?
                .size,
            Vec2::new(160., 80.)
        );
        assert_eq!(
            app.world()
                .get::<UiGlobalTransform>(overlay)
                .ok_or("Frame transform missing")?
                .translation,
            Vec2::new(200., 130.)
        );
        assert_eq!(
            app.world()
                .get::<ComputedUiTargetCamera>(overlay)
                .ok_or("Frame camera missing")?
                .get(),
            Some(camera)
        );
        assert!(
            app.world()
                .get::<bevy::ui::CalculatedClip>(source)
                .is_some()
        );
        assert_eq!(
            app.world().get::<bevy::ui::CalculatedClip>(overlay),
            app.world().get::<bevy::ui::CalculatedClip>(source)
        );
        assert!(
            app.world()
                .get::<InheritedVisibility>(overlay)
                .ok_or("Frame visibility missing")?
                .get()
        );
        // A layout target scale change updates physical bounds without reselection.
        app.world_mut()
            .get_mut::<Camera>(camera)
            .ok_or("Camera missing")?
            .computed
            .target_info
            .as_mut()
            .ok_or("Target info missing")?
            .scale_factor = 2.;
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(
            app.world()
                .get::<ComputedNode>(overlay)
                .ok_or("Frame bounds missing")?
                .size,
            Vec2::new(320., 160.)
        );
        assert_eq!(
            app.world()
                .get::<ComputedNode>(overlay)
                .ok_or("Frame bounds missing")?
                .inverse_scale_factor,
            0.5
        );
        app.world_mut()
            .get_mut::<Camera>(camera)
            .ok_or("Camera missing")?
            .computed
            .target_info
            .as_mut()
            .ok_or("Target info missing")?
            .scale_factor = 1.;
        app.world_mut()
            .get_mut::<Node>(source)
            .ok_or("Source missing")?
            .left = px(80);
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(
            app.world()
                .get::<UiGlobalTransform>(overlay)
                .ok_or("Frame transform missing")?
                .translation,
            Vec2::new(260., 130.)
        );
        // Nested nonuniform scaling and rotation can produce shear; never decompose.
        app.world_mut().entity_mut(parent).insert(UiTransform {
            scale: Vec2::new(1.3, 0.7),
            rotation: Rot2::degrees(20.),
            ..default()
        });
        app.world_mut().entity_mut(source).insert(UiTransform {
            rotation: Rot2::degrees(35.),
            ..default()
        });
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(
            app.world().get::<UiGlobalTransform>(overlay),
            app.world().get::<UiGlobalTransform>(source)
        );
        assert_eq!(
            app.world().get::<bevy::ui::CalculatedClip>(overlay),
            app.world().get::<bevy::ui::CalculatedClip>(source)
        );
        app.world_mut()
            .get_mut::<Node>(source)
            .ok_or("Source missing")?
            .display = Display::None;
        app.world_mut().run_schedule(PostUpdate);
        assert!(
            !app.world()
                .get::<InheritedVisibility>(overlay)
                .ok_or("Frame visibility missing")?
                .get()
        );
        app.world_mut()
            .get_mut::<Node>(source)
            .ok_or("Source missing")?
            .display = Display::Flex;
        app.world_mut()
            .get_mut::<Camera>(camera)
            .ok_or("Camera missing")?
            .is_active = false;
        app.world_mut().run_schedule(PostUpdate);
        assert!(
            !app.world()
                .get::<InheritedVisibility>(overlay)
                .ok_or("Frame visibility missing")?
                .get()
        );
        app.world_mut()
            .get_mut::<Camera>(camera)
            .ok_or("Camera missing")?
            .is_active = true;
        app.world_mut().entity_mut(source).insert(UiTransform {
            scale: Vec2::ZERO,
            ..default()
        });
        app.world_mut().run_schedule(PostUpdate);
        assert!(
            !app.world()
                .get::<InheritedVisibility>(overlay)
                .ok_or("Frame visibility missing")?
                .get(),
            "Degenerate geometry must not retain an old frame"
        );
        app.world_mut()
            .entity_mut(source)
            .insert(UiTransform::default());
        app.world_mut()
            .get_mut::<Node>(parent)
            .ok_or("Parent missing")?
            .overflow = Overflow::visible();
        app.world_mut().run_schedule(PostUpdate);
        assert!(
            app.world()
                .get::<InheritedVisibility>(overlay)
                .ok_or("Frame visibility missing")?
                .get()
        );
        assert!(
            app.world()
                .get::<bevy::ui::CalculatedClip>(overlay)
                .is_none(),
            "Removed clipping must not remain cached"
        );
        app.world_mut().entity_mut(source).despawn();
        app.world_mut().run_schedule(PostUpdate);
        assert!(!app.world().entities().contains(overlay));
        marker.clear(app.world_mut());
        // Parent selection gets its own actual box, not the union of descendants.
        projection.nodes.insert(address.clone(), parent);
        marker.update(app.world_mut(), &projection, Some(&address))?;
        app.world_mut().run_schedule(PostUpdate);
        let overlay = frame(app.world_mut())?;
        assert_eq!(
            app.world()
                .get::<ComputedNode>(overlay)
                .ok_or("Frame bounds missing")?
                .size,
            Vec2::new(400., 300.)
        );
        app.world_mut().entity_mut(camera).despawn();
        assert!(
            !app.world().entities().contains(overlay),
            "Camera retirement must reclaim the frame"
        );
        marker.clear(app.world_mut());
    }
    Ok(())
}

#[test]
fn hierarchy_ui_selection_creates_noninteractive_frame_without_source_style_edits()
-> Result<(), String> {
    let mut world = World::new();
    world.init_resource::<Assets<GizmoAsset>>();
    let camera = world.spawn(Camera::default()).id();
    let parent = world.spawn(Node::default()).id();
    let source = world
        .spawn((
            Node::default(),
            ComputedNode {
                size: Vec2::new(160., 80.),
                ..default()
            },
            UiGlobalTransform::from_xy(300., 200.),
            UiTargetCamera(camera),
            ChildOf(parent),
        ))
        .id();
    let address: NodeAddress = NodeId::new().into();
    let mut projection = projection::Projection::pending(1);
    projection.nodes.insert(address.clone(), source);
    let original = world.get::<Node>(source).ok_or("Source missing")?.clone();
    let original_border = *world
        .get::<BorderColor>(source)
        .ok_or("Source border missing")?;
    let mut marker = selection::Marker::default();
    marker.update(&mut world, &projection, Some(&address))?;
    let frames = world
        .query::<(Entity, &BorderColor, &Pickable)>()
        .iter(&world)
        .collect::<Vec<_>>();
    assert_eq!(
        frames.len(),
        1,
        "Hierarchy UI selection has no native frame"
    );
    assert_ne!(frames[0].0, source);
    assert!(!frames[0].2.is_hoverable && !frames[0].2.should_block_lower);
    assert_eq!(
        world.get::<bevy::ui::FocusPolicy>(frames[0].0),
        Some(&bevy::ui::FocusPolicy::Pass),
        "Legacy native UI focus must also pass through the decorative border"
    );
    assert_eq!(frames[0].1.top, Color::srgb_u8(177, 210, 255));
    assert_eq!(world.get::<Node>(source), Some(&original));
    assert_eq!(world.get::<BorderColor>(source), Some(&original_border));
    marker.update(&mut world, &projection, None)?;
    assert_eq!(
        world
            .query::<(&BorderColor, &Pickable)>()
            .iter(&world)
            .count(),
        0
    );
    marker.clear(&mut world);
    marker.update(&mut world, &projection, Some(&address))?;
    let non_ui: NodeAddress = NodeId::new().into();
    projection
        .nodes
        .insert(non_ui.clone(), world.spawn(Transform::IDENTITY).id());
    marker.update(&mut world, &projection, Some(&non_ui))?;
    assert_eq!(
        world
            .query::<(&BorderColor, &Pickable)>()
            .iter(&world)
            .count(),
        0
    );
    Ok(())
}
