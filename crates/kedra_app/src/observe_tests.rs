#![allow(
    clippy::unwrap_used,
    reason = "Headless renderer-observation assertions"
)]
use super::*;
use bevy::reflect::structs::DynamicStruct;

fn host() -> (World, Entity, Entity) {
    let mut world = World::new();
    world.insert_resource(crate::AppSession { id: "test".into() });
    let window = world.spawn((Window::default(), PrimaryWindow)).id();
    let camera = world
        .spawn((
            Camera::default(),
            RenderTarget::Window(bevy::window::WindowRef::Primary),
        ))
        .id();
    (world, window, camera)
}
fn node(world: &mut World, camera: Entity, id: &str) -> Entity {
    // Initialize the renderer-derived camera component without a native renderer.
    let mut target = ComputedUiTargetCamera::default();
    let mut reflected = DynamicStruct::default();
    reflected.insert("camera", Some(camera));
    target.apply(&reflected);
    world
        .spawn((
            ElementId::new(id),
            ComputedNode {
                size: Vec2::splat(10.),
                ..default()
            },
            UiGlobalTransform::default(),
            InheritedVisibility::VISIBLE,
            target,
        ))
        .id()
}

#[test]
fn observation_and_click_coordinates_include_ui_scale_and_viewport_origin() {
    for dpr in [1., 2.] {
        for ui_scale in [0.5, 2., 3.] {
            let (mut world, window, camera) = host();
            world.insert_resource(UiScale(ui_scale));
            let mut native_camera = world.get_mut::<Camera>(camera).unwrap();
            native_camera.viewport = Some(bevy::camera::Viewport {
                physical_position: UVec2::new(40, 60),
                physical_size: UVec2::new(400, 300),
                ..default()
            });
            native_camera.computed.target_info = Some(bevy::camera::RenderTargetInfo {
                physical_size: UVec2::new(800, 600),
                scale_factor: dpr,
            });
            let entity = node(&mut world, camera, "scaled");
            world.entity_mut(entity).insert((
                ComputedNode {
                    size: Vec2::new(100., 40.),
                    inverse_scale_factor: 1. / (dpr * ui_scale),
                    ..default()
                },
                UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::new(
                    200., 160.,
                ))),
            ));
            let observed = snapshot(&mut world).unwrap();
            let rect: Vec<f32> =
                serde_json::from_value(observed["elements"][0]["rect"].clone()).unwrap();
            for (actual, expected) in rect.into_iter().zip([190., 200., 290., 240.]) {
                assert!((actual - expected / dpr).abs() < 0.0001);
            }
            let point = element_center(&mut world, window, "scaled").unwrap();
            assert!((point - Vec2::new(240., 220.) / dpr).length() < 0.0001);
        }
    }
}

#[test]
fn actual_truncated_snapshot_never_resolves_a_duplicate_beyond_the_cap() {
    let (mut world, window, camera) = host();
    let mut last = None;
    for i in 0..4097 {
        let id = if i == 0 || i == 4096 {
            "duplicate".into()
        } else {
            format!("node.{i}")
        };
        last = Some(node(&mut world, camera, &id));
    }
    let snapshot = snapshot(&mut world).unwrap();
    assert_eq!(snapshot["truncated"], true);
    assert_eq!(snapshot["elements"].as_array().unwrap().len(), 4096);
    assert!(element_center(&mut world, window, "duplicate").is_err());
    assert!(element_center(&mut world, window, "node.1").is_err());
    world.despawn(last.unwrap());
    assert_eq!(super::snapshot(&mut world).unwrap()["truncated"], false);
    assert!(element_center(&mut world, window, "duplicate").is_ok());
}

#[test]
fn snapshot_reports_real_fields_focus_ancestors_and_window_scoped_identity() {
    let (mut world, window, camera) = host();
    world.init_resource::<crate::input::InputQueue>();
    world.insert_resource(bevy::diagnostic::FrameCount(42));
    let parent = world.spawn(InteractionDisabled).id();
    let entity = node(&mut world, camera, "field");
    world.entity_mut(entity).insert((
        ChildOf(parent),
        Text::new("Label"),
        EditableText::new("Кедра"),
        TextFont::default(),
        BackgroundColor(Color::srgb_u8(1, 2, 3)),
        TextColor(Color::srgb_u8(4, 5, 6)),
        ScreenId("screen".into()),
    ));
    world.insert_resource(InputFocus::from_entity(entity));
    let snapshot = observe(In(None), &mut world).unwrap();
    assert_eq!(snapshot["session"], "test");
    assert_eq!(snapshot["frame"], 42);
    assert_eq!(snapshot["windows"][0]["id"], window.to_bits());
    assert_eq!(snapshot["windows"][0]["primary"], true);
    assert_eq!(snapshot["screens"], json!(["screen"]));
    assert_eq!(snapshot["cameras"][0]["entity"], camera.to_bits());
    let field = &snapshot["elements"][0];
    assert_eq!(field["value"], "Кедра");
    assert_eq!(field["text"], "Label");
    assert_eq!(field["enabled"], false);
    assert_eq!(field["focused"], true);
    assert_eq!(field["background"], json!([1, 2, 3, 255]));
    assert_eq!(field["color"], json!([4, 5, 6, 255]));
    assert!(field["font"].is_object());
    assert_eq!(snapshot["truncated"], false);
    assert!(
        element_center(&mut world, window, "field")
            .unwrap_err()
            .contains("disabled")
    );
    world.entity_mut(parent).remove::<InteractionDisabled>();
    assert_eq!(
        element_center(&mut world, window, "field").unwrap(),
        Vec2::ZERO
    );
    let second = world.spawn(Window::default()).id();
    let second_camera = world
        .spawn((
            Camera::default(),
            RenderTarget::Window(bevy::window::WindowRef::Entity(second)),
        ))
        .id();
    node(&mut world, second_camera, "field");
    assert!(super::snapshot(&mut world).is_ok());
    assert!(element_center(&mut world, second, "field").is_ok());
    assert!(element_center(&mut world, window, "absent").is_err());
}

#[test]
fn duplicate_hidden_disabled_and_inactive_targets_fail_closed() {
    let (mut world, window, camera) = host();
    let first = node(&mut world, camera, "target");
    let duplicate = node(&mut world, camera, "target");
    assert!(
        observe(In(None), &mut world)
            .unwrap_err()
            .message
            .contains("Duplicate")
    );
    world
        .entity_mut(duplicate)
        .insert(InheritedVisibility::HIDDEN);
    assert!(element_center(&mut world, window, "target").is_ok());
    world.entity_mut(first).insert(InteractionDisabled);
    assert!(element_center(&mut world, window, "target").is_err());
    world.entity_mut(first).remove::<InteractionDisabled>();
    world.entity_mut(first).insert(CalculatedClip::FullyClipped);
    assert!(element_center(&mut world, window, "target").is_err());
    let clip = CalculatedClip::default()
        .with_rect(Rect::new(1., 1., 5., 5.), &UiGlobalTransform::default());
    world.entity_mut(first).insert(clip);
    assert!(
        element_center(&mut world, window, "target")
            .unwrap_err()
            .contains("clipped")
    );
    world.entity_mut(first).remove::<CalculatedClip>();
    world.get_mut::<Camera>(camera).unwrap().is_active = false;
    assert!(element_center(&mut world, window, "target").is_err());
    world.get_mut::<Camera>(camera).unwrap().is_active = true;
    world.entity_mut(first).insert(ComputedNode::default());
    assert!(element_center(&mut world, window, "target").is_err());
    world.despawn(camera);
    assert!(element_center(&mut world, window, "target").is_err());
}

#[test]
fn click_by_id_uses_observed_target_and_truncation_cannot_admit_input() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        crate::KedraAppPlugin,
    ));
    app.update();
    let window = app.world_mut().spawn(Window::default()).id();
    let camera = app
        .world_mut()
        .spawn((
            Camera::default(),
            RenderTarget::Window(bevy::window::WindowRef::Entity(window)),
        ))
        .id();
    node(app.world_mut(), camera, "button");
    let session = app.world().resource::<crate::AppSession>().id.clone();
    let id = uuid::Uuid::new_v4();
    let request = json!({"session":session,"id":id,"window":window.to_bits(),"input":{"kind":"click","element":"button"}});
    assert_eq!(
        crate::input::request(In(Some(request)), app.world_mut()).unwrap()["state"],
        "pending"
    );
    // Do not run input without the native input message plugins. The refusal
    // below must happen during validation, before any new receipt is admitted.
    for i in 0..4096 {
        node(app.world_mut(), camera, &format!("other.{i}"));
    }
    let rejected = uuid::Uuid::new_v4();
    let request = json!({"session":session,"id":rejected,"window":window.to_bits(),"input":{"kind":"click","element":"button"}});
    assert!(
        crate::input::request(In(Some(request)), app.world_mut())
            .unwrap_err()
            .message
            .contains("truncated")
    );
    assert!(
        app.world()
            .resource::<crate::operations::Operations>()
            .get(rejected)
            .is_none()
    );
}
