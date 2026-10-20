//! Native ownership tests without an OS event loop; readiness stays honestly false.
use super::*;
use crate::workspace::{input::regression_tests::setup, test_support::sources};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn restored_unready_host_recovers_without_losing_views() -> TestResult {
    let (mut app, _, b) = setup()?;
    app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let host = HostId::new();
            state
                .layout
                .execute(Command::Float {
                    selection: Selection::View(b),
                    host,
                })
                .map_err(|e| e.to_string())?;
            let native = auxiliary::stage(world, "Restored".into(), UVec2::new(800, 560))?;
            state.surfaces.insert(
                host,
                Surface {
                    window: native.window,
                    root: native.root,
                    owned: Some(native),
                    staged: Some(Duration::ZERO),
                },
            );
            let records = state.layout.snapshot().views.clone();
            world
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_secs(11));
            poll(world, &mut state)?;
            assert!(
                !state.layout.snapshot().hosts.contains_key(&host),
                "Unready restored host never recovered"
            );
            assert_eq!(state.layout.snapshot().views, records);
            let group = state
                .layout
                .snapshot()
                .location(b)
                .ok_or("recovered view")?
                .0;
            assert_eq!(state.layout.snapshot().host_of(group), Some(state.host));
            assert!(!world.entities().contains(native.window));
            Ok(())
        },
    )?;
    Ok(())
}

fn staged(
    world: &mut World,
    state: &mut EditorWorkspace,
    selection: Selection,
) -> Result<(HostId, auxiliary::Surface), String> {
    let host = HostId::new();
    state
        .layout
        .execute(Command::Float { selection, host })
        .map_err(|e| e.to_string())?;
    let native = auxiliary::stage(world, "Restored test".into(), UVec2::new(800, 560))?;
    state.surfaces.insert(
        host,
        Surface {
            window: native.window,
            root: native.root,
            owned: Some(native),
            staged: Some(world.resource::<Time<Real>>().elapsed()),
        },
    );
    Ok((host, native))
}

#[test]
fn restored_deadline_keeps_content_safe_and_valid_sibling_unchanged() -> TestResult {
    let (mut app, a, b) = setup()?;
    let world = app.world_mut();
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            state.stash = world.spawn((Node::default(), ChildOf(state.root))).id();
            let draft = world.spawn(Text::new("session draft 日本語")).id();
            state.renderer.register(b, draft)?;
            let (failed, failed_native) = staged(world, &mut state, Selection::View(b))?;
            redraw(world, &mut state)?;
            let mut ancestor = draft;
            while let Some(parent) = world.get::<ChildOf>(ancestor) {
                ancestor = parent.parent();
                assert_ne!(
                    ancestor, failed_native.root,
                    "Content transferred before native acknowledgment"
                );
            }
            poll(world, &mut state)?;
            assert!(state.layout.snapshot().hosts.contains_key(&failed));
            world
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_secs(11));
            let (sibling, sibling_native) = staged(world, &mut state, Selection::View(a))?;
            let sibling_before = state.layout.snapshot().hosts[&sibling].clone();
            let records = state.layout.snapshot().views.clone();
            let status = world.spawn(Text::new("")).id();
            state.status = Some(status);
            poll(world, &mut state)?;
            assert_eq!(state.layout.snapshot().views, records);
            assert_eq!(state.layout.snapshot().hosts[&sibling], sibling_before);
            assert!(state.surfaces.contains_key(&sibling));
            assert!(world.entities().contains(sibling_native.camera));
            assert!(!world.entities().contains(failed_native.camera));
            assert!(!state.layout.snapshot().hosts.contains_key(&failed));
            assert_eq!(
                world.get::<Text>(draft).map(|t| t.0.as_str()),
                Some("session draft 日本語")
            );
            assert!(
                world
                    .get::<Text>(status)
                    .is_some_and(|t| t.0.contains("timed out"))
            );
            assert_eq!(
                state
                    .layout
                    .snapshot()
                    .host_of(state.layout.snapshot().location(b).ok_or("b")?.0),
                Some(state.host)
            );
            state
                .layout
                .snapshot()
                .validate()
                .map_err(|e| e.to_string())?;
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn restored_creation_failure_recovers_every_record_and_cleans_staged_entities() -> TestResult {
    let (mut app, a, b) = setup()?;
    let world = app.world_mut();
    // A published but incomplete catalog makes create fail after native staging.
    let mut missing = kedra_scene::assets::PrefabSources::default();
    missing.dependencies_ready = true;
    world.insert_resource(missing);
    world.init_resource::<UiScale>();
    let window = world.resource::<EditorWorkspace>().window;
    world.insert_resource(kedra_desktop::DesktopSurface {
        window: Some(window),
        ..default()
    });
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            for view in [a, b] {
                state
                    .layout
                    .execute(Command::Float {
                        selection: Selection::View(view),
                        host: HostId::new(),
                    })
                    .map_err(|e| e.to_string())?;
            }
            let records = state.layout.snapshot().views.clone();
            let windows = world.query::<&Window>().iter(world).count();
            restore(world, &mut state, &BTreeMap::new())?;
            assert_eq!(state.layout.snapshot().hosts.len(), 1);
            assert_eq!(state.layout.snapshot().views, records);
            assert_eq!(state.surfaces.len(), 1);
            assert_eq!(world.query::<&Window>().iter(world).count(), windows);
            for view in [a, b] {
                assert_eq!(
                    state
                        .layout
                        .snapshot()
                        .host_of(state.layout.snapshot().location(view).ok_or("view")?.0),
                    Some(state.host)
                );
            }
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn fresh_float_errors_and_timeout_leave_source_and_retire_only_owned_entities() -> TestResult {
    let (mut app, a, b) = setup()?;
    let world = app.world_mut();
    let mut missing = kedra_scene::assets::PrefabSources::default();
    missing.dependencies_ready = true;
    world.insert_resource(missing);
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let before = state.layout.snapshot().clone();
            let count = world.query::<&Window>().iter(world).count();
            assert!(float(world, &mut state, Selection::View(ViewId::new())).is_err());
            assert!(float(world, &mut state, Selection::View(b)).is_err());
            assert_eq!(state.layout.snapshot(), &before);
            assert_eq!(world.query::<&Window>().iter(world).count(), count);
            let host = HostId::new();
            let candidate = state
                .layout
                .prepare(Command::Float {
                    selection: Selection::View(b),
                    host,
                })
                .map_err(|e| e.to_string())?;
            let native = auxiliary::stage(world, "Pending".into(), UVec2::new(800, 560))?;
            state.pending = Some(Pending {
                candidate,
                host,
                surface: Surface {
                    window: native.window,
                    root: native.root,
                    owned: Some(native),
                    staged: None,
                },
                started: Duration::ZERO,
                focus: Some(b),
            });
            assert!(float(world, &mut state, Selection::View(a)).is_err());
            poll(world, &mut state)?;
            assert!(state.pending.is_some());
            state
                .layout
                .execute(Command::Activate(a))
                .map_err(|e| e.to_string())?;
            let current = state.layout.snapshot().clone();
            world
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_secs(11));
            assert!(poll(world, &mut state).is_err());
            assert!(state.pending.is_none());
            assert_eq!(state.layout.snapshot(), &current);
            assert!(!world.entities().contains(native.window));
            assert!(world.entities().contains(state.window));
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn floating_close_hides_records_and_retires_after_retained_content_moves() -> TestResult {
    let (mut app, _, b) = setup()?;
    let world = app.world_mut();
    sources(world)?;
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let content = world.spawn(Text::new("retained")).id();
            state.renderer.register(b, content)?;
            let (host, native) = staged(world, &mut state, Selection::View(b))?;
            let frame = crate::views::mount(
                world,
                native.root,
                crate::views::document(0xc0600000000040008000000000000001),
                "test-frame",
            )?;
            let caption = crate::views::mount(
                world,
                frame.root,
                crate::views::document(0xc0900000000040008000000000000001),
                "test-caption",
            )?;
            crate::chrome::bind_window(world, &caption, native.window, Some(frame.root))?;
            state.surfaces.get_mut(&host).ok_or("surface")?.staged = None;
            redraw(world, &mut state)?;
            world
                .resource_mut::<CloseRequests>()
                .0
                .extend([state.window, native.window]);
            poll(world, &mut state)?;
            assert!(state.layout.snapshot().hidden.contains_key(&b));
            assert!(state.layout.snapshot().views.contains_key(&b));
            assert!(!state.surfaces.contains_key(&host));
            assert!(!world.entities().contains(native.root));
            assert!(world.entities().contains(content));
            assert!(world.entities().contains(state.window));
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn placement_clamps_and_retire_cancels_staged_and_pending_ownership() -> TestResult {
    let (mut app, _, b) = setup()?;
    let world = app.world_mut();
    sources(world)?;
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let (host, native) = staged(world, &mut state, Selection::View(b))?;
            place(
                world,
                native.window,
                &storage::Placement {
                    width: 800,
                    height: 560,
                    position: Some([i32::MAX, i32::MIN]),
                },
            )?;
            assert!(matches!(
                world.get::<Window>(native.window).ok_or("window")?.position,
                WindowPosition::Centered(_)
            ));
            let candidate = state
                .layout
                .prepare(Command::Float {
                    selection: Selection::Group(
                        state.layout.snapshot().hosts[&host].root.ok_or("root")?,
                    ),
                    host: HostId::new(),
                })
                .map_err(|e| e.to_string())?;
            let other = auxiliary::stage(world, "Pending".into(), UVec2::new(800, 560))?;
            state.pending = Some(Pending {
                candidate,
                host: HostId::new(),
                surface: Surface {
                    window: other.window,
                    root: other.root,
                    owned: Some(other),
                    staged: None,
                },
                started: Duration::ZERO,
                focus: Some(b),
            });
            retire(world, &mut state);
            assert!(state.pending.is_none());
            assert!(!world.entities().contains(native.root));
            assert!(!world.entities().contains(other.root));
            assert!(world.entities().contains(state.root));
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn restore_and_float_stage_authored_native_resources_without_premature_publication() -> TestResult {
    let (mut app, a, b) = setup()?;
    let world = app.world_mut();
    sources(world)?;
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let before = state.layout.snapshot().clone();
            float(world, &mut state, Selection::View(b))?;
            assert_eq!(state.layout.snapshot(), &before);
            let pending = state.pending.as_ref().ok_or("pending")?;
            assert!(!auxiliary::ready(
                world,
                pending.surface.owned.ok_or("owned")?
            ));
            assert!(
                !world
                    .get::<Window>(pending.surface.window)
                    .ok_or("window")?
                    .visible
            );
            assert!(world.get::<ChildOf>(pending.surface.root).is_some());
            retire(world, &mut state);
            let host = HostId::new();
            state
                .layout
                .execute(Command::Float {
                    selection: Selection::View(a),
                    host,
                })
                .map_err(|e| e.to_string())?;
            let placements = BTreeMap::from([(
                host,
                storage::Placement {
                    width: 640,
                    height: 480,
                    position: None,
                },
            )]);
            restore(world, &mut state, &placements)?;
            let surface = state.surfaces.get(&host).ok_or("restored")?;
            assert!(surface.staged.is_some());
            let window = world
                .get::<Window>(surface.window)
                .ok_or("restored window")?;
            assert_eq!(window.width(), 640.);
            assert_eq!(window.height(), 480.);
            assert!(!window.visible);
            redraw(world, &mut state)?;
            poll(world, &mut state)?;
            assert!(state.surfaces[&host].staged.is_some());
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn retired_staging_recovers_into_visible_maximum_and_preserves_hidden_metadata() -> TestResult {
    let (mut app, a, b) = setup()?;
    let world = app.world_mut();
    world.resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let original = state.layout.snapshot().location(a).ok_or("original")?.0;
            let unavailable = ViewId::new();
            state
                .layout
                .execute(Command::Open {
                    id: unavailable,
                    view: kedra_docking::model::View {
                        kind: "unavailable.extension".into(),
                        restore_key: Some("document:日本語".into()),
                    },
                    destination: Destination::Edge {
                        target: original,
                        edge: kedra_docking::model::Edge::Right,
                        fraction: 0.5,
                    },
                })
                .map_err(|e| e.to_string())?;
            let visible = state
                .layout
                .snapshot()
                .location(unavailable)
                .ok_or("visible")?
                .0;
            let host = state.host;
            state
                .layout
                .execute(Command::Maximize {
                    host,
                    group: Some(visible),
                })
                .map_err(|e| e.to_string())?;
            state
                .layout
                .execute(Command::Hide(a))
                .map_err(|e| e.to_string())?;
            let hidden = state.layout.snapshot().hidden.clone();
            let records = state.layout.snapshot().views.clone();
            let (failed, native) = staged(world, &mut state, Selection::View(b))?;
            redraw(world, &mut state)?;
            world.despawn(native.root);
            world.despawn(native.camera);
            world
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_secs(11));
            poll(world, &mut state)?;
            assert!(!state.surfaces.contains_key(&failed));
            assert_eq!(state.layout.snapshot().views, records);
            assert_eq!(state.layout.snapshot().hidden, hidden);
            assert_eq!(
                state.layout.snapshot().hosts[&host].maximized,
                Some(visible)
            );
            assert_eq!(
                state.layout.snapshot().location(b).map(|p| p.0),
                Some(visible)
            );
            Ok(())
        },
    )?;
    Ok(())
}
