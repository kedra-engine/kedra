//! Native Layout grouping preserves source bindings and nested section ownership.
use crate::support::*;
use bevy::{prelude::*, reflect::ParsedPath};
use kedra_inspector::{
    binding::{BindingToken, ValueBinding},
    editors::*,
    fields::ReflectedField,
    view::{Disclosure, TreeView},
};
use kedra_ui::{authored::components::UiLayout, fields::FieldAccess};

fn binding(layout: &UiLayout) -> ValueBinding<'_> {
    ValueBinding {
        value: layout,
        label: "ui.layout",
        path: None,
        token: BindingToken(1),
        owner_type: Some(std::any::TypeId::of::<UiLayout>()),
        access: &FieldAccess::Editable,
        diagnostic: None,
        metadata: None,
    }
}
#[test]
fn layout_groups_keep_native_paths_retained_fields_and_nested_disclosure_authority() -> TestResult {
    let (mut app, parent, templates) = fixture()?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let editors = kedra_inspector::builtin::editors()?;
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "layout",
    };
    let mut source = UiLayout {
        node: Node {
            width: px(24),
            height: percent(100),
            ..default()
        },
    };
    let mut tree = TreeView::mount(
        app.world_mut(),
        parent,
        &mut context,
        binding(&source),
        false,
    )?;
    let width_path = ParsedPath::from(
        ParsedPath::parse("node.width")?
            .0
            .into_iter()
            .map(|part| part.access)
            .collect::<Vec<_>>(),
    );
    let fields: Vec<_> = app
        .world_mut()
        .query::<(Entity, &ReflectedField)>()
        .iter(app.world())
        .filter(|(_, field)| field.path == width_path)
        .map(|(entity, field)| (entity, field.clone()))
        .collect();
    let [(width, field)] = fields.as_slice() else {
        return Err("Width must have exactly one native field binding across groups".into());
    };
    assert_eq!(
        field.propose("33")?.try_downcast_ref::<Val>(),
        Some(&Val::Px(33.))
    );
    assert_eq!(source.node.width, px(24));
    let action = app
        .world_mut()
        .query::<(&kedra_ui::ElementId, &Disclosure)>()
        .iter(app.world())
        .find(|(id, _)| id.0.ends_with("/layout/0/toggle"))
        .map(|(_, action)| action.clone())
        .ok_or("Size disclosure")?;
    assert!(tree.owns_disclosure(&action));
    assert!(tree.disclose(&action));
    *context.budget = RenderBudget::default();
    tree.update(app.world_mut(), &mut context, binding(&source), false)?;
    assert!(app.world().get::<ReflectedField>(*width).is_some());
    let body = app
        .world_mut()
        .query::<(Entity, &kedra_ui::ElementId)>()
        .iter(app.world())
        .find(|(_, id)| id.0.ends_with("/layout/0/body"))
        .map(|(entity, _)| entity)
        .ok_or("Size body")?;
    assert!(
        app.world()
            .get::<bevy_flair::style::components::ClassList>(body)
            .ok_or("body classes")?
            .contains("hidden")
    );
    tree.filter("width");
    *context.budget = RenderBudget::default();
    tree.update(app.world_mut(), &mut context, binding(&source), false)?;
    assert!(
        !app.world()
            .get::<bevy_flair::style::components::ClassList>(body)
            .ok_or("body classes")?
            .contains("hidden")
    );
    tree.filter("");
    *context.budget = RenderBudget::default();
    tree.update(app.world_mut(), &mut context, binding(&source), false)?;
    assert!(
        app.world()
            .get::<bevy_flair::style::components::ClassList>(body)
            .ok_or("body classes")?
            .contains("hidden")
    );
    assert!(tree.disclose(&action));
    source.node.width = px(48);
    *context.budget = RenderBudget::default();
    tree.update(app.world_mut(), &mut context, binding(&source), false)?;
    assert_eq!(
        app.world()
            .get::<ReflectedField>(*width)
            .ok_or("Retained width")?
            .display(),
        "48"
    );
    let stale = Disclosure {
        token: BindingToken(2),
        ..action
    };
    assert!(!tree.owns_disclosure(&stale));
    assert!(!tree.disclose(&stale));
    tree.retire(app.world_mut());
    assert!(app.world().entities().contains(parent));
    Ok(())
}

#[test]
fn selected_native_fields_do_not_spend_the_budget_on_excluded_collections() {
    #[derive(Reflect)]
    struct Layout {
        unrelated: Vec<f32>,
        other: Vec<f32>,
        more: Vec<f32>,
        width: f32,
    }
    let mut types = bevy::reflect::TypeRegistry::default();
    types.register::<Layout>();
    let value = Layout {
        width: 24.,
        unrelated: vec![1.; 1000],
        other: vec![1.; 1000],
        more: vec![1.; 1000],
    };
    let rows = kedra_inspector::discovery::inspect_fields(
        &value,
        &types,
        kedra_inspector::discovery::Limits {
            rows: 3,
            ..default()
        },
        &std::collections::BTreeSet::from(["width".to_owned()]),
    );
    assert!(rows.iter().any(|row| {
        row.path
            .as_ref()
            .is_some_and(|path| path.to_string() == ".width")
    }));
    assert!(
        !rows
            .iter()
            .any(|row| row.label == "unrelated" || row.incomplete)
    );
}
