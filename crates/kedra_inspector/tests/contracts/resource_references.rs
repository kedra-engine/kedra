//! Native reference wells preserve logical display and original request identity;
//! search/highlight is presentation only. Hosts retain source/delivery authority.
use crate::support::*;
use bevy::prelude::*;
use kedra_inspector::{
    binding::{BindingToken, ValueBinding, ValueMetadata},
    controls::Control,
    editors::*,
    resources::ReferenceEvent,
    view::TreeView,
};
use kedra_ui::fields::FieldAccess;
#[derive(Reflect)]
struct ImageReference {
    image: Handle<Image>,
}
#[derive(Resource, Default)]
struct Requests(Vec<ReferenceEvent>);
struct Metadata;
impl ValueMetadata for Metadata {
    fn describe(&self, _: Option<&bevy::reflect::ParsedPath>) -> Option<String> {
        None
    }
    fn asset_path(&self, _: &bevy::asset::UntypedHandle) -> Option<String> {
        Some("game://folder/étiquette.png#exact".into())
    }
    fn asset_choices(&self, asset_type: std::any::TypeId) -> Vec<String> {
        if asset_type == std::any::TypeId::of::<Image>() {
            vec!["game://first.png".into(), "game://second.png".into()]
        } else {
            Vec::new()
        }
    }
}
fn binding(value: &dyn PartialReflect, token: u64) -> ValueBinding<'_> {
    ValueBinding {
        value,
        token: BindingToken(token),
        label: "Reference",
        access: &FieldAccess::Editable,
        owner_type: None,
        path: None,
        diagnostic: None,
        metadata: Some(&Metadata),
    }
}
fn element(world: &mut World, suffix: &str) -> TestResult<Entity> {
    world
        .query::<(Entity, &kedra_ui::ElementId)>()
        .iter(world)
        .find(|(_, id)| id.0.ends_with(suffix))
        .map(|(entity, _)| entity)
        .ok_or_else(|| format!("Missing reference endpoint {suffix}").into())
}
#[test]
fn path_well_and_search_never_assign_but_result_click_retains_its_original_native_binding()
-> TestResult {
    let (mut app, parent, templates) = fixture()?;
    app.register_type::<ImageReference>()
        .add_plugins((
            bevy::text::TextPlugin,
            kedra_inspector::controls::InspectorControlsPlugin,
        ))
        .init_resource::<Requests>()
        .add_observer(
            |event: On<ReferenceEvent>, mut requests: ResMut<Requests>| {
                requests.0.push(event.clone())
            },
        );
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let editors = kedra_inspector::builtin::editors()?;
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "reference",
    };
    let source = ImageReference {
        image: Handle::default(),
    };
    let mut tree = TreeView::mount(
        app.world_mut(),
        parent,
        &mut context,
        binding(&source, 1),
        false,
    )?;
    for _ in 0..3 {
        app.update();
        *context.budget = RenderBudget::default();
        tree.update(app.world_mut(), &mut context, binding(&source, 1), false)?;
    }
    let path = element(app.world_mut(), "/path/value")?;
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(path)
            .ok_or("path")?
            .value(),
        "game://folder/étiquette.png#exact"
    );
    assert!(
        app.world()
            .get::<FieldAccess>(path)
            .is_some_and(FieldAccess::is_read_only)
    );
    let picker = element(app.world_mut(), "/picker")?;
    let original = app
        .world()
        .get::<Control>(picker)
        .ok_or("binding")?
        .0
        .clone();
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: picker });
    app.world_mut().flush();
    app.update();
    let search = element(app.world_mut(), "/search/value")?;
    app.world_mut()
        .get_mut::<bevy::text::EditableText>(search)
        .ok_or("search")?
        .editor_mut()
        .set_text("second");
    app.update();
    assert!(app.world().resource::<Requests>().0.is_empty());
    assert_eq!(source.image, Handle::default());
    let result = element(app.world_mut(), "/menu/0")?;
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: result });
    app.world_mut().flush();
    let request = app
        .world_mut()
        .resource_mut::<Requests>()
        .0
        .pop()
        .ok_or("request")?;
    assert_eq!(request.path.as_deref(), Some("game://second.png"));
    assert_eq!(request.binding.path.to_string(), ".image");
    assert!(std::sync::Arc::ptr_eq(&request.binding, &original));
    assert_eq!(source.image, Handle::default());
    // Rebinding retires an open popup rather than allowing its old choice to
    // borrow a refreshed source generation on the same retained UI entity.
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: picker });
    app.world_mut().flush();
    app.update();
    let old_result = element(app.world_mut(), "/menu/0")?;
    *context.budget = RenderBudget::default();
    tree.update(app.world_mut(), &mut context, binding(&source, 2), false)?;
    app.update();
    assert!(app.world().get_entity(old_result).is_err());
    assert!(app.world().resource::<Requests>().0.is_empty());
    assert_eq!(source.image, Handle::default());
    Ok(())
}

#[test]
fn search_then_enter_uses_the_ordered_native_query_and_an_empty_result_assigns_nothing()
-> TestResult {
    use bevy::{
        input::keyboard::{Key, KeyCode, KeyboardInput},
        input_focus::FocusedInput,
    };
    let (mut app, parent, templates) = fixture()?;
    app.add_plugins((
        bevy::text::TextPlugin,
        bevy::ui_widgets::TextInputPlugin,
        kedra_ui::text_input::TextSessionPlugin,
        kedra_inspector::controls::InspectorControlsPlugin,
    ))
    .init_resource::<bevy::clipboard::Clipboard>()
    .init_resource::<bevy::input_focus::InputFocus>()
    .init_resource::<bevy::input::ButtonInput<Key>>()
    .init_resource::<bevy::picking::events::PointerState>()
    .init_resource::<UiScale>()
    .add_message::<bevy::window::Ime>()
    .add_message::<bevy::picking::events::PointerRelease>()
    .init_resource::<Requests>()
    .add_observer(
        |event: On<ReferenceEvent>, mut requests: ResMut<Requests>| requests.0.push(event.clone()),
    );
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let editors = kedra_inspector::builtin::editors()?;
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "ordered-reference",
    };
    let source = Handle::<Image>::default();
    let _tree = TreeView::mount(
        app.world_mut(),
        parent,
        &mut context,
        binding(&source, 1),
        false,
    )?;
    let picker = element(app.world_mut(), "/picker")?;
    for (query, expected) in [("missing", None), ("second", Some("game://second.png"))] {
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity: picker });
        app.world_mut().flush();
        app.update();
        app.update();
        let search = element(app.world_mut(), "/search/value")?;
        app.world_mut()
            .get_mut::<bevy::text::EditableText>(search)
            .ok_or("search")?
            .queue_edit(bevy::text::TextEdit::Insert(query.into()));
        app.world_mut().trigger(FocusedInput::new(
            search,
            KeyboardInput {
                key_code: KeyCode::Enter,
                logical_key: Key::Enter,
                state: bevy::input::ButtonState::Pressed,
                text: None,
                repeat: false,
                window: parent,
            },
            parent,
        ));
        app.world_mut().flush();
        app.update();
        app.world_mut().flush();
        assert_eq!(
            app.world_mut()
                .resource_mut::<Requests>()
                .0
                .pop()
                .and_then(|request| request.path)
                .as_deref(),
            expected
        );
        if expected.is_none() {
            assert_eq!(source, Handle::default());
            app.world_mut()
                .trigger(bevy::ui_widgets::Activate { entity: picker });
            app.world_mut().flush();
        }
    }
    Ok(())
}

#[test]
fn empty_required_image_add_requests_a_typed_resource_without_inventing_an_item_or_clear()
-> TestResult {
    use kedra_ui::authored::components::UiImageDensity;
    let (mut app, parent, templates) = fixture()?;
    app.add_plugins((
        bevy::text::TextPlugin,
        kedra_inspector::controls::InspectorControlsPlugin,
    ))
    .init_resource::<Requests>()
    .add_observer(
        |event: On<ReferenceEvent>, mut requests: ResMut<Requests>| requests.0.push(event.clone()),
    );
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let editors = kedra_inspector::builtin::editors()?;
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "empty-image",
    };
    let source = Vec::<UiImageDensity>::new();
    let mut tree = TreeView::mount(
        app.world_mut(),
        parent,
        &mut context,
        binding(&source, 1),
        false,
    )?;
    for _ in 0..3 {
        app.update();
        *context.budget = RenderBudget::default();
        tree.update(app.world_mut(), &mut context, binding(&source, 1), false)?;
    }
    let add = element(app.world_mut(), "/add")?;
    assert!(
        !app.world()
            .entity(add)
            .contains::<bevy::ui::InteractionDisabled>()
    );
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: add });
    app.world_mut().flush();
    app.update();
    assert!(source.is_empty());
    assert!(!kedra_inspector::collections::unsettled(
        app.world(),
        tree.root(),
        None
    ));
    assert!(
        !app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|text| text.0 == "Clear")
    );
    let first = element(app.world_mut(), "/menu/0")?;
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: first });
    app.world_mut().flush();
    let request = app
        .world_mut()
        .resource_mut::<Requests>()
        .0
        .pop()
        .ok_or("image resource request")?;
    assert_eq!(request.entity, add);
    assert_eq!(request.path.as_deref(), Some("game://first.png"));
    assert_eq!(request.binding.token, BindingToken(1));
    assert!(request.binding.path.0.is_empty());
    assert!(
        source.is_empty(),
        "choosing never creates an unvalidated native handle/item"
    );
    Ok(())
}
