//! Headless native-component regressions; real input/frame acceptance is separate.
use super::*;
use bevy::input_focus::InputFocus;
use kedra_ui::tooltip::Tooltip;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture() -> Result<(App, Panel, ViewId, tempfile::TempDir), Box<dyn std::error::Error>> {
    let mut app = crate::workspace::browser_test_app()?;
    let directory = tempfile::tempdir()?;
    let world = app.world_mut();
    let host = world.spawn(Node::default()).id();
    let parent = crate::views::mount(
        world,
        host,
        crate::views::document(0xc0600000000040008000000000000001),
        "browser-test",
    )?
    .root;
    let id = ViewId::new();
    let mut panel = Panel::mount(world, parent, id, directory.path().into())?;
    panel.task = None;
    panel.tree_task = None;
    panel.model.loading = false;
    panel.tree.complete(
        0,
        PathBuf::new(),
        Ok(Listing {
            entries: Vec::new(),
            truncated: false,
        }),
    );
    panel.dirty = true;
    panel.tick(world, id)?;
    Ok((app, panel, id, directory))
}

fn geometry(world: &mut World, entity: Entity, top: f32, height: f32, content: f32) {
    // Native physical geometry at 2x scale must produce logical scroll offsets.
    world.entity_mut(entity).insert((
        ComputedNode {
            size: Vec2::new(392., height * 2.),
            content_size: Vec2::new(392., content * 2.),
            inverse_scale_factor: 0.5,
            ..default()
        },
        UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::new(
            196.,
            (top + height / 2.) * 2.,
        ))),
    ));
}

#[test]
fn tree_keyboard_reveals_home_end_up_down_and_parent_in_scrolled_viewport() -> TestResult {
    let (mut app, mut panel, id, _dir) = fixture()?;
    let world = app.world_mut();
    let entries = (0..12)
        .map(|i| model::Entry {
            name: format!("folder-{i:02}").into(),
            kind: Kind::Folder,
            bytes: None,
            tracked: false,
        })
        .collect();
    panel.tree.complete(
        0,
        PathBuf::new(),
        Ok(Listing {
            entries,
            truncated: false,
        }),
    );
    panel.render(world, id)?;
    let area = panel.element("folders")?;
    world
        .entity_mut(area)
        .insert(ScrollPosition(Vec2::new(7., 0.)));
    geometry(world, area, 100., 78., 338.);
    let cases = [
        (PathBuf::new(), KeyCode::End, 0., 12, 260.),
        (PathBuf::from("folder-11"), KeyCode::Home, 260., 0, 0.),
        (PathBuf::from("folder-03"), KeyCode::ArrowUp, 104., 3, 78.),
        (PathBuf::from("folder-04"), KeyCode::ArrowDown, 78., 6, 104.),
        (PathBuf::from("folder-11"), KeyCode::ArrowLeft, 260., 0, 0.),
        (PathBuf::new(), KeyCode::ArrowLeft, 260., 0, 0.),
    ];
    for (path, key, scroll, target, expected) in cases {
        world.get_mut::<ScrollPosition>(area).ok_or("scroll")?.y = scroll;
        for (index, (_, row)) in panel.folders.iter().enumerate() {
            geometry(
                world,
                row.root,
                100. + index as f32 * 26. - scroll,
                26.,
                26.,
            );
        }
        panel.tree_key(world, path, key)?;
        let expected_focus = panel.folders[target].1.element("navigate")?;
        assert_eq!(
            world.resource::<InputFocus>().get(),
            Some(expected_focus),
            "{key:?}"
        );
        let position = world.get::<ScrollPosition>(area).ok_or("scroll")?;
        assert_eq!(
            position.y, expected,
            "{key:?} must reveal the focused folder"
        );
        assert_eq!(
            position.x, 7.,
            "vertical reveal must preserve horizontal scroll"
        );
        assert!((0. ..=260.).contains(&position.y));
    }
    // Absent/unallocated native geometry must not fabricate a viewport or scroll jump.
    world.entity_mut(area).remove::<ComputedNode>();
    panel.tree_key(world, PathBuf::new(), KeyCode::End)?;
    assert_eq!(world.get::<ScrollPosition>(area).ok_or("scroll")?.y, 0.);
    Ok(())
}

#[test]
fn idle_browser_does_not_generate_hint_text_or_change_tooltip_ticks() -> TestResult {
    let (mut app, mut panel, id, _dir) = fixture()?;
    let world = app.world_mut();
    let entity = panel.element("back")?;
    let before = world
        .entity(entity)
        .get_change_ticks::<Tooltip>()
        .ok_or("tooltip")?;
    let generations = hints::GENERATIONS.with(|count| count.get());
    for _ in 0..4 {
        world.increment_change_tick();
        panel.tick(world, id)?;
    }
    assert_eq!(
        hints::GENERATIONS.with(|count| count.get()),
        generations,
        "idle ticks must not regenerate strings even when Tooltip equality is unchanged"
    );
    assert_eq!(
        world
            .entity(entity)
            .get_change_ticks::<Tooltip>()
            .ok_or("tooltip")?
            .changed,
        before.changed
    );
    Ok(())
}

#[test]
fn dirty_errors_and_reused_bindings_refresh_hints() -> TestResult {
    let (mut app, mut panel, id, _dir) = fixture()?;
    let world = app.world_mut();
    let root = panel.folders[0].1.element("navigate")?;
    panel
        .tree
        .complete(0, PathBuf::new(), Err("unreadable folder".into()));
    panel.dirty = true;
    panel.tick(world, id)?;
    assert!(
        world
            .get::<Tooltip>(root)
            .ok_or("tree hint")?
            .0
            .contains("unreadable folder")
    );
    panel.tree.refresh();
    panel.tree.complete(
        1,
        PathBuf::new(),
        Ok(Listing {
            entries: Vec::new(),
            truncated: false,
        }),
    );
    panel.model.error = Some("stale listing".into());
    panel.dirty = true;
    panel.tick(world, id)?;
    assert_eq!(world.get::<Tooltip>(root).ok_or("tree hint")?.0, "Assets");
    assert!(
        world
            .get::<Text>(panel.element("message")?)
            .ok_or("message")?
            .0
            .contains("stale listing")
    );
    panel.bind(world, root, id, Action::Unavailable("new binding reason"));
    assert_eq!(
        world.get::<Tooltip>(root).ok_or("reused hint")?.0,
        "new binding reason"
    );
    panel.model.entries = vec![model::Entry {
        name: "日本語 Straße.txt".into(),
        kind: Kind::File,
        bytes: None,
        tracked: false,
    }];
    panel.bind(world, root, id, Action::Select("日本語 Straße.txt".into()));
    assert_eq!(
        world.get::<Tooltip>(root).ok_or("source hint")?.0,
        "日本語 Straße.txt"
    );
    // A failed listing leaves the old source identified, rather than substituting
    // another entry. A later listing removal must clear a retained stale hint.
    panel.dirty = true;
    panel.tick(world, id)?;
    assert_eq!(
        world.get::<Tooltip>(root).ok_or("tree rebound")?.0,
        "Assets"
    );
    let tile = panel.rows[0].1.root;
    assert_eq!(
        world.get::<Tooltip>(tile).ok_or("listing hint")?.0,
        "日本語 Straße.txt"
    );
    panel.model.entries.clear();
    panel.model.error = None;
    panel.dirty = true;
    panel.tick(world, id)?;
    assert!(
        world.get_entity(tile).is_err(),
        "removed listing rows must retire their hints"
    );
    Ok(())
}

#[test]
fn tree_reveal_bounds_small_and_unallocated_native_viewports() -> TestResult {
    let (mut app, mut panel, _id, _dir) = fixture()?;
    let world = app.world_mut();
    let area = panel.element("folders")?;
    let row = panel.folders[0].1.root;
    world.entity_mut(area).insert(ScrollPosition(Vec2::ZERO));
    geometry(world, area, 100., 10., 26.);
    geometry(world, row, 100., 26., 26.);
    panel.tree_key(world, PathBuf::new(), KeyCode::Home)?;
    assert_eq!(
        world.get::<ScrollPosition>(area).ok_or("scroll")?.y,
        0.,
        "oversized rows align at their top rather than scrolling past it"
    );
    geometry(world, row, 500., 26., 26.);
    panel.tree_key(world, PathBuf::new(), KeyCode::End)?;
    assert_eq!(
        world.get::<ScrollPosition>(area).ok_or("scroll")?.y,
        16.,
        "reveal cannot exceed native content extent"
    );
    geometry(world, area, 100., 0., 26.);
    panel.tree_key(world, PathBuf::new(), KeyCode::Home)?;
    assert_eq!(
        world.get::<ScrollPosition>(area).ok_or("scroll")?.y,
        16.,
        "zero-sized viewport must not fabricate scroll geometry"
    );
    Ok(())
}
