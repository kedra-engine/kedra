//! Editor disclosure guards must settle every retained native Layout input.
use super::*;
use kedra_ui::fields::TextField;

#[test]
fn source_layout_refresh_and_cancel_allow_nested_disclosure_without_retiring_inputs() -> TestResult
{
    let mut app = crate::workspace::browser_test_app()?;
    app.add_plugins((
        bevy::text::TextPlugin,
        kedra_inspector::controls::InspectorControlsPlugin,
    ));
    let parent = app
        .world()
        .resource::<kedra_desktop::DesktopSurface>()
        .root
        .ok_or("root")?;
    let id = ViewId::new();
    let mut panel = Panel::mount(app.world_mut(), parent, id)?;
    let dir = tempfile::tempdir()?;
    let mut document = kedra_scene::document::PrefabDocument::new("Layout source");
    document.nodes[0].components.insert(
        "ui.layout".into(),
        kedra_scene::document::ComponentData::ron(1, "(node:())")?,
    );
    std::fs::write(dir.path().join("source.prefab.ron"), document.serialize()?)?;
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
    let target = documents
        .target(document.id, document.root.into())
        .ok_or("Layout target")?;
    for width in [Val::Auto, px(0), px(280)] {
        if width != Val::Auto {
            let registry = app.world().resource::<AppTypeRegistry>().clone();
            let change = documents.prepare_property(
                &target,
                &documents.revisions(),
                "ui.layout",
                &[kedra_authoring::properties::PropertyEdit {
                    path: bevy::reflect::ParsedPath::parse("node.width")?,
                    value: &width,
                }],
                &registry.read(),
                app.world()
                    .resource::<kedra_scene::reflection::ComponentPolicies>(),
            )?;
            documents.publish_change(change);
            panel.refresh_presentation();
            panel.tick(app.world_mut(), id, &mut documents)?;
        }
        for _ in 0..8 {
            app.update();
            panel.refresh_presentation();
            panel.tick(app.world_mut(), id, &mut documents)?;
        }
        let unsettled: Vec<_> = panel
            .inputs
            .iter()
            .filter(|input| TextField::unsettled(app.world(), **input))
            .map(|input| {
                (
                    app.world()
                        .get::<kedra_ui::ElementId>(*input)
                        .map(|id| &id.0),
                    app.world()
                        .get::<bevy::text::EditableText>(*input)
                        .map(|text| text.value().to_owned()),
                    app.world()
                        .get::<TextField>(*input)
                        .map(|field| field.accepted().to_owned()),
                )
            })
            .collect();
        assert!(
            !panel.has_drafts(app.world()),
            "Unsettled Layout inputs after {width:?}: {unsettled:?}"
        );
    }
    let input = app
        .world_mut()
        .query::<(Entity, &kedra_inspector::fields::ReflectedField)>()
        .iter(app.world())
        .find(|(_, field)| field.path.to_string() == ".node.width")
        .map(|(entity, _)| entity)
        .ok_or("Width input")?;
    app.world_mut()
        .get_mut::<bevy::text::EditableText>(input)
        .ok_or("Native input")?
        .editor_mut()
        .set_text("NaN");
    assert!(panel.has_drafts(app.world()));
    TextField::cancel(app.world_mut(), input)
        .map_err(|error| format!("Cancel failed: {error:?}"))?;
    for _ in 0..8 {
        app.update();
        panel.refresh_presentation();
        panel.tick(app.world_mut(), id, &mut documents)?;
    }
    assert!(!panel.has_drafts(app.world()));
    let (body, disclosure) = {
        let world = app.world_mut();
        let disclosure = world
            .query::<(&kedra_ui::ElementId, &Disclosure)>()
            .iter(world)
            .find(|(id, _)| id.0.ends_with("/layout/0/toggle"))
            .map(|(_, action)| action.clone())
            .ok_or("Size disclosure")?;
        let body = world
            .query::<(Entity, &kedra_ui::ElementId)>()
            .iter(world)
            .find(|(_, id)| id.0.ends_with("/layout/0/body"))
            .map(|(entity, _)| entity)
            .ok_or("Size body")?;
        (body, disclosure)
    };
    assert!(panel.owns(&disclosure));
    // This CPU fixture has no authored stylesheet/classes on section slots.
    app.world_mut()
        .entity_mut(body)
        .insert(bevy_flair::style::components::ClassList::empty());
    app.world_mut()
        .get_mut::<bevy::text::EditableText>(input)
        .ok_or("Native width")?
        .queue_edit(bevy::text::TextEdit::TextEnd(false));
    panel.action(app.world_mut(), Action::Disclosure(disclosure.clone()))?;
    assert!(panel.pending_presentation.is_some());
    for _ in 0..8 {
        app.update();
        panel.tick(app.world_mut(), id, &mut documents)?;
    }
    assert!(panel.pending_presentation.is_none());
    let bodies: Vec<_> = app
        .world_mut()
        .query::<(
            Entity,
            &kedra_ui::ElementId,
            Option<&bevy_flair::style::components::ClassList>,
        )>()
        .iter(app.world())
        .filter(|(_, id, _)| id.0.ends_with("/layout/0/body"))
        .map(|(entity, id, classes)| (entity, id.0.clone(), format!("{classes:?}")))
        .collect();
    assert!(
        app.world()
            .get::<bevy_flair::style::components::ClassList>(body)
            .is_some_and(|classes| classes.contains("hidden")),
        "Size body must collapse: {bodies:?}"
    );
    assert!(app.world().get_entity(input).is_ok());
    // Native work completing is not permission to discard a real text draft.
    app.world_mut()
        .get_mut::<bevy::text::EditableText>(input)
        .ok_or("Retained width")?
        .editor_mut()
        .set_text("unaccepted");
    app.world_mut()
        .get_mut::<bevy::text::EditableText>(input)
        .ok_or("Retained width")?
        .queue_edit(bevy::text::TextEdit::TextEnd(false));
    let revisions = documents.revisions();
    panel.action(app.world_mut(), Action::Disclosure(disclosure))?;
    for _ in 0..8 {
        app.update();
        panel.tick(app.world_mut(), id, &mut documents)?;
    }
    assert!(panel.pending_presentation.is_none());
    assert!(panel.has_drafts(app.world()));
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(input)
            .ok_or("Retained draft")?
            .value(),
        "unaccepted"
    );
    assert!(
        app.world()
            .get::<bevy_flair::style::components::ClassList>(body)
            .ok_or("Retained Size classes")?
            .contains("hidden")
    );
    assert_eq!(documents.revisions(), revisions);
    Ok(())
}
