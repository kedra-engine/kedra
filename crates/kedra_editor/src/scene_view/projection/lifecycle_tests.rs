use super::*;
use bevy::render::view::screenshot::{Captured, CapturedScreenshots, Capturing, ScreenshotPlugin};
use std::sync::{Mutex, mpsc};

#[test]
fn unextracted_scene_request_cancellation_reclaims_recipient_and_image_immediately() {
    let mut world = World::new();
    world.init_resource::<Assets<Image>>();
    let target = world
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            2,
            2,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    let entity = world
        .spawn((Screenshot::image(target.clone()), readbacks::SceneCapture))
        .id();
    readbacks::cancel(&mut world, entity);
    readbacks::cancel(&mut world, entity);
    readbacks::release_image(&mut world, target);
    assert!(!world.entities().contains(entity));
    assert!(world.resource::<Assets<Image>>().is_empty());
}

#[test]
fn retired_scene_targets_accept_native_completion_and_reclaim_resources()
-> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        ScreenshotPlugin,
    ));
    app.world_mut().init_resource::<Assets<Image>>();
    app.world_mut().init_resource::<Assets<Mesh>>();
    app.world_mut().init_resource::<Assets<StandardMaterial>>();
    let (sender, receiver) = mpsc::channel();
    app.insert_resource(CapturedScreenshots(Arc::new(Mutex::new(receiver))));
    let world = app.world_mut();
    let owner = world.spawn(Visibility::Inherited).id();
    let camera = world.spawn((Camera::default(), ChildOf(owner))).id();
    let viewport = world.spawn_empty().id();
    let mut projection = Projection::pending(1);
    projection.owner = Some(owner);
    projection.camera = Some(camera);
    projection.ui_canvas = Some(ui_canvas::UiCanvas::mount(world, owner, 1, &[]));
    projection.update(world, viewport, Vec2::splat(2.), true)?;
    let canvas = projection.ui_canvas.as_mut().ok_or("UI canvas missing")?;
    canvas.update(world, viewport, Vec2::splat(2.), false)?;
    let mut recipients = Vec::new();
    for (camera, target, size) in [
        (
            camera,
            projection.target.clone().ok_or("Scene target missing")?,
            projection.size,
        ),
        (
            canvas.camera,
            canvas.target.clone().ok_or("UI target missing")?,
            ui_canvas::UiCanvas::SIZE,
        ),
    ] {
        let version = *world
            .get::<TargetVersion>(camera)
            .ok_or("Target epoch missing")?;
        let read = world
            .spawn((
                Screenshot::image(target),
                Capturing,
                Readback { camera, version },
                readbacks::SceneCapture,
            ))
            .observe(captured)
            .id();
        recipients.push((read, size));
    }
    projection.readback = Some(recipients[0].0);
    projection
        .ui_canvas
        .as_mut()
        .ok_or("UI canvas missing")?
        .readback = Some(recipients[1].0);
    projection.retire(world);
    projection.retire(world);
    assert!(!world.entities().contains(owner));
    assert_eq!(world.resource::<Assets<Image>>().len(), 2);
    for (entity, _) in &recipients {
        assert!(world.get::<ChildOf>(*entity).is_none());
        assert!(world.get::<Readback>(*entity).is_none());
    }
    for (entity, size) in &recipients {
        sender.send((
            *entity,
            Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None),
        ))?;
    }
    app.update();
    for (entity, _) in &recipients {
        assert!(app.world().get::<Captured>(*entity).is_some());
    }
    assert!(
        app.world_mut()
            .query::<&Confirmed>()
            .iter(app.world())
            .next()
            .is_none()
    );
    app.update();
    for (entity, _) in recipients {
        assert!(!app.world().entities().contains(entity));
    }
    assert!(app.world().resource::<Assets<Image>>().is_empty());
    Ok(())
}

#[test]
fn resize_lighting_and_hide_cancel_epochs_but_not_native_recipients()
-> Result<(), Box<dyn std::error::Error>> {
    for action in ["resize", "lighting", "hide"] {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            ScreenshotPlugin,
        ));
        app.world_mut().init_resource::<Assets<Image>>();
        let (sender, receiver) = mpsc::channel();
        app.insert_resource(CapturedScreenshots(Arc::new(Mutex::new(receiver))));
        let world = app.world_mut();
        let owner = world.spawn(Visibility::Inherited).id();
        let camera = world.spawn((Camera::default(), ChildOf(owner))).id();
        let viewport = world.spawn_empty().id();
        let mut projection = Projection::pending(1);
        projection.owner = Some(owner);
        projection.camera = Some(camera);
        projection.inspection = Some(world.spawn(Visibility::Inherited).id());
        projection.update(world, viewport, Vec2::splat(2.), true)?;
        let target = projection.target.clone().ok_or("Scene target missing")?;
        let version = *world
            .get::<TargetVersion>(camera)
            .ok_or("Target epoch missing")?;
        let old = world
            .spawn((
                Screenshot::image(target.clone()),
                Capturing,
                Readback { camera, version },
                readbacks::SceneCapture,
            ))
            .observe(captured)
            .id();
        projection.readback = Some(old);
        match action {
            "resize" => projection.update(world, viewport, Vec2::splat(3.), true)?,
            "lighting" => projection.set_inspection(world, true)?,
            _ => projection.update(world, viewport, Vec2::ZERO, false)?,
        }
        assert!(world.entities().contains(old));
        assert!(world.get::<Readback>(old).is_none());
        assert!(world.resource::<Assets<Image>>().get(&target).is_some());
        sender.send((
            old,
            Image::new_target_texture(2, 2, TextureFormat::Rgba8UnormSrgb, None),
        ))?;
        app.update();
        assert!(!projection.frame_ready(app.world()));
        assert!(app.world().get::<Confirmed>(camera).is_none());
        app.update();
        assert!(!app.world().entities().contains(old));
        if action != "lighting" {
            assert!(
                app.world()
                    .resource::<Assets<Image>>()
                    .get(&target)
                    .is_none()
            );
        }
        // An independent current epoch still becomes ready after an old result.
        let world = app.world_mut();
        projection.update(world, viewport, Vec2::splat(3.), true)?;
        let version = *world
            .get::<TargetVersion>(camera)
            .ok_or("Target epoch missing")?;
        let current = world
            .spawn((
                Screenshot::image(projection.target.clone().ok_or("Scene target missing")?),
                Capturing,
                Readback { camera, version },
                readbacks::SceneCapture,
            ))
            .observe(captured)
            .id();
        sender.send((
            current,
            Image::new_target_texture(3, 3, TextureFormat::Rgba8UnormSrgb, None),
        ))?;
        app.update();
        assert!(projection.frame_ready(app.world()), "{action}");
        projection.release_target(app.world_mut());
        app.update();
        assert!(app.world().resource::<Assets<Image>>().is_empty());
    }
    Ok(())
}

#[test]
fn noncompleting_epochs_are_bounded_and_shared_target_releases_after_last_native_cleanup()
-> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        ScreenshotPlugin,
    ));
    app.world_mut().init_resource::<Assets<Image>>();
    let (sender, receiver) = mpsc::channel();
    app.insert_resource(CapturedScreenshots(Arc::new(Mutex::new(receiver))));
    let world = app.world_mut();
    let target = world
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            2,
            2,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    let mut recipients = Vec::new();
    for _ in 0..8 {
        readbacks::admit(world)?;
        let entity = world
            .spawn((
                Screenshot::image(target.clone()),
                Capturing,
                readbacks::SceneCapture,
            ))
            .id();
        readbacks::cancel(world, entity);
        recipients.push(entity);
    }
    assert!(
        readbacks::admit(world)
            .err()
            .ok_or("native capacity exceeded")?
            .contains("awaiting completion")
    );
    assert_eq!(readbacks::retiring_pixels(world, Some(&target)), 4);
    readbacks::release_image(world, target.clone());
    assert_eq!(
        readbacks::retained_pixels(world),
        4,
        "same image counted once"
    );
    assert_eq!(readbacks::retiring_pixels(world, Some(&target)), 0);
    for entity in &recipients[..7] {
        sender.send((
            *entity,
            Image::new_target_texture(2, 2, TextureFormat::Rgba8UnormSrgb, None),
        ))?;
    }
    app.update();
    app.update();
    assert_eq!(app.world().resource::<Assets<Image>>().len(), 1);
    sender.send((
        recipients[7],
        Image::new_target_texture(2, 2, TextureFormat::Rgba8UnormSrgb, None),
    ))?;
    app.update();
    app.update();
    assert!(app.world().resource::<Assets<Image>>().is_empty());
    readbacks::admit(app.world_mut())?;
    Ok(())
}
