//! Retained browser integration uses the existing Assets feature identity.
use super::*;
use crate::workspace::{
    input::regression_tests::setup,
    test_support::{TestResult, sources},
};

#[test]
fn browser_panel_mount_is_retained_and_reset_preserves_its_identity() -> TestResult {
    let dir = tempfile::tempdir()?;
    let project = kedra_project::create(dir.path(), "Panel test")?;
    std::fs::write(project.assets.join("日本語.txt"), b"not run")?;
    let (mut app, _, _) = setup()?;
    let world = app.world_mut();
    sources(world)?;
    world.insert_resource(crate::workspace::test_support::project_manager(
        project, None,
    )?);
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            preset(&mut state)?;
            mount(world, &mut state)?;
            let id = state
                .layout
                .snapshot()
                .views
                .iter()
                .find_map(|(id, view)| (view.kind == "assets").then_some(*id))
                .ok_or("assets")?;
            let root = state.renderer.content(id).ok_or("content")?;
            mount(world, &mut state)?;
            assert_eq!(state.renderer.content(id), Some(root));
            assert_eq!(state.panels.browsers.len(), 1);
            reset(world, &mut state)?;
            assert_eq!(state.renderer.content(id), Some(root));
            assert_eq!(state.panels.browsers[&id].root(), root);
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn browser_native_actions_navigate_and_ignore_stale_or_hidden_bindings() -> TestResult {
    use bevy::ui_widgets::Activate;
    use kedra_ui::ElementId;
    let directory = tempfile::tempdir()?;
    let project = kedra_project::create(directory.path(), "Browser actions")?;
    std::fs::create_dir(project.assets.join("Prefabs"))?;
    std::fs::write(project.assets.join("Prefabs/one.prefab.ron"), "not loaded")?;
    let (mut app, _, _) = setup()?;
    crate::asset_browser::install(&mut app);
    let world = app.world_mut();
    sources(world)?;
    world.insert_resource(crate::workspace::test_support::project_manager(
        project, None,
    )?);
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            preset(&mut state)?;
            mount(world, &mut state)
        },
    )?;
    fn settle(world: &mut World) {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            world.resource_scope(|world, mut state: Mut<EditorWorkspace>| poll(world, &mut state));
            if world
                .query::<&Text>()
                .iter(world)
                .any(|t| t.0 == "Prefabs" || t.0 == "one.prefab.ron")
            {
                break;
            }
            assert!(
                std::time::Instant::now() < until,
                "Browser listing did not complete"
            );
            std::thread::yield_now();
        }
    }
    settle(world);
    let id = world
        .resource::<EditorWorkspace>()
        .layout
        .snapshot()
        .views
        .iter()
        .find_map(|(id, v)| (v.kind == "assets").then_some(*id))
        .ok_or("Assets view")?;
    let element = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find_map(|(e, id)| id.0.ends_with("/item/0").then_some(e))
        .ok_or("Folder button")?;
    world.trigger(Activate { entity: element });
    world.flush();
    super::super::browser_action(
        world,
        id,
        std::path::PathBuf::new(),
        1,
        crate::asset_browser::Action::Open,
    );
    // Stale source-root commands cannot win after the folder request changed generation.
    super::super::browser_action(
        world,
        id,
        std::path::PathBuf::new(),
        1,
        crate::asset_browser::Action::Root,
    );
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !world
        .query::<&Text>()
        .iter(world)
        .any(|t| t.0 == "one.prefab.ron")
    {
        world.resource_scope(|world, mut state: Mut<EditorWorkspace>| poll(world, &mut state));
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
    assert!(
        world
            .query::<&Text>()
            .iter(world)
            .any(|t| t.0 == "Assets / Prefabs")
    );
    world
        .resource_mut::<EditorWorkspace>()
        .layout
        .execute(Command::Hide(id))?;
    super::super::browser_action(
        world,
        id,
        "Prefabs".into(),
        2,
        crate::asset_browser::Action::Up,
    );
    assert!(
        world
            .query::<&Text>()
            .iter(world)
            .any(|t| t.0 == "Assets / Prefabs")
    );
    Ok(())
}
