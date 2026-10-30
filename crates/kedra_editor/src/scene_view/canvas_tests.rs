//! Mixed projection regression: spatial extent and passive UI extent are independent.
use super::*;
use bevy::asset::{AssetApp, AssetPlugin};
use kedra_scene::{
    composition::Catalog,
    document::{ComponentData, Node as SourceNode, NodeId, PrefabDocument},
};
use std::collections::BTreeMap;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture(mixed: bool) -> Result<(App, Arc<CompiledPrefab>), Box<dyn std::error::Error>> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<AppTypeRegistry>()
        .add_plugins(kedra_ui::authored::AuthoredUiPlugin)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>();
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    kedra_scene::spatial::register(
        &mut registry.write(),
        &mut app.world_mut().resource_mut::<ComponentPolicies>(),
    )?;
    let mut source = PrefabDocument::new("Canvas regression");
    let ui = ComponentData::ron(1, "(node:(width:Percent(100.0),height:Percent(100.0)))")?;
    if mixed {
        source.nodes[0].components.insert(
            "scene.primitive".into(),
            ComponentData::ron(1, "Sphere(radius:1.0)")?,
        );
        source.nodes[0].components.insert("scene.surface".into(),ComponentData::ron(1,"(color:Srgba((red:1.0,green:1.0,blue:1.0,alpha:1.0)),texture:None,metallic:0.0,roughness:0.5)")?);
        source.nodes.push(SourceNode {
            id: NodeId::new(),
            name: "Passive canvas".into(),
            parent: Some(source.root.into()),
            components: BTreeMap::from([("ui.layout".into(), ui)]),
        });
    } else {
        source.nodes[0].components.insert("ui.layout".into(), ui);
    }
    let id = source.id;
    let mut catalog = Catalog::default();
    catalog.insert(source)?;
    let compiled = CompiledPrefab::compile(
        catalog.resolve(id)?,
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
        &mut app.world().resource::<AssetServer>().clone(),
        &BTreeMap::new(),
    )?;
    Ok((app, Arc::new(compiled)))
}

#[test]
fn mixed_spatial_target_fills_wide_and_tall_viewports_while_ui_canvas_stays_fixed() -> TestResult {
    let (mut app, source) = fixture(true)?;
    let world = app.world_mut();
    let mut projection = projection::Projection::pending(1);
    projection.mount(world, source.clone())?;
    let (address, ui_source) = projection
        .nodes
        .iter()
        .find(|(_, entity)| world.get::<Node>(**entity).is_some())
        .ok_or("UI source missing")?;
    let address = address.clone();
    let ui_camera = world
        .get::<UiTargetCamera>(*ui_source)
        .ok_or("UI target missing")?
        .0;
    let mut marker = selection::Marker::default();
    marker.update(world, &projection, Some(&address))?;
    let frame = world
        .query::<(Entity, &BorderColor, &UiTargetCamera)>()
        .iter(world)
        .find(|(_, border, _)| border.top == ui_marker::SELECTION_COLOR)
        .ok_or("Selection frame missing")?
        .0;
    assert_eq!(
        world.get::<UiTargetCamera>(frame).map(|camera| camera.0),
        Some(ui_camera)
    );
    assert_ne!(Some(ui_camera), projection.camera());
    let display = world.spawn(Node::default()).id();
    for size in [Vec2::new(1600., 600.), Vec2::new(600., 1000.)] {
        projection.update(world, display, size, true)?;
        marker.update(world, &projection, Some(&address))?;
        assert_eq!(
            world.get::<UiTargetCamera>(frame).map(|camera| camera.0),
            Some(ui_camera)
        );
        let node = world.get::<Node>(display).ok_or("Display missing")?;
        assert_eq!(
            (node.width, node.height, node.left, node.top),
            (percent(100), percent(100), percent(0), percent(0)),
            "Mixed UI must not letterbox the spatial picking/render region"
        );
        let image = world
            .get::<ImageNode>(display)
            .ok_or("Spatial image missing")?;
        let texture = world
            .resource::<Assets<Image>>()
            .get(&image.image)
            .ok_or("Spatial target missing")?;
        assert_eq!(
            (texture.width(), texture.height()),
            (size.x as u32, size.y as u32)
        );
        let overlay = world
            .get::<Children>(display)
            .and_then(|children| children.first())
            .copied()
            .ok_or("UI overlay absent")?;
        let image = world.get::<ImageNode>(overlay).ok_or("UI image missing")?;
        let texture = world
            .resource::<Assets<Image>>()
            .get(&image.image)
            .ok_or("UI target missing")?;
        assert_eq!((texture.width(), texture.height()), (1280, 720));
        let node = world
            .get::<Node>(overlay)
            .ok_or("Overlay geometry absent")?;
        let (Val::Percent(width), Val::Percent(height)) = (node.width, node.height) else {
            return Err("UI overlay must fit its parent".into());
        };
        assert!(((width * size.x) / (height * size.y) - 1280. / 720.).abs() < 0.0001);
        assert!(
            world
                .get::<bevy::picking::Pickable>(overlay)
                .is_some_and(|p| !p.is_hoverable && !p.should_block_lower)
        );
        let cameras = projection
            .nodes
            .values()
            .filter_map(|entity| world.get::<UiTargetCamera>(*entity))
            .map(|target| target.0)
            .collect::<Vec<_>>();
        assert!(!cameras.is_empty());
        assert!(
            cameras
                .iter()
                .all(|camera| Some(*camera) != projection.camera()
                    && world.get::<Camera2d>(*camera).is_some())
        );
        assert!(
            !projection.frame_ready(world),
            "Two allocated targets are not proof of two rendered frames"
        );
    }
    assert_eq!(world.resource::<Assets<Image>>().len(), 2);
    projection.update(world, display, Vec2::new(f32::NAN, 20.), true)?;
    assert!(world.resource::<Assets<Image>>().is_empty());
    assert!(world.get::<ImageNode>(display).is_none());
    assert!(
        world
            .query::<&Camera>()
            .iter(world)
            .all(|camera| !camera.is_active)
    );
    projection.update(world, display, Vec2::new(900., 500.), true)?;
    assert_eq!(world.resource::<Assets<Image>>().len(), 2);
    projection.retire(world);
    assert!(!world.entities().contains(frame));
    marker.clear(world);
    projection.retire(world);
    assert!(world.resource::<Assets<Image>>().is_empty());
    assert!(
        world
            .get::<Children>(display)
            .is_none_or(|children| children.is_empty())
    );
    assert!(
        source
            .authoring_world()
            .iter_entities()
            .all(|e| e.get::<UiTargetCamera>().is_none())
    );
    Ok(())
}

#[test]
fn standalone_passive_ui_keeps_its_existing_fixed_aspect_profile() -> TestResult {
    let (mut app, source) = fixture(false)?;
    let world = app.world_mut();
    let mut projection = projection::Projection::pending(1);
    projection.mount(world, source)?;
    let address = projection
        .nodes
        .keys()
        .next()
        .ok_or("UI root missing")?
        .clone();
    let mut marker = selection::Marker::default();
    marker.update(world, &projection, Some(&address))?;
    let frame_camera = world
        .query::<(&BorderColor, &UiTargetCamera)>()
        .iter(world)
        .find(|(border, _)| border.top == ui_marker::SELECTION_COLOR)
        .ok_or("Standalone UI selection frame missing")?
        .1
        .0;
    assert_eq!(Some(frame_camera), projection.camera());
    let display = world.spawn(Node::default()).id();
    projection.update(world, display, Vec2::new(1600., 600.), true)?;
    let node = world.get::<Node>(display).ok_or("Display absent")?;
    assert!(matches!(node.width,Val::Percent(width) if (width-66.66667).abs()<0.001));
    assert_eq!(node.height, percent(100));
    assert_eq!(world.resource::<Assets<Image>>().len(), 1);
    projection.retire(world);
    marker.clear(world);
    Ok(())
}
