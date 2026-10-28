//! Headless host lifecycle: typed payload ownership, cancellation and retired scopes.
use super::*;
use crate::{documents::Documents, workspace::EditorWorkspace};
use bevy::{
    camera::NormalizedRenderTarget,
    ecs::system::RunSystemOnce,
    picking::{
        events::Pointer,
        pointer::{Location, PointerButton, PointerId},
    },
    window::WindowRef,
};
use kedra_scene::document::PrefabDocument;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn admit(
    app: &mut App,
    directory: &std::path::Path,
) -> Result<PrefabDocument, Box<dyn std::error::Error>> {
    let document = PrefabDocument::new("Drag source");
    std::fs::write(directory.join("source.prefab.ron"), document.serialize()?)?;
    app.world_mut()
        .resource_scope(|world, mut state: Mut<EditorWorkspace>| {
            state
                .documents
                .request(world, directory.into(), "source.prefab.ron".into())
        })?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let ready = app
            .world_mut()
            .resource_mut::<EditorWorkspace>()
            .documents
            .poll();
        if ready.is_some() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "source admission timed out"
        );
        std::thread::yield_now();
    }
    Ok(document)
}

#[test]
fn pointer_cancellation_revokes_payload_without_a_hover_target_and_preserves_foreign_drags()
-> TestResult {
    let mut app = crate::workspace::browser_test_app()?;
    install(&mut app);
    let dir = tempfile::tempdir()?;
    admit(&mut app, dir.path())?;
    let before = std::fs::read(dir.path().join("source.prefab.ron"))?;
    let asset = crate::context::asset(app.world(), "game://source.prefab.ron").ok_or("asset")?;
    let expected = DragPayload::Assets(vec![asset]);
    let window = app.world_mut().spawn(Window::default()).id();
    let foreign_window = app.world_mut().spawn(Window::default()).id();
    let owner = app.world_mut().spawn(ChildOf(window)).id();
    let other_pointer = app.world_mut().spawn(ChildOf(window)).id();
    let foreign = app.world_mut().spawn(ChildOf(foreign_window)).id();
    let generic = app.world_mut().spawn(ChildOf(window)).id();
    for entity in [owner, other_pointer, foreign, generic] {
        begin(app.world_mut(), entity, expected.clone())?;
    }
    app.world_mut()
        .get_mut::<Dragged>(owner)
        .ok_or("capture")?
        .pointer = Some(PointerId::Mouse);
    app.world_mut()
        .get_mut::<Dragged>(other_pointer)
        .ok_or("capture")?
        .pointer = Some(PointerId::Touch(7));
    app.world_mut()
        .get_mut::<Dragged>(foreign)
        .ok_or("capture")?
        .pointer = Some(PointerId::Mouse);
    app.world_mut().write_message(PointerInput::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Entity(window).normalize(None).ok_or("window")?,
            ),
            position: Vec2::ZERO,
        },
        PointerAction::Cancel,
    ));
    app.world_mut()
        .run_system_once(pointer_cancel)
        .map_err(|error| error.to_string())?;
    assert!(payload(app.world(), owner).is_none());
    assert!(payload(app.world(), generic).is_none());
    assert_eq!(payload(app.world(), other_pointer), Some(expected.clone()));
    assert_eq!(payload(app.world(), foreign), Some(expected));
    assert!(app.world().entities().contains(owner));
    assert_eq!(std::fs::read(dir.path().join("source.prefab.ron"))?, before);
    assert!(
        !app.world()
            .resource::<EditorWorkspace>()
            .documents
            .can_history(false)
    );
    Ok(())
}

#[test]
fn typed_drag_end_and_scope_retirement_revoke_payload_without_source_edits() -> TestResult {
    let mut app = crate::workspace::browser_test_app()?;
    install(&mut app);
    let dir = tempfile::tempdir()?;
    let document = admit(&mut app, dir.path())?;
    let before = std::fs::read(dir.path().join("source.prefab.ron"))?;
    let asset = crate::context::asset(app.world(), "game://source.prefab.ron").ok_or("asset")?;
    let expected = DragPayload::Assets(vec![asset]);
    let owner = app.world_mut().spawn_empty().id();
    begin(app.world_mut(), owner, expected.clone())?;
    assert_eq!(payload(app.world(), owner), Some(expected.clone()));
    assert!(begin(app.world_mut(), owner, DragPayload::Assets(vec![])).is_err());
    assert_eq!(payload(app.world(), owner), Some(expected.clone()));
    let window = app.world_mut().spawn(Window::default()).id();
    let pointer = Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Entity(window).normalize(None).ok_or("window")?,
            ),
            position: Vec2::ZERO,
        },
    );
    app.world_mut().trigger(PointerDragEnd {
        entity: owner,
        pointer,
        button: PointerButton::Primary,
        distance: Vec2::ZERO,
    });
    app.world_mut().flush();
    assert!(payload(app.world(), owner).is_none());
    begin(app.world_mut(), owner, expected.clone())?;
    let state = app.world().resource::<EditorWorkspace>();
    assert!(
        state
            .documents
            .begin_edit("Probe", &["game://source.prefab.ron".into()])
            .is_ok()
    );
    assert!(state.documents.selection.items().is_empty());
    assert!(state.documents.active.is_none());
    assert!(state.documents.opened.is_empty());
    assert!(
        crate::context::library(app.world())
            .ok_or("library")?
            .node(document.id, document.root.into())
            .is_some()
    );
    app.world_mut().resource_mut::<EditorWorkspace>().documents = Documents::default();
    assert!(payload(app.world(), owner).is_none());
    assert!(begin(app.world_mut(), owner, expected).is_err());
    assert_eq!(std::fs::read(dir.path().join("source.prefab.ron"))?, before);
    Ok(())
}

#[test]
fn mixed_shared_selection_does_not_silently_become_one_browser_asset_drag() -> TestResult {
    let mut app = crate::workspace::browser_test_app()?;
    let dir = tempfile::tempdir()?;
    let document = admit(&mut app, dir.path())?;
    let library = crate::context::library(app.world()).ok_or("library")?;
    let asset = library.asset("game://source.prefab.ron").ok_or("asset")?;
    let node = library
        .node(document.id, document.root.into())
        .ok_or("node")?;
    crate::context::select(
        app.world_mut(),
        vec![SelectionItem::Asset(asset), SelectionItem::Node(node)],
    )?;
    let before = crate::context::selection(app.world())
        .ok_or("selection")?
        .clone();
    let owner = app.world_mut().spawn_empty().id();
    begin_browser(
        app.world_mut(),
        owner,
        "game://source.prefab.ron",
        PointerId::Mouse,
    );
    assert!(app.world().get::<Dragged>(owner).is_none());
    assert_eq!(crate::context::selection(app.world()), Some(&before));
    Ok(())
}
