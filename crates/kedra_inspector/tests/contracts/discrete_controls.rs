//! Inspector-owned proposal identity and controlled state; not OS input acceptance.
use crate::support::*;
use bevy::{prelude::*, reflect::TypeRegistry};
use kedra_inspector::{
    binding::{BindingToken, ValueBinding},
    controls::*,
    editors::*,
};
use kedra_ui::fields::FieldAccess;

#[derive(Resource, Default)]
struct Proposals(Vec<ControlEvent>);
fn binding<'a>(value: &'a bool, access: &'a FieldAccess) -> ValueBinding<'a> {
    ValueBinding {
        value,
        access,
        token: BindingToken(1),
        label: "Enabled",
        path: None,
        owner_type: None,
        diagnostic: None,
        metadata: None,
    }
}
#[test]
fn checkbox_proposes_without_changing_checked_state_and_revokes_disabled_actions() -> TestResult {
    let (mut app, parent, templates) = fixture()?;
    app.add_plugins(InspectorControlsPlugin)
        .init_resource::<Proposals>()
        .add_observer(
            |event: On<ControlEvent>, mut proposals: ResMut<Proposals>| {
                proposals.0.push(event.clone())
            },
        );
    let types = TypeRegistry::default();
    let editors = EditorRegistry::default();
    let mut budget = RenderBudget::default();
    let context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "choice",
    };
    let view = ChoiceView::mount(app.world_mut(), parent, &context, "enabled")?;
    let access = FieldAccess::Editable;
    let current = binding(&false, &access);
    view.update(
        app.world_mut(),
        current,
        parent,
        "false",
        finite_choices(current).ok_or("choices")?,
    )?;
    view.checkbox(app.world_mut(), false, "Enabled");
    assert_eq!(
        app.world()
            .get::<bevy::a11y::AccessibilityNode>(view.root())
            .ok_or("accessible checkbox")?
            .label(),
        Some("Enabled")
    );
    app.world_mut().trigger(bevy::ui_widgets::ValueChange {
        source: view.root(),
        value: true,
        is_final: true,
    });
    app.world_mut().flush();
    let proposals = &app.world().resource::<Proposals>().0;
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].value.try_downcast_ref::<bool>(), Some(&true));
    assert!(app.world().get::<bevy::ui::Checked>(view.root()).is_none());
    app.world_mut()
        .entity_mut(view.root())
        .insert(FieldAccess::ReadOnly {
            reason: "Retired source".into(),
        });
    app.world_mut().trigger(bevy::ui_widgets::ValueChange {
        source: view.root(),
        value: true,
        is_final: true,
    });
    app.world_mut().flush();
    assert_eq!(app.world().resource::<Proposals>().0.len(), 1);
    view.retire(app.world_mut());
    assert!(app.world().entities().contains(parent));
    Ok(())
}

#[test]
fn menu_actions_keep_original_binding_and_cannot_borrow_refreshed_source_identity() -> TestResult {
    let (mut app, parent, templates) = fixture()?;
    app.add_plugins(InspectorControlsPlugin)
        .init_resource::<Proposals>()
        .add_observer(
            |event: On<ControlEvent>, mut proposals: ResMut<Proposals>| {
                proposals.0.push(event.clone())
            },
        );
    let types = TypeRegistry::default();
    let editors = EditorRegistry::default();
    let mut budget = RenderBudget::default();
    let context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "choice",
    };
    let view = ChoiceView::mount(app.world_mut(), parent, &context, "enabled")?;
    let access = FieldAccess::Editable;
    let current = binding(&false, &access);
    view.update(
        app.world_mut(),
        current,
        parent,
        "false",
        finite_choices(current).ok_or("choices")?,
    )?;
    app.world_mut().trigger(bevy::ui_widgets::Activate {
        entity: view.root(),
    });
    app.world_mut().flush();
    let item = app
        .world_mut()
        .query_filtered::<Entity, With<bevy::ui_widgets::MenuItem>>()
        .iter(app.world())
        .next()
        .ok_or("menu item")?;
    let original = app
        .world()
        .get::<Control>(view.root())
        .ok_or("control")?
        .0
        .clone();
    view.update(
        app.world_mut(),
        current,
        parent,
        "false",
        finite_choices(current).ok_or("choices")?,
    )?;
    assert!(!std::sync::Arc::ptr_eq(
        &original,
        &app.world().get::<Control>(view.root()).ok_or("control")?.0
    ));
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: item });
    app.world_mut().flush();
    assert!(app.world().resource::<Proposals>().0.is_empty());
    assert!(!app.world().entities().contains(item));
    assert!(app.world().entities().contains(view.root()));
    Ok(())
}

#[derive(Reflect, Debug, PartialEq)]
enum Payload {
    Off,
    Radius(f32),
    Named { name: String },
}
#[test]
fn payload_variants_use_native_defaults_and_keep_the_current_exact_payload() -> TestResult {
    let mut types = TypeRegistry::default();
    types.register::<Payload>();
    types.register::<f32>();
    types.register::<String>();
    let current = Payload::Radius(1.234567);
    let choices = enum_choices(&current, &types).ok_or("native variants")?;
    assert_eq!(
        Payload::from_reflect(choices[0].1.as_ref()),
        Some(Payload::Off)
    );
    assert_eq!(Payload::from_reflect(choices[1].1.as_ref()), Some(current));
    assert_eq!(
        Payload::from_reflect(choices[2].1.as_ref()),
        Some(Payload::Named {
            name: String::new()
        })
    );
    let missing = TypeRegistry::empty();
    assert!(enum_choices(&Payload::Off, &missing).is_none());
    Ok(())
}
