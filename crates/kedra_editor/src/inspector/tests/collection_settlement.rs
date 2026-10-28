//! The editor validates immediate complete proposals; the array owns no source draft.
use super::*;
use kedra_inspector::controls::{Control, ControlEvent};
use kedra_ui::authored::components::UiClasses;
#[derive(Resource, Default)]
struct Proposals(Vec<ControlEvent>);
fn endpoint(world: &mut World, suffix: &str) -> Result<Entity, Box<dyn std::error::Error>> {
    world
        .query::<(Entity, &kedra_ui::ElementId)>()
        .iter(world)
        .find(|(_, id)| id.0.ends_with(suffix))
        .map(|(entity, _)| entity)
        .ok_or_else(|| format!("Missing collection endpoint {suffix}").into())
}
fn activate(app: &mut App, suffix: &str) -> TestResult {
    let entity = endpoint(app.world_mut(), suffix)?;
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity });
    app.world_mut().flush();
    app.update();
    Ok(())
}
fn refresh(app: &mut App, panel: &mut Panel, id: ViewId, documents: &mut Documents) -> TestResult {
    for _ in 0..8 {
        app.update();
        panel.refresh_presentation();
        panel.tick(app.world_mut(), id, documents)?;
    }
    Ok(())
}
fn names(
    app: &App,
    documents: &Documents,
    target: &crate::documents::inspection::Target,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let types = app.world().resource::<AppTypeRegistry>().read();
    Ok(documents
        .inspect(target)?
        .value(
            "ui.classes",
            &types,
            app.world()
                .resource::<kedra_scene::reflection::ComponentPolicies>(),
        )?
        .try_downcast_ref::<UiClasses>()
        .ok_or("classes")?
        .names
        .clone())
}
#[test]
fn immediate_remove_has_one_source_undo_and_rejected_or_replaced_proposals_preserve_sources()
-> TestResult {
    let mut app = crate::workspace::browser_test_app()?;
    app.add_plugins((
        bevy::text::TextPlugin,
        kedra_inspector::controls::InspectorControlsPlugin,
    ))
    .init_resource::<Proposals>()
    .add_observer(|event: On<ControlEvent>, mut events: ResMut<Proposals>| {
        events.0.push(event.clone())
    });
    let parent = app
        .world()
        .resource::<kedra_desktop::DesktopSurface>()
        .root
        .ok_or("root")?;
    let id = ViewId::new();
    let mut panel = Panel::mount(app.world_mut(), parent, id)?;
    let dir = tempfile::tempdir()?;
    let mut document = kedra_scene::document::PrefabDocument::new("Collection source");
    document.nodes[0].components.insert(
        "ui.classes".into(),
        kedra_scene::document::ComponentData::ron(1, "(names:[\"first\",\"второй\"])")?,
    );
    let source = document.serialize()?;
    std::fs::write(dir.path().join("source.prefab.ron"), &source)?;
    let mut documents = Documents::default();
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
    refresh(&mut app, &mut panel, id, &mut documents)?;
    let target = panel.state.target.clone().ok_or("target")?;
    let input = endpoint(app.world_mut(), "/items/2/[]/value")?;
    assert!(app.world().get::<editing::SourceField>(input).is_some());
    activate(&mut app, "/items/1/grip")?;
    refresh(&mut app, &mut panel, id, &mut documents)?;
    let before_context = panel.history_context(app.world(), id, false);
    activate(&mut app, "/remove")?;
    let accepted = app
        .world_mut()
        .resource_mut::<Proposals>()
        .0
        .pop()
        .ok_or("proposal")?;
    panel.property_control(app.world_mut(), &mut documents, accepted.clone())?;
    refresh(&mut app, &mut panel, id, &mut documents)?;
    assert_eq!(names(&app, &documents, &target)?, ["второй"]);
    assert!(!panel.has_drafts(app.world()));
    assert_eq!(endpoint(app.world_mut(), "/items/2/[]/value")?, input);
    let after_context = panel.history_context(app.world(), id, false);
    let before = documents.revisions();
    panel.property_control(app.world_mut(), &mut documents, accepted)?;
    assert_eq!(documents.revisions(), before);
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let policies = app
        .world()
        .resource::<kedra_scene::reflection::ComponentPolicies>()
        .clone();
    let undo = documents.prepare_history(false, &types, &policies)?;
    documents.publish_change(undo);
    panel.restore_history_context(before_context, false);
    refresh(&mut app, &mut panel, id, &mut documents)?;
    assert_eq!(names(&app, &documents, &target)?, ["first", "второй"]);
    assert_eq!(endpoint(app.world_mut(), "/items/2/[]/value")?, input);
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(input)
            .ok_or("retained input")?
            .value(),
        "второй"
    );
    assert!(!documents.can_history(false));
    let redo = documents.prepare_history(true, &types, &policies)?;
    documents.publish_change(redo);
    panel.restore_history_context(after_context, false);
    refresh(&mut app, &mut panel, id, &mut documents)?;
    activate(&mut app, "/add")?;
    let original = app
        .world_mut()
        .resource_mut::<Proposals>()
        .0
        .pop()
        .ok_or("proposal")?;
    let mut invalid = original.clone();
    invalid.value = std::sync::Arc::new(vec!["duplicate".to_owned(), "duplicate".to_owned()]);
    let before = documents.revisions();
    panel.property_control(app.world_mut(), &mut documents, invalid)?;
    assert_eq!(documents.revisions(), before);
    assert_eq!(names(&app, &documents, &target)?, ["второй"]);
    assert!(
        !app.world()
            .get::<Text>(original.binding.diagnostic)
            .ok_or("diagnostic")?
            .0
            .is_empty()
    );
    // A response for a different value cannot retire the original request.
    assert!(panel.has_drafts(app.world()));
    kedra_inspector::collections::respond(app.world_mut(), &original, false);
    refresh(&mut app, &mut panel, id, &mut documents)?;
    assert!(!panel.has_drafts(app.world()));
    activate(&mut app, "/add")?;
    let stale = app
        .world_mut()
        .resource_mut::<Proposals>()
        .0
        .pop()
        .ok_or("proposal")?;
    app.world_mut()
        .entity_mut(stale.entity)
        .insert(Control(std::sync::Arc::new(
            kedra_inspector::controls::ControlBinding {
                token: stale.binding.token,
                path: stale.binding.path.clone(),
                diagnostic: stale.binding.diagnostic,
            },
        )));
    panel.property_control(app.world_mut(), &mut documents, stale)?;
    assert_eq!(documents.revisions(), before);
    refresh(&mut app, &mut panel, id, &mut documents)?;
    assert!(!panel.has_drafts(app.world()));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("source.prefab.ron"))?,
        source
    );
    Ok(())
}
