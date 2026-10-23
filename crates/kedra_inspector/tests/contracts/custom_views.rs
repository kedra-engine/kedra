//! Real native custom presentations use the same kit and immutable bindings.
use crate::support::*;
use bevy::{
    prelude::*,
    reflect::{ParsedPath, TypeRegistry},
};
use kedra_inspector::{
    binding::{BindingToken, ValueBinding},
    editors::{EditorContext, EditorRegistry, RenderBudget, ValueEditor, ValueEditorView},
    view::TreeView,
};
use kedra_ui::fields::{FieldAccess, ValueView};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{any::TypeId, sync::Arc};

#[derive(Reflect)]
struct Settings {
    gain: f32,
    description: String,
}
struct Percent;
struct PercentView(ValueView);
impl ValueEditor for Percent {
    fn mount(
        &self,
        world: &mut World,
        parent: Entity,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<Box<dyn ValueEditorView>, String> {
        let mut view = PercentView(
            ValueView::mount(world, parent, context.templates, context.scope)
                .map_err(|e| e.to_string())?,
        );
        view.update(world, context, binding)?;
        Ok(Box::new(view))
    }
}

struct Broken(Arc<AtomicUsize>);
impl ValueEditor for Broken {
    fn mount(
        &self,
        world: &mut World,
        parent: Entity,
        _: &mut EditorContext<'_>,
        _: ValueBinding<'_>,
    ) -> Result<Box<dyn ValueEditorView>, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        world.spawn((Node::default(), ChildOf(parent)));
        Err("Broken presentation".into())
    }
}
#[test]
fn failed_custom_mount_is_retained_as_a_diagnostic_without_retrying_or_leaking() -> TestResult {
    let (mut app, parent, templates) = fixture()?;
    let mut types = TypeRegistry::default();
    types.register::<f32>();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut editors = EditorRegistry::default();
    editors.register_type(TypeId::of::<f32>(), Arc::new(Broken(calls.clone())))?;
    let access = FieldAccess::ReadOnly {
        reason: "Fixture".into(),
    };
    let source = 0.5f32;
    let binding = ValueBinding {
        token: BindingToken(1),
        value: &source,
        label: "Gain",
        access: &access,
        owner_type: Some(TypeId::of::<f32>()),
        path: None,
        diagnostic: None,
        metadata: None,
    };
    let before = app.world().entities().count_spawned();
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "fixture",
    };
    let mut tree = TreeView::mount(app.world_mut(), parent, &mut context, binding, false)?;
    let after = app.world().entities().count_spawned();
    for _ in 0..3 {
        let mut budget = RenderBudget::default();
        let mut context = EditorContext {
            templates: &templates,
            types: &types,
            editors: &editors,
            budget: &mut budget,
            scope: "fixture",
        };
        tree.update(app.world_mut(), &mut context, binding, false)?;
        assert_eq!(app.world().entities().count_spawned(), after);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|t| t.0.contains("Broken presentation"))
    );
    tree.retire(app.world_mut());
    assert_eq!(app.world().entities().count_spawned(), before);
    Ok(())
}
impl ValueEditorView for PercentView {
    fn root(&self) -> Entity {
        self.0.root()
    }
    fn update(
        &mut self,
        world: &mut World,
        _: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<(), String> {
        let value = binding
            .value
            .try_downcast_ref::<f32>()
            .ok_or("Expected exact f32")?;
        self.0
            .update(world, &format!("{}%", value * 100.), binding.access)
            .map_err(|e| e.to_string())
    }
}
fn text(world: &mut World) -> Vec<String> {
    world
        .query::<&bevy::text::EditableText>()
        .iter(world)
        .map(|v| v.value().to_string())
        .collect()
}
#[test]
fn real_custom_controls_retain_native_roots_and_default_view_reveals_original_values() -> TestResult
{
    let (mut app, parent, templates) = fixture()?;
    let mut types = TypeRegistry::default();
    types.register::<Settings>();
    let mut editors = EditorRegistry::default();
    editors.register_type(TypeId::of::<f32>(), Arc::new(Percent))?;
    editors.register_field(
        TypeId::of::<Settings>(),
        ParsedPath::parse("gain")?,
        Arc::new(Percent),
    )?;
    let source = Settings {
        gain: 0.25,
        description: "source".into(),
    };
    let access = FieldAccess::ReadOnly {
        reason: "Fixture".into(),
    };
    let binding = ValueBinding {
        token: BindingToken(1),
        value: &source,
        label: "Settings",
        access: &access,
        owner_type: Some(TypeId::of::<Settings>()),
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
        scope: "fixture",
    };
    let mut tree = TreeView::mount(app.world_mut(), parent, &mut context, binding, false)?;
    assert!(text(app.world_mut()).contains(&"25%".into()));
    let roots: Vec<_> = app
        .world_mut()
        .query::<(Entity, &bevy::text::EditableText)>()
        .iter(app.world())
        .map(|(e, _)| e)
        .collect();
    for _ in 0..3 {
        let mut budget = RenderBudget::default();
        let mut context = EditorContext {
            templates: &templates,
            types: &types,
            editors: &editors,
            budget: &mut budget,
            scope: "fixture",
        };
        tree.update(app.world_mut(), &mut context, binding, false)?;
        let actual: Vec<_> = app
            .world_mut()
            .query::<(Entity, &bevy::text::EditableText)>()
            .iter(app.world())
            .map(|(e, _)| e)
            .collect();
        assert_eq!(actual, roots);
    }
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "fixture",
    };
    tree.update(app.world_mut(), &mut context, binding, true)?;
    assert!(text(app.world_mut()).contains(&"0.25".into()));
    assert!(!text(app.world_mut()).contains(&"25%".into()));
    assert_eq!(source.gain, 0.25);
    tree.retire(app.world_mut());
    assert!(text(app.world_mut()).is_empty());
    Ok(())
}

#[test]
fn sibling_consumers_share_the_discovery_budget_and_failed_mount_reclaims_its_root() -> TestResult {
    let (mut app, parent, templates) = fixture()?;
    let mut types = TypeRegistry::default();
    types.register::<Vec<Vec<u32>>>();
    let source = vec![vec![1u32; 128]; 128];
    let access = FieldAccess::ReadOnly {
        reason: "Fixture".into(),
    };
    let binding = ValueBinding {
        token: BindingToken(1),
        value: &source,
        label: "Bounded",
        access: &access,
        owner_type: None,
        path: None,
        diagnostic: None,
        metadata: None,
    };
    let editors = EditorRegistry::default();
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "first",
    };
    let tree = TreeView::mount(app.world_mut(), parent, &mut context, binding, false)?;
    assert!(context.budget.exhausted());
    let before = app.world().entities().count_spawned();
    context.scope = "second";
    assert!(TreeView::mount(app.world_mut(), parent, &mut context, binding, false).is_err());
    assert_eq!(before, app.world().entities().count_spawned());
    tree.retire(app.world_mut());
    Ok(())
}

#[derive(Reflect)]
enum RootChoice {
    Off,
    On { count: u32 },
}

#[test]
fn omitted_collection_and_row_budget_matches_remain_an_incomplete_search() -> TestResult {
    let mut collection_limited = vec![vec![0u32; 129]];
    collection_limited[0][128] = 987654321;
    let mut row_limited = vec![vec![0u32; 128]; 128];
    row_limited[127][127] = 987654321;
    for source in [collection_limited, row_limited] {
        let (mut app, parent, templates) = fixture()?;
        let mut types = TypeRegistry::default();
        types.register::<Vec<Vec<u32>>>();
        let access = FieldAccess::ReadOnly {
            reason: "Fixture".into(),
        };
        let binding = ValueBinding {
            token: BindingToken(1),
            value: &source,
            label: "Collection",
            access: &access,
            owner_type: None,
            path: None,
            diagnostic: None,
            metadata: None,
        };
        let editors = EditorRegistry::default();
        let mut budget = RenderBudget::default();
        let mut context = EditorContext {
            templates: &templates,
            types: &types,
            editors: &editors,
            budget: &mut budget,
            scope: "incomplete",
        };
        let tree = TreeView::mount_filtered(
            app.world_mut(),
            parent,
            &mut context,
            binding,
            false,
            "987654321",
        )?;
        assert!(!tree.has_matches());
        assert!(tree.search_incomplete());
        let classes = app
            .world()
            .get::<bevy_flair::style::components::ClassList>(tree.root())
            .ok_or("root classes")?;
        assert!(
            !classes.contains("hidden"),
            "Do not filter out the incomplete-search warning"
        );
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|t| t.0.contains("Search incomplete"))
        );
        tree.retire(app.world_mut());
    }
    Ok(())
}

#[test]
fn root_enum_variant_and_initial_search_have_explicit_presentations() -> TestResult {
    let (mut app, parent, templates) = fixture()?;
    let mut types = TypeRegistry::default();
    types.register::<RootChoice>();
    let source = RootChoice::Off;
    let access = FieldAccess::ReadOnly {
        reason: "Fixture".into(),
    };
    let binding = ValueBinding {
        token: BindingToken(1),
        value: &source,
        label: "Mode",
        access: &access,
        owner_type: None,
        path: None,
        diagnostic: None,
        metadata: None,
    };
    let editors = EditorRegistry::default();
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "enum",
    };
    let tree = TreeView::mount(app.world_mut(), parent, &mut context, binding, false)?;
    assert!(
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|t| t.0.contains("Off")),
        "Current root variant must be visible even without payload fields"
    );
    tree.retire(app.world_mut());
    let source = Settings {
        gain: 0.5,
        description: "unrelated".into(),
    };
    types.register::<Settings>();
    let binding = ValueBinding {
        value: &source,
        label: "Settings",
        ..binding
    };
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &templates,
        types: &types,
        editors: &editors,
        budget: &mut budget,
        scope: "filtered",
    };
    let tree = TreeView::mount_filtered(
        app.world_mut(),
        parent,
        &mut context,
        binding,
        false,
        "missing",
    )?;
    assert!(!tree.has_matches());
    tree.retire(app.world_mut());
    Ok(())
}
