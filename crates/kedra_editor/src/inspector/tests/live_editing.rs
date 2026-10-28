//! Source adapter acknowledgment, raw numeric echo and gesture history, on a CPU host.
//! Native dispatch/IME belongs to kedra_ui; real frame acceptance is separate.
use super::*;
use kedra_ui::fields::{TextField, TextFieldEvent};

#[derive(Resource, Default)]
struct Events(Vec<TextFieldEvent>);

#[test]
fn live_native_numbers_retain_raw_text_and_publish_one_undo_group_without_saving() -> TestResult {
    let mut app = crate::workspace::browser_test_app()?;
    app.add_plugins((
        bevy::input::InputPlugin,
        bevy::input_focus::InputFocusPlugin,
        bevy::input_focus::InputDispatchPlugin,
        bevy::text::TextPlugin,
        bevy::ui_widgets::TextInputPlugin,
        kedra_ui::native_input::NativeInputPlugin,
        kedra_inspector::controls::InspectorControlsPlugin,
    ));
    app.add_message::<bevy::window::Ime>()
        .init_resource::<bevy::picking::events::PointerState>()
        .init_resource::<UiScale>()
        .add_message::<bevy::picking::events::PointerRelease>()
        .init_resource::<Events>()
        .add_observer(|event: On<TextFieldEvent>, mut events: ResMut<Events>| {
            events.0.push(event.clone());
        });
    app.finish();
    app.cleanup();
    let parent = app
        .world()
        .resource::<kedra_desktop::DesktopSurface>()
        .root
        .ok_or("root")?;
    let id = ViewId::new();
    let mut panel = Panel::mount(app.world_mut(), parent, id)?;
    let dir = tempfile::tempdir()?;
    let mut document = kedra_scene::document::PrefabDocument::new("Live source");
    document.nodes[0].components.insert(
        "ui.layout".into(),
        kedra_scene::document::ComponentData::ron(1, "(node:(width:Px(0.0)))")?,
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
    for _ in 0..8 {
        app.update();
        panel.refresh_presentation();
        panel.tick(app.world_mut(), id, &mut documents)?;
    }
    let input = app
        .world_mut()
        .query::<(Entity, &kedra_inspector::fields::ReflectedField)>()
        .iter(app.world())
        .find(|(_, field)| field.path.to_string() == ".node.width")
        .map(|(entity, _)| entity)
        .ok_or("width")?;
    assert_eq!(
        app.world()
            .get::<Text>(panel.element("source")?)
            .ok_or("source label")?
            .0,
        "source.prefab.ron"
    );
    assert!(
        app.world()
            .get::<kedra_ui::fields::LiveTextField>(input)
            .is_some()
    );
    // This CPU host does not install Bevy's visibility propagation/render systems.
    let mut ancestor = Some(input);
    while let Some(entity) = ancestor {
        ancestor = app.world().get::<ChildOf>(entity).map(ChildOf::parent);
        app.world_mut()
            .entity_mut(entity)
            .insert(InheritedVisibility::VISIBLE);
    }
    app.insert_resource(bevy::input_focus::InputFocus::from_entity(input));
    let baseline = documents.revisions();
    for value in ["1", "1.", "1.5", "invalid", "15"] {
        {
            let mut text = app
                .world_mut()
                .get_mut::<bevy::text::EditableText>(input)
                .ok_or("text")?;
            text.queue_edit(bevy::text::TextEdit::SelectAll);
            text.queue_edit(bevy::text::TextEdit::Insert(value.into()));
        }
        let before = documents.revisions();
        let mut notifications = Vec::new();
        for _ in 0..3 {
            app.update();
            let events = std::mem::take(&mut app.world_mut().resource_mut::<Events>().0);
            for event in events {
                notifications.push(format!("{:?}", event.outcome));
                panel.property_input(app.world_mut(), &mut documents, event)?;
            }
            panel.refresh_presentation();
            panel.tick(app.world_mut(), id, &mut documents)?;
        }
        assert_eq!(
            app.world()
                .get::<bevy::text::EditableText>(input)
                .ok_or("text")?
                .value(),
            value
        );
        if value == "invalid" {
            assert_eq!(documents.revisions(), before);
            assert!(panel.has_drafts(app.world()));
        } else {
            assert!(
                !panel.has_drafts(app.world()),
                "{value} must be acknowledged: events={notifications:?}, accepted={:?}",
                app.world().get::<TextField>(input).map(TextField::accepted),
            );
        }
        let target = documents
            .target(document.id, document.root.into())
            .ok_or("target")?;
        let registry = app.world().resource::<AppTypeRegistry>().clone();
        let types = registry.read();
        let inspection = documents.inspect(&target)?;
        let current = inspection.value(
            "ui.layout",
            &types,
            app.world()
                .resource::<kedra_scene::reflection::ComponentPolicies>(),
        )?;
        let expected = if value == "invalid" {
            1.5
        } else {
            value.parse::<f32>()?
        };
        assert_eq!(
            current
                .try_downcast_ref::<kedra_ui::authored::components::UiLayout>()
                .ok_or("layout")?
                .node
                .width,
            px(expected)
        );
    }
    assert_ne!(documents.revisions(), baseline);
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let policies = app
        .world()
        .resource::<kedra_scene::reflection::ComponentPolicies>();
    let change = documents.prepare_history(false, &types, policies)?;
    documents.publish_change(change);
    assert!(
        !documents.can_history(false),
        "one gesture is one source Undo"
    );
    let target = documents
        .target(document.id, document.root.into())
        .ok_or("target")?;
    let inspection = documents.inspect(&target)?;
    let value = inspection.value("ui.layout", &types, policies)?;
    assert_eq!(
        value
            .try_downcast_ref::<kedra_ui::authored::components::UiLayout>()
            .ok_or("layout")?
            .node
            .width,
        px(0)
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("source.prefab.ron"))?,
        source
    );
    Ok(())
}
