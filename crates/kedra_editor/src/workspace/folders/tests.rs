//! Real folder effects through the editor coordinator; native menus remain separate acceptance.
use super::*;
use crate::workspace::test_support::{setup, sources};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn pending_folder_operation_refuses_inspector_publication_until_its_owner_finishes() -> TestResult {
    use kedra_inspector::controls::{Control, ControlEvent};
    let mut app = crate::workspace::browser_test_app()?;
    let root = app
        .world()
        .resource::<kedra_desktop::DesktopSurface>()
        .root
        .ok_or("surface")?;
    let view = ViewId::new();
    let mut panel = crate::inspector::Panel::mount(app.world_mut(), root, view)?;
    let directory = tempfile::tempdir()?;
    let mut document = kedra_scene::document::PrefabDocument::new("Input owner");
    document.nodes[0].components.insert(
        "ui.editable".into(),
        kedra_scene::document::ComponentData::ron(1, "(autofocus:false,select_all:true)")?,
    );
    std::fs::write(
        directory.path().join("Input.prefab.ron"),
        document.serialize()?,
    )?;
    let mut documents = crate::documents::Documents::default();
    documents.request(
        app.world(),
        directory.path().into(),
        "Input.prefab.ron".into(),
    )?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let ready = loop {
        if let Some(ready) = documents.poll() {
            break ready;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Source admission timed out"
        );
        std::thread::yield_now();
    };
    documents.published(ready, ViewId::new(), document.root.into());
    panel.tick(app.world_mut(), view, &mut documents)?;
    let (entity, binding) = app
        .world_mut()
        .query::<(Entity, &Control)>()
        .iter(app.world())
        .find(|(_, control)| control.0.path.to_string() == ".autofocus")
        .map(|(id, control)| (id, control.0.clone()))
        .ok_or_else(|| {
            format!(
                "autofocus control missing; controls: {:?}",
                app.world_mut()
                    .query::<&Control>()
                    .iter(app.world())
                    .map(|c| c.0.path.to_string())
                    .collect::<Vec<_>>()
            )
        })?;
    let event = ControlEvent {
        entity,
        binding,
        value: std::sync::Arc::new(true),
    };
    let revision = documents.revisions();
    let task =
        Wakeup::for_world(app.world()).spawn(IoTaskPool::get(), std::future::pending::<Outcome>());
    app.world_mut().insert_resource(Pending {
        request: Request {
            view,
            root: directory.path().into(),
            path: PathBuf::new(),
            mode: Mode::Create,
        },
        change: Change::Create("Folder".into()),
        task,
    });
    panel.property_control(app.world_mut(), &mut documents, event.clone())?;
    assert_eq!(documents.revisions(), revision);
    assert!(documents.update_clean());
    app.world_mut().remove_resource::<Pending>();
    panel.property_control(app.world_mut(), &mut documents, event)?;
    assert_ne!(documents.revisions(), revision);
    assert!(!documents.update_clean());
    Ok(())
}

fn finish(app: &mut App) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while pending(app.world()) {
        assert!(
            std::time::Instant::now() < deadline,
            "Folder worker did not finish"
        );
        app.update();
        std::thread::yield_now();
    }
}

#[test]
fn renamed_prefab_folder_keeps_identity_and_referenced_deletion_preserves_sources() -> TestResult {
    use kedra_scene::document::{InstanceId, NestedInstance, PrefabDocument};
    let directory = tempfile::tempdir()?;
    let project = kedra_project::create(directory.path(), "References")?;
    let root = project.assets.clone();
    std::fs::create_dir(root.join("Prefabs"))?;
    let prefab = PrefabDocument::new("Player");
    let mut parent = PrefabDocument::new("Arena");
    parent.instances.push(NestedInstance {
        id: InstanceId::new(),
        asset: prefab.id,
        parent: parent.root.into(),
        bindings: default(),
        overrides: Vec::new(),
    });
    std::fs::write(root.join("Prefabs/Player.prefab.ron"), prefab.serialize()?)?;
    std::fs::write(root.join("Arena.prefab.ron"), parent.serialize()?)?;
    let (mut app, _, _) = setup()?;
    install(&mut app);
    sources(app.world_mut())?;
    app.world_mut()
        .insert_resource(crate::workspace::test_support::project_manager(
            project, None,
        )?);
    let view = app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<_, String> {
            panels::preset(&mut state)?;
            panels::mount(world, &mut state)?;
            let view = *state.panels.browsers.keys().next().ok_or("browser")?;
            submit(
                world,
                &mut state,
                Request {
                    view,
                    root: root.clone(),
                    path: "Prefabs".into(),
                    mode: Mode::Rename,
                },
                "Objects",
            )?;
            Ok(view)
        },
    )?;
    finish(&mut app);
    assert!(!root.join("Prefabs").exists());
    assert_eq!(
        PrefabDocument::parse(&std::fs::read_to_string(
            root.join("Objects/Player.prefab.ron")
        )?)?
        .id,
        prefab.id
    );
    assert!(
        app.world()
            .resource::<EditorWorkspace>()
            .documents
            .contains(prefab.id)
    );
    app.world_mut()
        .resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            start(
                world,
                &mut state,
                Request {
                    view,
                    root: root.clone(),
                    path: "Objects".into(),
                    mode: Mode::Delete,
                },
                Change::Delete("Objects".into()),
            )
        })?;
    finish(&mut app);
    assert_eq!(
        std::fs::read_to_string(root.join("Arena.prefab.ron"))?,
        parent.serialize()?
    );
    assert_eq!(
        std::fs::read_to_string(root.join("Objects/Player.prefab.ron"))?,
        prefab.serialize()?
    );
    assert!(
        app.world()
            .resource::<EditorWorkspace>()
            .documents
            .contains(prefab.id)
    );
    Ok(())
}
#[test]
fn folder_worker_preserves_project_ownership_and_refreshes_browser_after_create_rename_delete()
-> TestResult {
    let directory = tempfile::tempdir()?;
    let project = kedra_project::create(directory.path(), "Folders")?;
    let root = project.assets.clone();
    let (mut app, _, _) = setup()?;
    install(&mut app);
    sources(app.world_mut())?;
    app.world_mut()
        .insert_resource(crate::workspace::test_support::project_manager(
            project, None,
        )?);
    let view = app.world_mut().resource_scope(
        |world, mut state: Mut<EditorWorkspace>| -> Result<_, String> {
            panels::preset(&mut state)?;
            panels::mount(world, &mut state)?;
            let view = *state.panels.browsers.keys().next().ok_or("browser")?;
            let request = Request {
                view,
                root: root.clone(),
                path: PathBuf::new(),
                mode: Mode::Create,
            };
            submit(world, &mut state, request.clone(), "Текстуры")?;
            assert!(submit(world, &mut state, request, "Concurrent").is_err());
            assert!(settlement::source_edits(world, &state));
            Ok(view)
        },
    )?;
    finish(&mut app);
    assert!(root.join("Текстуры").is_dir());
    assert!(!root.join("Concurrent").exists());
    app.world_mut()
        .resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            submit(
                world,
                &mut state,
                Request {
                    view,
                    root: root.clone(),
                    path: "Текстуры".into(),
                    mode: Mode::Rename,
                },
                "Textures",
            )
        })?;
    finish(&mut app);
    assert!(root.join("Textures").is_dir());
    assert!(!root.join("Текстуры").exists());
    app.world_mut()
        .resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            start(
                world,
                &mut state,
                Request {
                    view,
                    root: root.clone(),
                    path: "Textures".into(),
                    mode: Mode::Delete,
                },
                Change::Delete("Textures".into()),
            )
        })?;
    finish(&mut app);
    assert!(!root.join("Textures").exists());
    assert!(
        app.world()
            .resource::<EditorWorkspace>()
            .panels
            .browsers
            .contains_key(&view)
    );
    app.world_mut()
        .resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            assert!(
                submit(
                    world,
                    &mut state,
                    Request {
                        view,
                        root: directory.path().join("foreign"),
                        path: PathBuf::new(),
                        mode: Mode::Create
                    },
                    "Foreign"
                )
                .is_err()
            );
        });
    assert!(!pending(app.world()));
    Ok(())
}
