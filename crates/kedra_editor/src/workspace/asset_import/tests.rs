//! Synthetic native-message routing; OS transfer and dialogs require manual acceptance.
use super::*;
use crate::workspace::test_support::{setup, sources};

#[test]
fn native_drop_routes_to_its_visible_browser_and_refreshes_imported_sources()
-> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let project = kedra_project::create(temporary.path(), "Import")?;
    let assets = project.assets.clone();
    let outside = temporary.path().join("license.txt");
    std::fs::write(&outside, "source")?;
    let (mut app, _, _) = setup()?;
    install(&mut app);
    let world = app.world_mut();
    sources(world)?;
    world.insert_resource(crate::workspace::test_support::project_manager(
        project, None,
    )?);
    let (view, window) = world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<_, String> {
            panels::preset(&mut state)?;
            panels::mount(world, &mut state)?;
            let view = *state.panels.browsers.keys().next().ok_or("browser")?;
            let root = state.panels.browsers[&view].root();
            let window = state.window;
            let camera = world
                .spawn((
                    Camera::default(),
                    bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window)),
                ))
                .id();
            world.entity_mut(root).insert((
                UiTargetCamera(camera),
                ComputedNode {
                    size: Vec2::new(300., 200.),
                    inverse_scale_factor: 1.,
                    ..default()
                },
                UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::new(
                    150., 100.,
                ))),
            ));
            world
                .get_mut::<Window>(window)
                .ok_or("window")?
                .set_cursor_position(Some(Vec2::new(100., 100.)));
            assert_eq!(target(world, &state, window), Some(view));
            let foreign = world.spawn(Window::default()).id();
            assert_eq!(target(world, &state, foreign), None);
            state.menu.push(root);
            assert_eq!(target(world, &state, window), None);
            state.menu.clear();
            world
                .get_mut::<Window>(window)
                .ok_or("window")?
                .set_cursor_position(Some(Vec2::new(500., 500.)));
            assert_eq!(target(world, &state, window), None);
            world
                .get_mut::<Window>(window)
                .ok_or("window")?
                .set_cursor_position(Some(Vec2::new(100., 100.)));
            Ok((view, window))
        },
    )?;
    world.write_message(FileDragAndDrop::HoveredFileCanceled { window });
    app.update();
    assert!(!pending(app.world()));
    assert!(!assets.join("license.txt").exists());
    let ready_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let ready = app.world_mut().resource_scope(
            |world, mut state: Mut<EditorWorkspace>| -> Result<bool, String> {
                let browser = state
                    .panels
                    .browsers
                    .get_mut(&view)
                    .ok_or("browser retired")?;
                browser.tick(world, view)?;
                Ok(browser.listing_ready())
            },
        )?;
        if ready {
            break;
        }
        assert!(
            std::time::Instant::now() < ready_deadline,
            "Browser listing did not become ready"
        );
        app.update();
        std::thread::yield_now();
    }
    app.world_mut().write_message(FileDragAndDrop::DroppedFile {
        window,
        path_buf: outside.clone(),
    });
    app.update();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while pending(app.world()) {
        assert!(
            std::time::Instant::now() < deadline,
            "Import did not complete"
        );
        app.update();
        std::thread::yield_now();
    }
    assert_eq!(
        std::fs::read_to_string(assets.join("license.txt"))?,
        "source"
    );
    assert!(assets.join("license.txt.meta").exists());
    assert_eq!(std::fs::read_to_string(outside)?, "source");
    assert!(
        app.world()
            .resource::<EditorWorkspace>()
            .panels
            .browsers
            .contains_key(&view)
    );
    Ok(())
}
