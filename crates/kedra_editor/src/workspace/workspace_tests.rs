//! Authored workspace binding and retained panel lifecycle without a renderer.
use super::*;
use test_support::{TestResult, sources};

#[test]
fn authored_bind_mounts_real_panel_records_and_retire_preserves_main_owner() -> TestResult {
    let (mut app, _, _) = input::regression_tests::setup()?;
    let world = app.world_mut();
    sources(world)?;
    let old = world
        .remove_resource::<EditorWorkspace>()
        .ok_or("old workspace")?;
    let (root, window) = (old.root, old.window);
    world.init_resource::<crate::model::ProjectManager>();
    let source = world
        .resource::<kedra_scene::assets::PrefabSources>()
        .snapshot();
    let compiled = source
        .compiled
        .get(&crate::views::document(0xc0600000000040008000000000000001))
        .ok_or("frame")?;
    let view = UiView::mount(
        world,
        compiled,
        Some(root),
        &BTreeMap::new(),
        "test.editor",
        1.,
    )?;
    bind(world, &view)?;
    assert!(bind(world, &view).is_err());
    let state = world.resource::<EditorWorkspace>();
    assert_eq!(state.layout.snapshot().views.len(), 4);
    assert!(
        state
            .layout
            .snapshot()
            .views
            .values()
            .any(|v| v.kind == "preview")
    );
    {
        let kind = "preview";
        let id = state
            .layout
            .snapshot()
            .views
            .iter()
            .find_map(|(id, view)| (view.kind == kind).then_some(*id))
            .ok_or("empty tool")?;
        let content = state.renderer.content(id).ok_or("empty content")?;
        // The synthetic catalog exposes just the empty binding, not a draft field.
        assert_eq!(world.get::<Children>(content).map_or(0, Children::len), 1);
    }
    assert_eq!(state.panels.inspectors.len(), 1);
    for (id, panel) in &state.panels.inspectors {
        assert_eq!(state.renderer.content(*id), Some(panel.root()));
        assert!(panel.state.target.is_none());
    }
    assert_eq!(state.panels.hierarchies.len(), 1);
    assert!(state.status.is_some());
    assert!(
        state
            .layout
            .snapshot()
            .views
            .keys()
            .all(|v| state.renderer.content(*v).is_some())
    );
    retire(world);
    assert!(!world.contains_resource::<EditorWorkspace>());
    assert!(world.entities().contains(root) && world.get::<Window>(window).is_some());
    tick(world); // No workspace is an ordinary no-op, not a resource panic.
    Ok(())
}

#[test]
fn panel_mount_is_idempotent_reset_retains_content_and_unknown_restore_locator() -> TestResult {
    let (mut app, a, _) = input::regression_tests::setup()?;
    let world = app.world_mut();
    sources(world)?;
    world.init_resource::<crate::model::ProjectManager>();
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            panels::mount(world, &mut state)?;
            let content = state.renderer.content(a).ok_or("content")?;
            let count = world.entities().len();
            panels::mount(world, &mut state)?;
            assert_eq!(world.entities().len(), count);
            assert!(state.titles[&a].contains("Unavailable"));
            let records = state.layout.snapshot().views.clone();
            panels::reset(world, &mut state)?;
            assert_eq!(state.renderer.content(a), Some(content));
            for (id, view) in records {
                assert_eq!(state.layout.snapshot().views.get(&id), Some(&view));
            }
            assert!(
                !state
                    .layout
                    .snapshot()
                    .views
                    .values()
                    .any(|v| v.kind == "notes")
            );
            assert!(state.layout.snapshot().views.len() >= 4);
            Ok(())
        },
    )?;
    Ok(())
}
