//! Consumer-owned reflected values; independent of project/source inspection.
use bevy::{prelude::*, reflect::ParsedPath};
use kedra_inspector::{
    binding::{BindingToken, ValueBinding},
    editors::{EditorContext, EditorRegistry, RenderBudget, ValueEditor, ValueEditorView},
    view::{Disclosure, TreeView},
};
use kedra_ui::{
    authored::view::UiView,
    fields::{FieldAccess, FieldTemplates, ValueView},
};
use std::{any::TypeId, sync::Arc};

#[derive(Clone, Reflect)]
struct Level {
    current: u16,
    maximum: u16,
}
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub(super) struct Settings {
    name: String,
    enabled: bool,
    gain: f32,
    threshold: f32,
    exact: u64,
    level: Level,
    samples: Vec<u32>,
}
#[derive(Resource)]
struct Fixture {
    root: UiView,
    tree: Option<TreeView>,
    templates: FieldTemplates,
    editors: EditorRegistry,
    source: Settings,
    raw: bool,
    dirty: bool,
    width: u32,
}
#[derive(Component, Clone, Copy)]
enum Command {
    Toggle,
    Refresh,
}
struct Compact {
    percent: bool,
}
struct CompactView(ValueView, bool);
impl ValueEditor for Compact {
    fn mount(
        &self,
        world: &mut World,
        parent: Entity,
        context: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<Box<dyn ValueEditorView>, String> {
        let mut view = CompactView(
            ValueView::mount(world, parent, context.templates, context.scope)
                .map_err(|e| e.to_string())?,
            self.percent,
        );
        if let Err(error) = view.update(world, context, binding) {
            view.retire(world);
            return Err(error);
        }
        Ok(Box::new(view))
    }
}
impl ValueEditorView for CompactView {
    fn root(&self) -> Entity {
        self.0.root()
    }
    fn update(
        &mut self,
        world: &mut World,
        _: &mut EditorContext<'_>,
        binding: ValueBinding<'_>,
    ) -> Result<(), String> {
        let text = if self.1 {
            format!(
                "{}%",
                binding
                    .value
                    .try_downcast_ref::<f32>()
                    .ok_or("Expected native f32")?
                    * 100.
            )
        } else {
            let level = binding
                .value
                .try_downcast_ref::<Level>()
                .ok_or("Expected native Level")?;
            format!("{} / {}", level.current, level.maximum)
        };
        self.0
            .update(world, &text, binding.access)
            .map_err(|e| e.to_string())?;
        self.0
            .describe(
                world,
                binding.label,
                "Read-only fixture specialization; default view retains every native field",
            )
            .map_err(|e| e.to_string())
    }
}

pub(super) fn install(app: &mut App) {
    app.add_observer(activate).add_systems(Update, update);
    kedra_app::scripting::register(
        app,
        kedra_script::Program::sequence(
            "inspector.presentation",
            "crates/kedra_editor/src/diagnostics/values.luau",
            include_str!("values.luau"),
            kedra_app::scripting::install_app_api,
        ),
    );
}

pub(super) fn bind(world: &mut World, root: UiView) -> Result<(), String> {
    if world
        .get_resource::<Fixture>()
        .is_some_and(|fixture| world.entities().contains(fixture.root.root))
    {
        return Err("Reflected-value consumer is already bound".into());
    }
    let source = world
        .get::<Settings>(root.root)
        .ok_or("Missing reflected values")?
        .clone();
    for (key, command) in [("toggle", Command::Toggle), ("refresh", Command::Refresh)] {
        world
            .entity_mut(root.element(key).map_err(|e| e.to_string())?)
            .insert(command);
    }
    let templates = crate::inspector::templates(world, 1.)?;
    let mut editors = EditorRegistry::default();
    editors.register_type(TypeId::of::<Level>(), Arc::new(Compact { percent: false }))?;
    editors.register_field(
        TypeId::of::<Settings>(),
        ParsedPath::parse("gain").map_err(|e| e.to_string())?,
        Arc::new(Compact { percent: true }),
    )?;
    world.insert_resource(Fixture {
        root,
        tree: None,
        templates,
        editors,
        source,
        raw: false,
        dirty: true,
        width: 0,
    });
    Ok(())
}

pub(super) fn retire_binding(world: &mut World, root: Entity) {
    if world
        .get_resource::<Fixture>()
        .is_some_and(|fixture| fixture.root.root == root)
    {
        world.remove_resource::<Fixture>();
    }
}

#[cfg(test)]
pub(super) fn has_binding(world: &World) -> bool {
    world.contains_resource::<Fixture>()
}
#[cfg(test)]
pub(super) fn binding_entity(world: &World) -> Option<Entity> {
    world.resource_entity::<Fixture>()
}
fn activate(
    event: On<bevy::ui_widgets::Activate>,
    commands_query: Query<&Command>,
    disclosures: Query<&Disclosure>,
    mut commands: Commands,
) {
    let command = commands_query.get(event.entity).ok().copied();
    let disclosure = disclosures.get(event.entity).ok().cloned();
    if command.is_none() && disclosure.is_none() {
        return;
    }
    commands.queue(move |world: &mut World| {
        let Some(mut fixture) = world.get_resource_mut::<Fixture>() else {
            return;
        };
        match command {
            Some(Command::Toggle) => fixture.raw = !fixture.raw,
            Some(Command::Refresh) => fixture.source.enabled = !fixture.source.enabled,
            None => {}
        }
        if let Some(action) = disclosure
            && let Some(tree) = &mut fixture.tree
            && !tree.disclose(&action)
            && command.is_none()
        {
            return;
        }
        fixture.dirty = true;
    });
}
fn update(world: &mut World) {
    if !world.contains_resource::<Fixture>() {
        return;
    }
    let retired = !world
        .entities()
        .contains(world.resource::<Fixture>().root.root);
    if retired {
        world.remove_resource::<Fixture>();
        return;
    }
    world.resource_scope(|world, mut fixture: Mut<Fixture>| {
        if let Err(error) = tick(world, &mut fixture) {
            eprintln!("Inspector fixture failed: {error}");
            world.write_message(AppExit::error());
        }
    });
}
fn tick(world: &mut World, fixture: &mut Fixture) -> Result<(), String> {
    let root = &fixture.root;
    let width = world
        .get::<ComputedNode>(root.root)
        .map_or(0, |n| (n.size().x * n.inverse_scale_factor()) as u32);
    if width != fixture.width {
        fixture.width = width;
        fixture.dirty = true;
    }
    if !fixture.dirty {
        return Ok(());
    }
    let registry = world.resource::<AppTypeRegistry>().clone();
    let types = registry.read();
    let access = FieldAccess::ReadOnly {
        reason: "Independent consumer-owned value".into(),
    };
    let binding = ValueBinding {
        token: BindingToken(1),
        value: &fixture.source,
        label: "Settings",
        access: &access,
        owner_type: Some(TypeId::of::<Settings>()),
        path: None,
        diagnostic: None,
        metadata: None,
    };
    let mut budget = RenderBudget::default();
    let mut context = EditorContext {
        templates: &fixture.templates,
        types: &types,
        editors: &fixture.editors,
        budget: &mut budget,
        scope: "fixture/settings",
    };
    if let Some(tree) = &mut fixture.tree {
        tree.update(world, &mut context, binding, fixture.raw)?;
    } else {
        fixture.tree = Some(TreeView::mount(
            world,
            root.element("values").map_err(|e| e.to_string())?,
            &mut context,
            binding,
            fixture.raw,
        )?);
    }
    fixture.dirty = false;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inspector_scene_preserves_nested_controls_values_and_viewport() -> Result<(), String> {
        let mut app = crate::test_support::diagnostic_app();
        let scene = crate::views::compiled(app.world(), "editor://fixtures/inspector.scene.ron")?;
        crate::diagnostics::mount(app.world_mut(), &scene).map_err(|error| error.to_string())?;
        let world = app.world();
        let fixture = world.resource::<Fixture>();
        for key in ["toggle", "refresh", "values"] {
            let entity = fixture
                .root
                .element(key)
                .map_err(|error| error.to_string())?;
            assert_eq!(
                world
                    .get::<kedra_ui::ElementId>(entity)
                    .map(|id| id.0.as_str()),
                Some(format!("fixture/{key}").as_str())
            );
        }
        assert_eq!(fixture.source.name, "Calibration 名称");
        assert!(fixture.source.enabled);
        assert_eq!(fixture.source.gain, 0.25);
        assert_eq!(fixture.source.threshold, 0.33333334);
        assert_eq!(fixture.source.exact, u64::MAX);
        assert_eq!(fixture.source.level.current, 3);
        assert_eq!(fixture.source.level.maximum, 9);
        assert_eq!(fixture.source.samples, [2, 3, 5]);
        let window = world
            .resource::<kedra_desktop::DesktopSurface>()
            .window
            .ok_or("Inspector window missing")?;
        assert_eq!(
            world.get::<Window>(window).ok_or("Window missing")?.size(),
            Vec2::new(420., 740.)
        );
        Ok(())
    }

    #[test]
    fn document_disclosures_do_not_require_a_value_consumer() {
        let mut app = App::new();
        app.add_observer(activate);
        let owner = app.world_mut().spawn_empty().id();
        let entity = app
            .world_mut()
            .spawn(Disclosure {
                owner,
                key: vec![],
                token: BindingToken(1),
            })
            .id();
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity });
        app.world_mut().flush();
        assert!(!app.world().contains_resource::<Fixture>());
        update(app.world_mut());
        assert!(app.world().entities().contains(owner));
    }
}
