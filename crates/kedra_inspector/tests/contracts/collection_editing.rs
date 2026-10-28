//! Collection presentation owns full native item paths and atomic structural
//! proposals/retained identities. Consumers own validation, history and publication.
use crate::support::*;
use bevy::prelude::*;
use kedra_inspector::{
    binding::{BindingToken, ValueBinding},
    collections,
    controls::{Control, ControlEvent},
    editors::*,
    fields::ReflectedField,
    view::TreeView,
};
use kedra_ui::{
    authored::components::UiClasses,
    fields::{FieldAccess, TextField},
};

#[derive(Resource, Default)]
struct Proposals(Vec<ControlEvent>);
fn binding(value: &dyn PartialReflect) -> ValueBinding<'_> {
    ValueBinding {
        value,
        label: "ui.classes",
        token: BindingToken(1),
        path: None,
        owner_type: None,
        access: &FieldAccess::Editable,
        diagnostic: None,
        metadata: None,
    }
}
fn element(world: &mut World, suffix: &str) -> TestResult<Entity> {
    world
        .query::<(Entity, &kedra_ui::ElementId)>()
        .iter(world)
        .find(|(_, id)| id.0.ends_with(suffix))
        .map(|(entity, _)| entity)
        .ok_or_else(|| format!("Missing collection endpoint {suffix}").into())
}
fn activate(app: &mut App, suffix: &str) -> TestResult {
    let entity = element(app.world_mut(), suffix)?;
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity });
    app.world_mut().flush();
    app.update();
    Ok(())
}
fn setup() -> TestResult<(App, Entity, kedra_ui::fields::FieldTemplates)> {
    let (mut app, parent, templates) = fixture()?;
    app.add_plugins((
        kedra_inspector::controls::InspectorControlsPlugin,
        bevy::text::TextPlugin,
    ))
    .init_resource::<Proposals>()
    .add_observer(
        |event: On<ControlEvent>, mut proposals: ResMut<Proposals>| proposals.0.push(event.clone()),
    );
    Ok((app, parent, templates))
}
fn refresh(
    app: &mut App,
    tree: &mut TreeView,
    context: &mut EditorContext<'_>,
    source: &dyn PartialReflect,
) -> TestResult {
    for _ in 0..3 {
        app.update();
        *context.budget = RenderBudget::default();
        tree.update(app.world_mut(), context, binding(source), false)?;
    }
    Ok(())
}
fn proposal(app: &mut App) -> TestResult<ControlEvent> {
    app.world_mut()
        .resource_mut::<Proposals>()
        .0
        .pop()
        .ok_or("Missing complete proposal".into())
}

#[test]
fn explicit_attachments_restore_distinct_item_mappings_for_equal_values() -> TestResult {
    let (mut app, parent, templates) = setup()?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let editors = kedra_inspector::builtin::editors()?;
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "classes",
    };
    let mut source = UiClasses {
        names: vec!["same".into()],
    };
    let mut tree = TreeView::mount(
        app.world_mut(),
        parent,
        &mut context,
        binding(&source),
        false,
    )?;
    refresh(&mut app, &mut tree, &mut context, &source)?;
    let first = collections::presentation::capture(app.world(), tree.root(), false);
    element(app.world_mut(), "/items/1/[]/value")?;
    source.names.clear();
    refresh(&mut app, &mut tree, &mut context, &source)?;
    source.names.push("same".into());
    refresh(&mut app, &mut tree, &mut context, &source)?;
    element(app.world_mut(), "/items/2/[]/value")?;
    let second = collections::presentation::capture(app.world(), tree.root(), false);
    for (snapshot, expected, absent) in [
        (&first, "/items/1/[]/value", "/items/2/[]/value"),
        (&second, "/items/2/[]/value", "/items/1/[]/value"),
    ] {
        collections::presentation::restore(app.world_mut(), tree.root(), snapshot, false);
        refresh(&mut app, &mut tree, &mut context, &source)?;
        element(app.world_mut(), expected)?;
        assert!(element(app.world_mut(), absent).is_err());
        assert_eq!(source.names, ["same"]);
    }
    // A mismatched attachment is ignored, not permission to resurrect a source item.
    source.names.clear();
    collections::presentation::restore(app.world_mut(), tree.root(), &first, false);
    refresh(&mut app, &mut tree, &mut context, &source)?;
    assert!(element(app.world_mut(), "/items/1/[]/value").is_err());
    Ok(())
}
#[test]
fn history_attachments_restore_focus_only_to_the_collection_that_owned_it() -> TestResult {
    use bevy::input_focus::{FocusCause, InputFocus};
    let (mut app, parent, templates) = setup()?;
    app.init_resource::<InputFocus>();
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let editors = kedra_inspector::builtin::editors()?;
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "focused-classes",
    };
    let source = UiClasses {
        names: vec!["selected".into()],
    };
    let other_source = UiClasses { names: vec![] };
    let mut first = TreeView::mount(
        app.world_mut(),
        parent,
        &mut context,
        binding(&source),
        false,
    )?;
    refresh(&mut app, &mut first, &mut context, &source)?;
    activate(&mut app, "focused-classes/[]/items/1/grip")?;
    let input = element(app.world_mut(), "focused-classes/[]/items/1/[]/value")?;
    let session = kedra_ui::text_input::TextSession::attach(
        app.world()
            .get::<bevy::text::EditableText>(input)
            .ok_or("native input")?,
        4096,
    )
    .map_err(|error| format!("Text session: {error:?}"))?;
    app.world_mut().entity_mut(input).insert(session);
    TextField::attach(app.world_mut(), input, 1)
        .map_err(|error| format!("Text field: {error:?}"))?;
    context.scope = "other-classes";
    let mut other = TreeView::mount(
        app.world_mut(),
        parent,
        &mut context,
        binding(&other_source),
        false,
    )?;
    refresh(&mut app, &mut other, &mut context, &other_source)?;
    app.world_mut()
        .entity_mut(other.root())
        .insert(Visibility::Hidden);
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(input, FocusCause::Navigated);
    let before = collections::presentation::capture(app.world(), first.root(), false);
    let foreign = collections::presentation::capture(app.world(), other.root(), false);
    app.world_mut().resource_mut::<InputFocus>().clear();
    collections::presentation::restore(app.world_mut(), first.root(), &before, true);
    collections::presentation::restore(app.world_mut(), other.root(), &foreign, true);
    context.scope = "focused-classes";
    *context.budget = RenderBudget::default();
    first.update(app.world_mut(), &mut context, binding(&source), false)?;
    context.scope = "other-classes";
    *context.budget = RenderBudget::default();
    other.update(app.world_mut(), &mut context, binding(&other_source), false)?;
    // The source adapter independently grants editing after reflected refresh.
    app.world_mut()
        .entity_mut(input)
        .insert(FieldAccess::Editable);
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(input));
    assert_eq!(source.names, ["selected"]);
    assert!(other_source.names.is_empty());
    assert!(app.world().entities().contains(other.root()));
    // A hidden/non-owning view attachment cannot acquire focus when none was captured.
    app.world_mut().resource_mut::<InputFocus>().clear();
    let unfocused = collections::presentation::capture(app.world(), first.root(), false);
    collections::presentation::restore(app.world_mut(), first.root(), &unfocused, true);
    context.scope = "focused-classes";
    refresh(&mut app, &mut first, &mut context, &source)?;
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
    Ok(())
}

#[test]
fn item_fields_use_full_paths_and_structural_actions_propose_without_optimistic_mutation()
-> TestResult {
    let (mut app, parent, templates) = setup()?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let editors = kedra_inspector::builtin::editors()?;
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "classes",
    };
    let mut source = UiClasses {
        names: vec!["first".into(), "второй".into()],
    };
    let mut tree = TreeView::mount(
        app.world_mut(),
        parent,
        &mut context,
        binding(&source),
        false,
    )?;
    refresh(&mut app, &mut tree, &mut context, &source)?;
    let input = element(app.world_mut(), "/items/2/[]/value")?;
    assert_eq!(
        app.world()
            .get::<ReflectedField>(input)
            .ok_or("field")?
            .path
            .to_string(),
        ".names[1]"
    );
    assert!(
        app.world().get::<TextField>(input).is_none(),
        "the consumer, not the array, owns native acknowledgment"
    );
    assert!(!app.world().iter_entities().any(|entity| {
        entity
            .get::<kedra_ui::ElementId>()
            .is_some_and(|id| id.0.ends_with("/apply") || id.0.ends_with("/cancel"))
    }));
    activate(&mut app, "/items/1/grip")?;
    refresh(&mut app, &mut tree, &mut context, &source)?;
    activate(&mut app, "/remove")?;
    let event = proposal(&mut app)?;
    assert_eq!(event.binding.path.to_string(), ".names");
    assert_eq!(
        Vec::<String>::from_reflect(event.value.as_ref()).ok_or("native list")?,
        ["второй"]
    );
    assert_eq!(source.names, ["first", "второй"]);
    assert!(collections::unsettled(app.world(), tree.root(), None));
    collections::respond(app.world_mut(), &event, false);
    refresh(&mut app, &mut tree, &mut context, &source)?;
    assert!(!collections::unsettled(app.world(), tree.root(), None));
    assert_eq!(element(app.world_mut(), "/items/2/[]/value")?, input);
    // A fresh, accepted removal keeps the surviving identity but changes its path.
    activate(&mut app, "/remove")?;
    let event = proposal(&mut app)?;
    let before = collections::presentation::capture(app.world(), tree.root(), false);
    source.names = Vec::<String>::from_reflect(event.value.as_ref()).ok_or("native list")?;
    collections::respond(app.world_mut(), &event, true);
    refresh(&mut app, &mut tree, &mut context, &source)?;
    assert_eq!(element(app.world_mut(), "/items/2/[]/value")?, input);
    assert_eq!(
        app.world()
            .get::<ReflectedField>(input)
            .ok_or("field")?
            .path
            .to_string(),
        ".names[0]"
    );
    let root = event.entity;
    let current = app.world().get::<Control>(root).ok_or("binding")?.0.clone();
    assert!(!std::sync::Arc::ptr_eq(&event.binding, &current));
    collections::respond(app.world_mut(), &event, true);
    assert_eq!(source.names, ["второй"]);
    // Host-owned Undo restores the exact prior list. The array recovers the
    // removed selection, preserves its surviving sibling and rebinds its index.
    source.names = vec!["first".into(), "второй".into()];
    collections::presentation::restore(app.world_mut(), tree.root(), &before, true);
    refresh(&mut app, &mut tree, &mut context, &source)?;
    assert_eq!(element(app.world_mut(), "/items/2/[]/value")?, input);
    assert_eq!(
        app.world()
            .get::<ReflectedField>(input)
            .ok_or("restored field")?
            .path
            .to_string(),
        ".names[1]"
    );
    activate(&mut app, "/remove")?;
    let restored = proposal(&mut app)?;
    assert_eq!(
        Vec::<String>::from_reflect(restored.value.as_ref()).ok_or("restored selection")?,
        ["второй"]
    );
    collections::respond(app.world_mut(), &restored, false);
    // An unrelated external list must not inherit the old selected identity.
    source.names = vec!["foreign".into()];
    refresh(&mut app, &mut tree, &mut context, &source)?;
    let remove = element(app.world_mut(), "/remove")?;
    assert!(
        app.world()
            .entity(remove)
            .contains::<bevy::ui::InteractionDisabled>()
    );
    Ok(())
}

#[test]
fn add_uses_a_valid_unique_class_default_and_native_text_blocks_structural_actions_even_hidden()
-> TestResult {
    let (mut app, parent, templates) = setup()?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let editors = kedra_inspector::builtin::editors()?;
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "classes",
    };
    let mut source = UiClasses {
        names: vec!["class".into(), "class-2".into()],
    };
    let mut tree = TreeView::mount(
        app.world_mut(),
        parent,
        &mut context,
        binding(&source),
        false,
    )?;
    refresh(&mut app, &mut tree, &mut context, &source)?;
    activate(&mut app, "/add")?;
    let event = proposal(&mut app)?;
    let proposed = Vec::<String>::from_reflect(event.value.as_ref()).ok_or("native list")?;
    assert_eq!(proposed, ["class", "class-2", "class-3"]);
    source.names = proposed;
    collections::respond(app.world_mut(), &event, true);
    refresh(&mut app, &mut tree, &mut context, &source)?;
    let input = element(app.world_mut(), "/items/3/[]/value")?;
    let native = app
        .world()
        .get::<bevy::text::EditableText>(input)
        .ok_or("text")?;
    let session =
        kedra_ui::text_input::TextSession::attach(native, 4096).map_err(|e| format!("{e:?}"))?;
    app.world_mut().entity_mut(input).insert(session);
    TextField::attach(app.world_mut(), input, 1).map_err(|e| format!("{e:?}"))?;
    app.world_mut()
        .get_mut::<bevy::text::EditableText>(input)
        .ok_or("text")?
        .editor_mut()
        .set_text("invalid class");
    app.world_mut()
        .entity_mut(tree.root())
        .insert(Visibility::Hidden);
    assert!(collections::unsettled(app.world(), tree.root(), None));
    app.world_mut()
        .entity_mut(tree.root())
        .insert(Visibility::Inherited);
    activate(&mut app, "/add")?;
    assert!(app.world().resource::<Proposals>().0.is_empty());
    assert_eq!(source.names, ["class", "class-2", "class-3"]);
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(input)
            .ok_or("text")?
            .value(),
        "invalid class"
    );
    Ok(())
}

#[test]
fn source_refresh_revokes_queued_structural_actions_and_preserves_native_shadow_construction()
-> TestResult {
    let (mut app, parent, templates) = setup()?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let editors = kedra_inspector::builtin::editors()?;
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "shadows",
    };
    let source = BoxShadow::default();
    let mut tree = TreeView::mount(
        app.world_mut(),
        parent,
        &mut context,
        binding(&source),
        false,
    )?;
    refresh(&mut app, &mut tree, &mut context, &source)?;
    activate(&mut app, "/add")?;
    let event = proposal(&mut app)?;
    let proposed =
        Vec::<ShadowStyle>::from_reflect(event.value.as_ref()).ok_or("native shadows")?;
    assert_eq!(proposed, [ShadowStyle::default()]);
    collections::respond(app.world_mut(), &event, true);
    let source = BoxShadow(proposed);
    refresh(&mut app, &mut tree, &mut context, &source)?;
    let mut ids = std::collections::BTreeSet::new();
    for id in app
        .world_mut()
        .query::<&kedra_ui::ElementId>()
        .iter(app.world())
    {
        if id.0.contains("/items/") {
            assert!(
                ids.insert(id.0.clone()),
                "Shadow header collides with retained native disclosure: {}",
                id.0
            );
        }
    }
    activate(&mut app, "/items/1/header")?;
    refresh(&mut app, &mut tree, &mut context, &source)?;
    assert!(
        app.world()
            .get::<Text>(event.binding.diagnostic)
            .ok_or("collection diagnostic")?
            .0
            .is_empty(),
        "unchanged source must not stale its disclosure action"
    );
    activate(&mut app, "/items/1/header")?;
    refresh(&mut app, &mut tree, &mut context, &source)?;
    let paths = app
        .world_mut()
        .query::<&ReflectedField>()
        .iter(app.world())
        .map(|field| field.path.to_string())
        .collect::<Vec<_>>();
    assert!(
        paths.iter().any(|path| path.contains("[0].blur_radius")),
        "native Val drawer must survive reconstruction: {paths:?}"
    );
    let add = element(app.world_mut(), "/add")?;
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: add });
    app.world_mut().flush();
    let root = event.entity;
    let old = app.world().get::<Control>(root).ok_or("binding")?.0.clone();
    app.world_mut()
        .entity_mut(root)
        .insert(Control(std::sync::Arc::new(
            kedra_inspector::controls::ControlBinding {
                token: old.token,
                path: old.path.clone(),
                diagnostic: old.diagnostic,
            },
        )));
    app.update();
    assert!(app.world().resource::<Proposals>().0.is_empty());
    assert_eq!(source.0, [ShadowStyle::default()]);
    Ok(())
}

// Synthetic boxes exercise the array's drop/ownership policy, not native layout.
#[test]
fn drag_hover_outside_drop_and_escape_do_not_publish_but_completed_drop_retains_the_item()
-> TestResult {
    use bevy::{
        camera::{NormalizedRenderTarget, RenderTarget},
        input::keyboard::{Key, KeyboardInput},
        input_focus::FocusedInput,
        picking::{
            backend::HitData,
            events::{Pointer, PointerDrag, PointerDragEnd, PointerDragStart},
            pointer::{Location, PointerButton, PointerId},
        },
        window::WindowRef,
    };
    let (mut app, parent, templates) = setup()?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let editors = kedra_inspector::builtin::editors()?;
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "classes",
    };
    let mut source = UiClasses {
        names: vec!["first".into(), "second".into()],
    };
    let mut tree = TreeView::mount(
        app.world_mut(),
        parent,
        &mut context,
        binding(&source),
        false,
    )?;
    refresh(&mut app, &mut tree, &mut context, &source)?;
    let window = app.world_mut().spawn(Window::default()).id();
    let camera = app
        .world_mut()
        .spawn((
            Camera::default(),
            RenderTarget::Window(WindowRef::Entity(window)),
        ))
        .id();
    app.world_mut()
        .entity_mut(parent)
        .insert(UiTargetCamera(camera));
    let first = element(app.world_mut(), "/items/1/grip")?;
    let second = element(app.world_mut(), "/items/2/grip")?;
    let input = element(app.world_mut(), "/items/1/[]/value")?;
    let row1 = app
        .world()
        .get::<ChildOf>(first)
        .ok_or("first row")?
        .parent();
    let row2 = app
        .world()
        .get::<ChildOf>(second)
        .ok_or("second row")?
        .parent();
    let area = app.world().get::<ChildOf>(row1).ok_or("area")?.parent();
    for (entity, center, size) in [
        (area, Vec2::new(100., 50.), Vec2::new(200., 100.)),
        (row1, Vec2::new(100., 20.), Vec2::new(200., 22.)),
        (row2, Vec2::new(100., 60.), Vec2::new(200., 22.)),
    ] {
        app.world_mut().entity_mut(entity).insert((
            ComputedNode {
                size,
                inverse_scale_factor: 1.,
                ..default()
            },
            UiGlobalTransform::from(bevy::math::Affine2::from_translation(center)),
        ));
    }
    let target =
        NormalizedRenderTarget::Window(WindowRef::Entity(window).normalize(None).ok_or("window")?);
    let pointer = |position| {
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: target.clone(),
                position,
            },
        )
    };
    let start = || PointerDragStart {
        entity: first,
        pointer: pointer(Vec2::new(10., 20.)),
        button: PointerButton::Primary,
        hit: HitData::new(camera, 0., None, None),
    };
    let finish = |position| PointerDragEnd {
        entity: first,
        pointer: pointer(position),
        button: PointerButton::Primary,
        distance: position - Vec2::new(10., 20.),
    };
    app.world_mut().trigger(start());
    app.world_mut().flush();
    app.world_mut().trigger(PointerDrag {
        entity: first,
        pointer: pointer(Vec2::new(10., 80.)),
        button: PointerButton::Primary,
        distance: Vec2::new(0., 60.),
        delta: Vec2::new(0., 60.),
    });
    app.world_mut().flush();
    assert!(app.world().resource::<Proposals>().0.is_empty());
    assert_eq!(source.names, ["first", "second"]);
    app.world_mut().trigger(finish(Vec2::new(220., 80.)));
    app.world_mut().flush();
    app.update();
    assert!(app.world().resource::<Proposals>().0.is_empty());
    app.insert_resource(ButtonInput::<Key>::default());
    app.world_mut().trigger(start());
    app.world_mut().flush();
    app.world_mut().trigger(FocusedInput::new(
        first,
        KeyboardInput {
            key_code: KeyCode::Escape,
            logical_key: Key::Escape,
            state: bevy::input::ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        },
        window,
    ));
    app.world_mut().flush();
    app.world_mut().trigger(finish(Vec2::new(10., 80.)));
    app.world_mut().flush();
    app.update();
    assert!(app.world().resource::<Proposals>().0.is_empty());
    assert!(!collections::unsettled(app.world(), tree.root(), None));
    // Revocation must retire the gesture even if no DragEnd reaches its former
    // window. Restoring access/ownership cannot revive that captured authority.
    for transfer in [false, true] {
        app.world_mut().trigger(start());
        app.world_mut().flush();
        assert!(collections::unsettled(app.world(), tree.root(), None));
        if transfer {
            let other = app.world_mut().spawn(Window::default()).id();
            app.world_mut()
                .entity_mut(camera)
                .insert(RenderTarget::Window(WindowRef::Entity(other)));
        } else {
            app.world_mut()
                .entity_mut(first)
                .insert(FieldAccess::ReadOnly {
                    reason: "Access revoked".into(),
                });
        }
        app.update();
        assert!(!collections::unsettled(app.world(), tree.root(), None));
        app.world_mut()
            .entity_mut(first)
            .insert(FieldAccess::Editable);
        app.world_mut()
            .entity_mut(camera)
            .insert(RenderTarget::Window(WindowRef::Entity(window)));
        app.world_mut().trigger(finish(Vec2::new(10., 80.)));
        app.world_mut().flush();
        app.update();
        assert!(app.world().resource::<Proposals>().0.is_empty());
        assert_eq!(source.names, ["first", "second"]);
    }
    app.world_mut().trigger(start());
    app.world_mut().flush();
    app.world_mut().trigger(finish(Vec2::new(10., 80.)));
    app.world_mut().flush();
    app.update();
    let event = proposal(&mut app)?;
    assert_eq!(
        Vec::<String>::from_reflect(event.value.as_ref()).ok_or("list")?,
        ["second", "first"]
    );
    assert_eq!(source.names, ["first", "second"]);
    source.names = Vec::<String>::from_reflect(event.value.as_ref()).ok_or("list")?;
    collections::respond(app.world_mut(), &event, true);
    refresh(&mut app, &mut tree, &mut context, &source)?;
    assert_eq!(element(app.world_mut(), "/items/1/[]/value")?, input);
    assert_eq!(
        app.world()
            .get::<ReflectedField>(input)
            .ok_or("field")?
            .path
            .to_string(),
        ".names[1]"
    );
    Ok(())
}
