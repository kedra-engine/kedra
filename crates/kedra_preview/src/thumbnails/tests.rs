//! Thumbnail ownership invariants, independent of native screenshot correctness.
use super::*;
fn request(path: &str, revision: &str) -> Request {
    Request {
        path: path.into(),
        revision: revision.into(),
        kind: Kind::Model,
    }
}
#[test]
fn resident_demand_pins_capacity_and_revision_never_reuses_an_old_image() {
    let mut world = World::new();
    let mut images = Assets::<Image>::default();
    let image = images.add(Image::default());
    world.insert_resource(images);
    let mut cache = Thumbnails::default();
    let mut demand = Vec::new();
    for index in 0..CAPACITY {
        let request = request(&format!("game://{index}.glb"), "a");
        cache.cache.insert(
            request.key(),
            Cached {
                image: Some(image.clone()),
                error: None,
                used: 0,
            },
        );
        demand.push(request);
    }
    demand.push(request("game://new.glb", "a"));
    cache.update(&mut world, &demand, 1, None);
    assert!(!cache.busy());
    assert!(cache.image(&demand[0]).is_some());
    assert!(cache.image(&request(&demand[0].path, "b")).is_none());
    assert!(cache.image(&demand[CAPACITY]).is_none());
}

#[test]
fn visible_grid_beyond_thirty_two_tiles_continues_loading() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<Image>();
    let image = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    let mut cache = Thumbnails::default();
    let demand: Vec<_> = (0..40)
        .map(|index| Request {
            path: format!("tile-{index}.png"),
            revision: "grid".into(),
            kind: Kind::Image,
        })
        .collect();
    for request in &demand[..32] {
        cache.cache.insert(
            request.key(),
            Cached {
                image: Some(image.clone()),
                error: None,
                used: 0,
            },
        );
    }
    cache.update(app.world_mut(), &demand, 1, None);
    assert!(
        cache.busy(),
        "Visible pinned tiles must not starve the rest of the grid"
    );
    assert!(
        demand[..32]
            .iter()
            .all(|request| cache.image(request).is_some())
    );
}

#[test]
fn captured_model_pixels_are_published_for_both_ui_and_disk() -> Result<(), String> {
    let mut world = World::new();
    world.init_resource::<Time>();
    world.init_resource::<Assets<Image>>();
    let request = request("game://model.glb", "current");
    let image = Image {
        asset_usage: bevy::asset::RenderAssetUsages::MAIN_WORLD,
        ..default()
    };
    let mut cache = Thumbnails {
        job: Some(Job {
            request: request.clone(),
            loading: Loading::Model(default()),
            root: None,
            target: None,
            capture: Some(world.spawn_empty().id()),
            frames: 12,
            elapsed: 1.,
            result: Arc::new(Mutex::new(Some(image))),
            conversion: None,
        }),
        ..default()
    };
    cache.update(&mut world, std::slice::from_ref(&request), 1, None);
    let handle = cache.image(&request).ok_or("Published model thumbnail")?;
    let image = world
        .resource::<Assets<Image>>()
        .get(&handle)
        .ok_or("Retained pixels")?;
    assert!(image.asset_usage.contains(
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD
    ));
    assert!(!cache.busy());
    Ok(())
}

#[test]
fn withdrawn_capture_retires_only_its_forest_and_never_publishes_late_pixels() {
    let mut world = World::new();
    world.init_resource::<Time>();
    world.init_resource::<Assets<Image>>();
    let foreign = world.spawn_empty().id();
    let root = world.spawn_empty().id();
    let child = world.spawn(ChildOf(root)).id();
    let capture = world.spawn_empty().id();
    let request = request("game://old.glb", "old-scope");
    let mut cache = Thumbnails {
        job: Some(Job {
            request: request.clone(),
            loading: Loading::Image(default()),
            root: Some(root),
            target: None,
            capture: Some(capture),
            frames: 12,
            elapsed: 1.,
            result: Arc::new(Mutex::new(Some(Image::default()))),
            conversion: None,
        }),
        ..default()
    };
    cache.update(&mut world, &[], 1, None);
    assert!(!cache.busy());
    assert!(cache.image(&request).is_none());
    assert!(world.get_entity(root).is_err());
    assert!(world.get_entity(child).is_err());
    assert!(world.get_entity(capture).is_err());
    assert!(world.get_entity(foreign).is_ok());
    assert!(world.resource::<Assets<Image>>().is_empty());
}
