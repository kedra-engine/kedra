//! Native math drawers own composition/retention/fallback, not source mutation.
use crate::support::*;
use bevy::{prelude::*, reflect::TypeRegistry};
use kedra_inspector::{
    binding::{BindingToken, ValueBinding, ValueMetadata},
    builtin,
    editors::{EditorContext, RenderBudget},
    view::TreeView,
};
use kedra_ui::fields::FieldAccess;

fn inputs(world: &mut World) -> Vec<(Entity, String)> {
    world
        .query::<(Entity, &bevy::text::EditableText)>()
        .iter(world)
        .map(|(entity, text)| (entity, text.value().to_string()))
        .collect()
}

fn transform_binding<'a>(value: &'a Transform, access: &'a FieldAccess) -> ValueBinding<'a> {
    ValueBinding {
        token: BindingToken(1),
        value,
        label: "scene.transform",
        access,
        owner_type: Some(std::any::TypeId::of::<Transform>()),
        path: None,
        diagnostic: None,
        metadata: Some(&Provenance),
    }
}

struct Provenance;
impl ValueMetadata for Provenance {
    fn describe(&self, path: Option<&bevy::reflect::ParsedPath>) -> Option<String> {
        Some(format!(
            "origin:{}",
            path.map(ToString::to_string).unwrap_or_default()
        ))
    }
}

#[test]
fn transform_uses_three_inline_vectors_and_retains_controls_through_refresh_and_search()
-> TestResult {
    let (mut app, parent, templates) = fixture()?;
    let mut types = TypeRegistry::default();
    types.register::<Transform>();
    let editors = builtin::editors()?;
    let access = FieldAccess::ReadOnly {
        reason: "Source editing not enabled".into(),
    };
    let mut source = Transform::from_xyz(1., 2., 3.).with_scale(Vec3::new(4., 5., 6.));
    let original = source;
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "transform",
    };
    let mut tree = TreeView::mount(
        app.world_mut(),
        parent,
        &mut context,
        transform_binding(&source, &access),
        false,
    )?;
    let initial = inputs(app.world_mut());
    assert_eq!(initial.len(), 9);
    for (suffix, origin) in [
        ("/translation/x/value", "origin:.translation.x"),
        ("/rotation/x/value", "origin:.rotation"),
    ] {
        let input = app
            .world_mut()
            .query::<(Entity, &kedra_ui::ElementId)>()
            .iter(app.world())
            .find(|(_, id)| id.0.ends_with(suffix))
            .map(|(entity, _)| entity)
            .ok_or("axis")?;
        let tooltip = &app
            .world()
            .get::<kedra_ui::tooltip::Tooltip>(input)
            .ok_or("tooltip")?
            .0;
        assert!(tooltip.contains(origin));
        if suffix.starts_with("/rotation") {
            assert!(!tooltip.contains("origin:.rotation.x"));
        }
    }
    for label in ["Transform", "Position", "Rotation", "Scale"] {
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0 == label),
            "missing {label}"
        );
    }
    assert_eq!(source, original);
    source.translation.x = 12.5;
    let before_rotation = source.rotation.to_array();
    tree.filter("Position");
    let mut budget = RenderBudget::default();
    context.budget = &mut budget;
    tree.update(
        app.world_mut(),
        &mut context,
        transform_binding(&source, &access),
        false,
    )?;
    assert!(
        tree.has_matches(),
        "search must recognize displayed component labels"
    );
    let refreshed = inputs(app.world_mut());
    assert_eq!(
        refreshed.iter().map(|(e, _)| e).collect::<Vec<_>>(),
        initial.iter().map(|(e, _)| e).collect::<Vec<_>>()
    );
    assert!(refreshed.iter().any(|(_, value)| value == "12.5"));
    assert_eq!(source.rotation.to_array(), before_rotation);
    tree.filter("");
    let mut budget = RenderBudget::default();
    context.budget = &mut budget;
    tree.update(
        app.world_mut(),
        &mut context,
        transform_binding(&source, &access),
        true,
    )?;
    assert!(
        !app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|text| text.0 == "Position")
    );
    assert!(
        inputs(app.world_mut())
            .iter()
            .any(|(_, value)| value == "12.5")
    );
    assert_eq!(source.rotation.to_array(), before_rotation);
    tree.retire(app.world_mut());
    assert!(inputs(app.world_mut()).is_empty());
    Ok(())
}

#[derive(Reflect)]
struct Object {
    transform: Transform,
}

#[test]
fn base_section_collapses_nested_custom_blocks_and_shares_state_with_default_groups() -> TestResult
{
    use kedra_inspector::view::Disclosure;
    let (mut app, parent, templates) = fixture()?;
    let mut types = TypeRegistry::default();
    types.register::<Object>();
    let editors = builtin::editors()?;
    let source = Object {
        transform: Transform::from_xyz(1., 2., 3.),
    };
    let before = source.transform;
    let access = FieldAccess::ReadOnly {
        reason: "Immutable fixture".into(),
    };
    let binding = ValueBinding {
        token: BindingToken(3),
        value: &source,
        label: "Object",
        access: &access,
        owner_type: Some(std::any::TypeId::of::<Object>()),
        path: None,
        diagnostic: None,
        metadata: None,
    };
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "nested",
    };
    let mut tree = TreeView::mount(app.world_mut(), parent, &mut context, binding, false)?;
    let before_inputs = inputs(app.world_mut());
    let action = app
        .world_mut()
        .query::<(&kedra_ui::ElementId, &Disclosure)>()
        .iter(app.world())
        .find(|(id, _)| id.0.ends_with("/chrome/toggle"))
        .map(|(_, action)| action.clone())
        .ok_or("base block disclosure")?;
    assert_eq!(action.owner, tree.root());
    assert!(tree.disclose(&action));
    let mut budget = RenderBudget::default();
    context.budget = &mut budget;
    tree.update(app.world_mut(), &mut context, binding, false)?;
    let body = app
        .world_mut()
        .query::<(Entity, &kedra_ui::ElementId)>()
        .iter(app.world())
        .find(|(_, id)| id.0.ends_with("/chrome/body"))
        .map(|(entity, _)| entity)
        .ok_or("block body")?;
    assert!(
        app.world()
            .get::<bevy_flair::style::components::ClassList>(body)
            .ok_or("body classes")?
            .contains("hidden")
    );
    assert_eq!(
        inputs(app.world_mut()),
        before_inputs,
        "collapse must not destroy or refresh away native controls"
    );
    let mut stale = action.clone();
    stale.token = BindingToken(2);
    assert!(!tree.disclose(&stale));
    let mut budget = RenderBudget::default();
    context.budget = &mut budget;
    tree.update(app.world_mut(), &mut context, binding, true)?;
    let default_body = app
        .world_mut()
        .query::<(Entity, &kedra_ui::ElementId)>()
        .iter(app.world())
        .find(|(_, id)| id.0 == "nested/[0]/body")
        .map(|(entity, _)| entity)
        .ok_or("default body")?;
    assert!(
        app.world()
            .get::<bevy_flair::style::components::ClassList>(default_body)
            .ok_or("default classes")?
            .contains("hidden")
    );
    assert!(tree.disclose(&action));
    let mut budget = RenderBudget::default();
    context.budget = &mut budget;
    tree.update(app.world_mut(), &mut context, binding, false)?;
    let body = app
        .world_mut()
        .query::<(Entity, &kedra_ui::ElementId)>()
        .iter(app.world())
        .find(|(_, id)| id.0.ends_with("/chrome/body"))
        .map(|(entity, _)| entity)
        .ok_or("restored block body")?;
    assert!(
        !app.world()
            .get::<bevy_flair::style::components::ClassList>(body)
            .ok_or("body classes")?
            .contains("hidden")
    );
    assert_eq!(source.transform, before);
    tree.retire(app.world_mut());
    Ok(())
}

#[test]
fn invalid_quaternion_is_visible_and_default_view_remains_reachable_without_normalizing_source()
-> TestResult {
    let (mut app, parent, templates) = fixture()?;
    let mut types = TypeRegistry::default();
    types.register::<Quat>();
    let editors = builtin::editors()?;
    let source = Quat::from_xyzw(0., 0., 0., 0.);
    let before = source.to_array();
    let access = FieldAccess::ReadOnly {
        reason: "Immutable fixture".into(),
    };
    let binding = ValueBinding {
        token: BindingToken(1),
        value: &source,
        label: "Rotation",
        access: &access,
        owner_type: Some(std::any::TypeId::of::<Quat>()),
        path: None,
        diagnostic: None,
        metadata: None,
    };
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "quaternion",
    };
    let mut tree = TreeView::mount(app.world_mut(), parent, &mut context, binding, false)?;
    assert!(
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|text| text.0.contains("unit quaternion"))
    );
    let mut budget = RenderBudget::default();
    context.budget = &mut budget;
    tree.update(app.world_mut(), &mut context, binding, true)?;
    assert!(
        !app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|text| text.0.contains("unit quaternion"))
    );
    assert_eq!(source.to_array(), before);
    tree.retire(app.world_mut());
    assert!(inputs(app.world_mut()).is_empty());
    Ok(())
}
