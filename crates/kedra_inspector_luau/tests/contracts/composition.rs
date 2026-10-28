//! Scoped native composition/lifetime contracts, not native input or frame acceptance.
use crate::support::*;
use bevy::prelude::*;
use kedra_inspector::{
    binding::{BindingToken, ValueBinding},
    editors::*,
    fields::ReflectedField,
};
use kedra_inspector_luau::LuauEditor;
use kedra_ui::fields::FieldAccess;

#[derive(Reflect)]
struct Example {
    name: String,
    enabled: bool,
    alignment: Justify,
}
fn binding(value: &Example) -> ValueBinding<'_> {
    ValueBinding {
        token: BindingToken(7),
        value,
        label: "Example",
        access: &FieldAccess::Editable,
        owner_type: Some(std::any::TypeId::of::<Example>()),
        path: None,
        metadata: None,
        diagnostic: None,
    }
}
fn fields(world: &mut World, owner: Entity) -> Vec<(Entity, ReflectedField)> {
    let inputs: Vec<_> = world
        .query::<(Entity, &ReflectedField)>()
        .iter(world)
        .map(|(entity, field)| (entity, field.clone()))
        .collect();
    inputs
        .into_iter()
        .filter(|(entity, _)| {
            world
                .query::<&ChildOf>()
                .query(world)
                .iter_ancestors(*entity)
                .any(|ancestor| ancestor == owner)
        })
        .collect()
}

#[test]
fn script_composition_binds_native_paths_and_refreshes_retained_controls_without_source_authority()
-> TestResult {
    let (mut app, parent, templates) = fixture()?;
    app.register_type::<Example>().register_type::<Justify>();
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let editors = EditorRegistry::default();
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "fixture/script",
    };
    let mut source = Example {
        name: "日本語".into(),
        enabled: false,
        alignment: Justify::Left,
    };
    let editor = LuauEditor::new(
        "fixture.drawer",
        "drawers/example.luau",
        "function run(ctx, args) ctx.fields.add('name', 'Name'); ctx.fields.add('enabled', 'Enabled'); ctx.fields.add('alignment', 'Alignment') end",
    );
    let mut view = editor.mount(app.world_mut(), parent, &mut context, binding(&source))?;
    app.update();
    view.update(app.world_mut(), &mut context, binding(&source))?;
    let inputs = fields(app.world_mut(), view.root());
    assert!(
        inputs
            .iter()
            .all(|(_, field)| kedra_inspector_luau::ready(app.world(), field))
    );
    let (input, name) = inputs
        .iter()
        .find(|(_, field)| field.display() == "日本語")
        .ok_or("name field")?;
    assert_eq!(
        name.propose("Кедра")?.try_downcast_ref::<String>(),
        Some(&"Кедра".to_string())
    );
    assert_eq!(source.name, "日本語");
    assert!(!source.enabled);
    let discrete: Vec<_> = app
        .world_mut()
        .query::<(Entity, &kedra_inspector::controls::Control)>()
        .iter(app.world())
        .map(|(entity, control)| (entity, control.0.clone()))
        .collect();
    let enabled = discrete
        .iter()
        .find(|(_, control)| control.path.to_string() == ".enabled")
        .ok_or("checkbox")?
        .0;
    assert!(
        app.world()
            .get::<bevy::ui_widgets::Checkbox>(enabled)
            .is_some()
    );
    assert!(app.world().get::<bevy::ui::Checked>(enabled).is_none());
    let alignment = discrete
        .iter()
        .find(|(_, control)| control.path.to_string() == ".alignment")
        .ok_or("alignment choice")?
        .0;
    assert!(
        app.world()
            .get::<bevy::ui_widgets::Button>(alignment)
            .is_some()
    );
    source.name = "Updated".into();
    source.enabled = true;
    view.update(app.world_mut(), &mut context, binding(&source))?;
    assert!(app.world().get::<bevy::ui::Checked>(enabled).is_some());
    assert!(
        app.world()
            .get::<kedra_inspector::controls::Control>(alignment)
            .is_some()
    );
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(*input)
            .ok_or("input")?
            .value(),
        "Updated"
    );
    let foreign = app.world_mut().spawn(Text::new("Foreign")).id();
    view.retire(app.world_mut());
    app.update();
    assert!(!app.world().entities().contains(*input));
    assert!(!kedra_inspector_luau::ready(app.world(), name));
    assert!(
        view.update(app.world_mut(), &mut context, binding(&source))
            .is_err()
    );
    assert_eq!(
        app.world().get::<Text>(foreign).ok_or("foreign")?.0,
        "Foreign"
    );
    Ok(())
}

#[test]
fn script_failure_disables_partial_controls_retains_diagnostic_and_leaves_default_view_available()
-> TestResult {
    for source in [
        "function run(ctx, args) ctx.fields.add('name','Name'); error('drawer failed') end",
        "function run(ctx, args) ctx.fields.add('name','Name'); ctx.fields.add('missing','Missing') end",
        "function run(ctx, args) ctx.fields.add('name','Name'); ctx.fields.add('.name','Duplicate') end",
    ] {
        let (mut app, parent, templates) = fixture()?;
        app.register_type::<Example>().register_type::<Justify>();
        let registry = app.world().resource::<AppTypeRegistry>().clone();
        let types = registry.read();
        let editors = EditorRegistry::default();
        let mut budget = RenderBudget::default();
        let mut context = EditorContext {
            templates: &templates,
            types: &types,
            editors: &editors,
            budget: &mut budget,
            scope: "fixture/failure",
        };
        let value = Example {
            name: "Preserved".into(),
            enabled: true,
            alignment: Justify::Right,
        };
        let editor = LuauEditor::new("fixture.failure", "drawers/failure.luau", source);
        let mut view = editor.mount(app.world_mut(), parent, &mut context, binding(&value))?;
        app.update();
        assert!(
            view.update(app.world_mut(), &mut context, binding(&value))
                .is_err()
        );
        for (input, field) in fields(app.world_mut(), view.root()) {
            assert!(!kedra_inspector_luau::ready(app.world(), &field));
            assert!(
                app.world()
                    .get::<FieldAccess>(input)
                    .is_some_and(FieldAccess::is_read_only)
            );
        }
        assert_eq!(value.name, "Preserved");
        assert_eq!(value.alignment, Justify::Right);
        view.retire(app.world_mut());
        let default = kedra_inspector::view::TreeView::mount(
            app.world_mut(),
            parent,
            &mut context,
            binding(&value),
            true,
        )?;
        default.retire(app.world_mut());
    }
    Ok(())
}
