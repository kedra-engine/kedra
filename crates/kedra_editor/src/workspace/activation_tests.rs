//! Content authority follows committed docking, not tool keyboard focus.
use super::*;
use kedra_docking::{command::Selection, model::View};
use kedra_scene::document::PrefabDocument;
use test_support::TestResult;

struct Fixture {
    app: App,
    _directory: tempfile::TempDir,
    first: PrefabDocument,
    second: PrefabDocument,
    a: ViewId,
    b: ViewId,
}
impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let project = kedra_project::create(directory.path(), "Activation")?;
        let first = PrefabDocument::new("First");
        let second = PrefabDocument::new("Second");
        for (path, source) in [("first.prefab.ron", &first), ("second.prefab.ron", &second)] {
            std::fs::write(project.assets.join(path), source.serialize()?)?;
        }
        let root = project.assets.clone();
        let (mut app, _, _) = test_support::setup()?;
        let world = app.world_mut();
        test_support::sources(world)?;
        world.insert_resource(test_support::project_manager(project, None)?);
        world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            panels::preset(&mut state)?;
            panels::mount(world, &mut state)
        })?;
        for (path, source) in [("first.prefab.ron", &first), ("second.prefab.ron", &second)] {
            world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
                state.documents.request(world, root.clone(), path.into())
            })?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while world.resource::<EditorWorkspace>().documents.active != Some(source.id) {
                world.resource_scope(|world, mut state: Mut<EditorWorkspace>| {
                    panels::poll(world, &mut state)
                });
                assert!(
                    std::time::Instant::now() < deadline,
                    "Source open timed out"
                );
                std::thread::yield_now();
            }
        }
        let state = world.resource::<EditorWorkspace>();
        let a = state.documents.opened[&first.id].view;
        let b = state.documents.opened[&second.id].view;
        Ok(Self {
            app,
            _directory: directory,
            first,
            second,
            a,
            b,
        })
    }
    fn command(&mut self, command: Command) -> Result<(), String> {
        self.app
            .world_mut()
            .resource_scope(|world, mut state: Mut<EditorWorkspace>| {
                apply(world, &mut state, command)
            })
    }
    fn state(&self) -> &EditorWorkspace {
        self.app.world().resource::<EditorWorkspace>()
    }
    fn group(&self, view: ViewId) -> NodeId {
        self.state()
            .layout
            .snapshot()
            .location(view)
            .unwrap_or_else(|| panic!("visible view"))
            .0
    }
}

#[test]
fn restored_missing_source_fails_once_without_replacing_admitted_documents() -> TestResult {
    let mut f = Fixture::new()?;
    let missing = ViewId::new();
    f.command(Command::Open {
        id: missing,
        view: View {
            kind: "document".into(),
            restore_key: Some("missing.scene.ron".into()),
        },
        destination: Destination::Tabs {
            group: f.group(f.b),
            index: 2,
        },
    })?;
    f.app
        .world_mut()
        .resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            panels::mount(world, &mut state)?;
            document_views::poll(world, &mut state);
            assert!(state.documents.failure.take().is_some());
            assert!(!state.documents.opening());
            assert!(!state.panels.restore_sources.contains_key(&missing));
            let opened: Vec<_> = state.documents.opened.keys().copied().collect();
            document_views::poll(world, &mut state);
            assert!(
                state.documents.failure.is_none(),
                "Failed restoration must not retry every frame"
            );
            assert_eq!(
                state.documents.opened.keys().copied().collect::<Vec<_>>(),
                opened
            );
            assert!(state.layout.snapshot().views.contains_key(&missing));
            Ok::<_, String>(())
        })?;
    f.command(Command::Activate(f.a))?;
    assert_eq!(f.state().documents.active, Some(f.first.id));
    Ok(())
}

#[test]
fn closing_active_document_activates_visible_sibling_and_last_close_clears_authority() -> TestResult
{
    let mut f = Fixture::new()?;
    f.command(Command::Hide(f.b))?;
    assert_eq!(f.state().documents.active, Some(f.first.id));
    assert_eq!(
        f.state().documents.opened[&f.second.id].selected,
        Some(f.second.root.into())
    );
    f.command(Command::Hide(f.a))?;
    assert_eq!(f.state().documents.active, None);
    assert_eq!(
        f.state().documents.opened.len(),
        2,
        "Hide retains accepted sessions"
    );
    Ok(())
}

#[test]
fn showing_hidden_document_activates_it_without_an_explicit_activate() -> TestResult {
    let mut f = Fixture::new()?;
    f.command(Command::Hide(f.a))?;
    f.command(Command::Show {
        view: f.a,
        destination: Destination::Tabs {
            group: f.group(f.b),
            index: 0,
        },
    })?;
    assert_eq!(f.state().documents.active, Some(f.first.id));
    Ok(())
}

#[test]
fn showing_asset_preview_clears_document_authority_and_close_restores_sibling() -> TestResult {
    let mut f = Fixture::new()?;
    let asset = ViewId::new();
    f.command(Command::Open {
        id: asset,
        view: View {
            kind: "asset-preview".into(),
            restore_key: Some("image.png".into()),
        },
        destination: Destination::Tabs {
            group: f.group(f.b),
            index: 2,
        },
    })?;
    f.command(Command::Hide(asset))?;
    f.command(Command::Activate(f.b))?;
    f.command(Command::Show {
        view: asset,
        destination: Destination::Tabs {
            group: f.group(f.b),
            index: 2,
        },
    })?;
    assert_eq!(f.state().documents.active, None);
    f.command(Command::Hide(asset))?;
    assert_eq!(f.state().documents.active, Some(f.second.id));
    Ok(())
}

#[test]
fn tool_focus_does_not_choose_another_document_and_failed_commands_preserve_authority() -> TestResult
{
    let mut f = Fixture::new()?;
    let tool = f
        .state()
        .layout
        .snapshot()
        .views
        .iter()
        .find_map(|(id, view)| (view.kind == "hierarchy").then_some(*id))
        .ok_or("tool")?;
    f.command(Command::Activate(tool))?;
    assert_eq!(f.state().documents.active, Some(f.second.id));
    // Even a tool sharing the content group must not choose an inactive sibling.
    f.command(Command::Move {
        selection: Selection::View(tool),
        destination: Destination::Tabs {
            group: f.group(f.b),
            index: 0,
        },
    })?;
    f.command(Command::Activate(tool))?;
    assert_eq!(f.state().documents.active, Some(f.second.id));
    assert!(f.command(Command::Activate(ViewId::new())).is_err());
    assert_eq!(f.state().documents.active, Some(f.second.id));
    Ok(())
}

#[test]
fn moving_inactive_document_activates_it_and_floating_active_document_preserves_it() -> TestResult {
    let mut f = Fixture::new()?;
    let host = HostId::new();
    {
        let mut state = f.app.world_mut().resource_mut::<EditorWorkspace>();
        let surface = state.surfaces[&state.host];
        state.surfaces.insert(host, surface);
    }
    f.command(Command::Float {
        selection: Selection::View(f.a),
        host,
    })?;
    assert_eq!(f.state().documents.active, Some(f.first.id));
    f.command(Command::Move {
        selection: Selection::View(f.a),
        destination: Destination::Tabs {
            group: f.group(f.b),
            index: 1,
        },
    })?;
    assert_eq!(f.state().documents.active, Some(f.first.id));
    // Production Float publishes a prepared candidate directly, then redraws.
    f.app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
            let host = HostId::new();
            let surface = state.surfaces[&state.host];
            state.surfaces.insert(host, surface);
            let candidate = state
                .layout
                .prepare(Command::Float {
                    selection: Selection::View(f.a),
                    host,
                })
                .map_err(|e| e.to_string())?;
            state.layout.commit(candidate).map_err(|e| e.to_string())?;
            redraw(world, &mut state)
        },
    )?;
    assert_eq!(f.state().documents.active, Some(f.first.id));
    Ok(())
}

#[test]
fn closing_last_content_in_a_host_uses_another_visible_group_then_clears() -> TestResult {
    let mut f = Fixture::new()?;
    let host = HostId::new();
    {
        let mut state = f.app.world_mut().resource_mut::<EditorWorkspace>();
        let surface = state.surfaces[&state.host];
        state.surfaces.insert(host, surface);
    }
    f.command(Command::Float {
        selection: Selection::View(f.a),
        host,
    })?;
    f.command(Command::Activate(f.a))?;
    f.command(Command::HideHost(host))?;
    assert_eq!(f.state().documents.active, Some(f.second.id));
    f.command(Command::Hide(f.b))?;
    assert_eq!(f.state().documents.active, None);
    assert_eq!(f.state().documents.opened.len(), 2);
    Ok(())
}

#[test]
fn browser_float_redock_hide_show_reset_respects_the_actual_group_membership() -> TestResult {
    // Redock uses the first main-host group by ID, not a reserved tool group.
    // Cover both possible destinations without relying on random UUID ordering.
    for into_documents in [false, true] {
        let mut f = Fixture::new()?;
        let assets = f
            .state()
            .layout
            .snapshot()
            .views
            .iter()
            .find_map(|(id, view)| (view.kind == "assets").then_some(*id))
            .ok_or("Assets missing")?;
        let hierarchy = f
            .state()
            .layout
            .snapshot()
            .views
            .iter()
            .find_map(|(id, view)| (view.kind == "hierarchy").then_some(*id))
            .ok_or("Hierarchy missing")?;
        let host = HostId::new();
        let group = f.group(assets);
        f.app.world_mut().resource_scope(
            |world, mut state: Mut<EditorWorkspace>| -> Result<(), String> {
                let candidate = state
                    .layout
                    .prepare(Command::Float {
                        selection: Selection::Group(group),
                        host,
                    })
                    .map_err(|e| e.to_string())?;
                state.layout.commit(candidate).map_err(|e| e.to_string())?;
                let surface = state.surfaces[&state.host];
                state.surfaces.insert(host, surface);
                redraw(world, &mut state)?;
                let window = world
                    .get_mut::<Window>(surface.window)
                    .ok_or("Window missing")?;
                window.into_inner().resolution.set(480., 560.);
                redraw(world, &mut state)
            },
        )?;
        assert_eq!(f.state().documents.active, Some(f.second.id));
        let target = f.group(if into_documents { f.b } else { hierarchy });
        f.command(Command::Move {
            selection: Selection::Group(f.group(assets)),
            destination: Destination::Tabs {
                group: target,
                index: 0,
            },
        })?;
        assert_eq!(
            f.state().documents.active,
            Some(f.second.id),
            "Tool redock must not select content"
        );
        let tabs = match &f.state().layout.snapshot().nodes[&target] {
            kedra_docking::model::Node::Group { tabs, .. } => tabs.clone(),
            _ => return Err("Redock target is not a group".into()),
        };
        // Production Hide group dispatches Hide for every tab, including documents.
        for view in tabs {
            f.command(Command::Hide(view))?;
        }
        let expected = (!into_documents).then_some(f.second.id);
        assert_eq!(f.state().documents.active, expected);
        for view in [f.a, f.b] {
            assert_eq!(
                f.state().layout.snapshot().hidden.contains_key(&view),
                into_documents
            );
        }
        let destination = input::bookmark_destination(f.state(), assets);
        f.command(Command::Show {
            view: assets,
            destination,
        })?;
        assert_eq!(
            f.state().documents.active,
            expected,
            "Show Assets must not reopen hidden documents"
        );
        assert_eq!(
            f.state().documents.opened.len(),
            2,
            "Admission is not visibility or activation"
        );
        f.app
            .world_mut()
            .resource_scope(|world, mut state: Mut<EditorWorkspace>| {
                panels::reset(world, &mut state)
            })?;
        assert_eq!(f.state().documents.active, expected);
        if !into_documents {
            let node = &f.state().layout.snapshot().nodes[&f.group(f.b)];
            assert!(
                matches!(node, kedra_docking::model::Node::Group { active, .. } if *active == f.b)
            );
            assert_ne!(f.group(f.b), f.group(assets));
        }
    }
    Ok(())
}
