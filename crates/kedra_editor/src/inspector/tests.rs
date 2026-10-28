use super::*;
use crate::documents::Documents;
use kedra_docking::model::ViewId;
type TestResult = Result<(), Box<dyn std::error::Error>>;
mod collection_settlement;
mod layout_settlement;
mod live_editing;

#[test]
fn source_panel_retains_controls_and_locked_identity_without_viewport_or_source_writes()
-> TestResult {
    let mut app = crate::workspace::browser_test_app()?;
    let parent = app
        .world()
        .resource::<kedra_desktop::DesktopSurface>()
        .root
        .ok_or("root")?;
    let id = ViewId::new();
    let mut panel = Panel::mount(app.world_mut(), parent, id)?;
    let root = panel.root();
    let mut documents = Documents::default();
    panel.tick(app.world_mut(), id, &mut documents)?;
    let dir = tempfile::tempdir()?;
    let mut document = kedra_scene::document::PrefabDocument::new("Inspector source");
    document.nodes[0].components.insert(
        "ui.editable".into(),
        kedra_scene::document::ComponentData::ron(1, "(autofocus:false,select_all:true)")?,
    );
    let source = document.serialize()?;
    std::fs::write(dir.path().join("source.prefab.ron"), &source)?;
    documents.request(app.world(), dir.path().into(), "source.prefab.ron".into())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let ready = loop {
        if let Some(ready) = documents.poll() {
            break ready;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "source admission timed out"
        );
        std::thread::yield_now();
    };
    documents.published(ready, ViewId::new(), document.root.into());
    panel.tick(app.world_mut(), id, &mut documents)?;
    let inputs = |world: &mut World| {
        world
            .query_filtered::<(Entity, Has<bevy::ui::Checked>), With<bevy::ui_widgets::Checkbox>>()
            .iter(world)
            .collect::<Vec<_>>()
    };
    let initial = inputs(app.world_mut());
    assert!(initial.iter().any(|(_, checked)| *checked));
    panel.tick(app.world_mut(), id, &mut documents)?;
    assert_eq!(inputs(app.world_mut()), initial);
    panel.action(app.world_mut(), Action::Lock)?;
    documents.active = None;
    documents.selection.clear();
    panel.tick(app.world_mut(), id, &mut documents)?;
    assert_eq!(inputs(app.world_mut()), initial);
    panel.action(app.world_mut(), Action::Lock)?;
    panel.tick(app.world_mut(), id, &mut documents)?;
    assert!(inputs(app.world_mut()).is_empty());
    assert_eq!(root, panel.root());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("source.prefab.ron"))?,
        source
    );
    Ok(())
}
