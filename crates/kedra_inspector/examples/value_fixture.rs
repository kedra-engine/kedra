//! Native consumer of the kit and inspector without editor or authoring dependencies.
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
use std::{any::TypeId, collections::BTreeMap, path::PathBuf, sync::Arc};

#[derive(Reflect)]
struct Level {
    current: u16,
    maximum: u16,
}
#[derive(Reflect)]
struct Settings {
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
    root: Option<UiView>,
    tree: Option<TreeView>,
    templates: Option<FieldTemplates>,
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 && args.len() != 3 {
        return Err(
            "Usage: value_fixture VERIFIED_DELIVERY [NEW_CREDENTIALS ARTIFACT_DIRECTORY]".into(),
        );
    }
    eprintln!("Verifying inspector fixture delivery");
    let (root, manifest) =
        kedra_assets::delivery::read_directory(&PathBuf::from(&args[0]), "editor-desktop")?;
    let mut app = App::new();
    kedra_assets::register_source(
        &mut app,
        "editor",
        root.to_str().ok_or("Delivery path must be Unicode")?.into(),
        manifest,
    );
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Independent reflected value inspector".into(),
                    resolution: (420, 740).into(),
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                use_asset_processor_override: Some(false),
                watch_for_changes_override: Some(false),
                ..default()
            }),
    )
    .add_plugins((
        kedra_app::KedraAppPlugin,
        bevy::input_focus::tab_navigation::TabNavigationPlugin,
        kedra_ui::authored::AuthoredUiPlugin,
        kedra_ui::tooltip::TooltipPlugin,
        kedra_scene::assets::PrefabPlugin {
            catalog_path: "editor://fixtures/inspector.prefabs.ron".into(),
        },
    ))
    .add_systems(Startup, |mut commands: Commands| {
        commands.spawn(Camera2d);
    })
    .add_observer(activate)
    .add_systems(Update, update);
    app.world()
        .resource::<AppTypeRegistry>()
        .write()
        .register::<Settings>();
    let mut editors = EditorRegistry::default();
    editors.register_type(TypeId::of::<Level>(), Arc::new(Compact { percent: false }))?;
    editors.register_field(
        TypeId::of::<Settings>(),
        ParsedPath::parse("gain")?,
        Arc::new(Compact { percent: true }),
    )?;
    app.insert_resource(Fixture {
        root: None,
        tree: None,
        templates: None,
        editors,
        source: Settings {
            name: "Calibration 名称".into(),
            enabled: true,
            gain: 0.25,
            threshold: 0.33333334,
            exact: u64::MAX,
            level: Level {
                current: 3,
                maximum: 9,
            },
            samples: vec![2, 3, 5],
        },
        raw: false,
        dirty: true,
        width: 0,
    });
    if args.len() == 3 {
        let endpoint = kedra_app::transport::enable(
            &mut app,
            0,
            &PathBuf::from(&args[1]),
            &PathBuf::from(&args[2]),
        )?;
        eprintln!("Fixture automation listening at {}", endpoint.url);
    }
    eprintln!("Loading shared authored property kit");
    if app.run().is_error() {
        return Err("Inspector fixture failed; inspect native diagnostics".into());
    }
    eprintln!("Inspector fixture stopped successfully");
    Ok(())
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
        let mut fixture = world.resource_mut::<Fixture>();
        match command {
            Some(Command::Toggle) => fixture.raw = !fixture.raw,
            Some(Command::Refresh) => fixture.source.enabled = !fixture.source.enabled,
            None => {}
        }
        if let Some(action) = disclosure
            && let Some(tree) = &mut fixture.tree
        {
            tree.disclose(&action);
        }
        fixture.dirty = true;
    });
}
fn update(world: &mut World) {
    world.resource_scope(|world, mut fixture: Mut<Fixture>| {
        if let Err(error) = tick(world, &mut fixture) {
            eprintln!("Inspector fixture failed: {error}");
            world.write_message(AppExit::error());
        }
    });
}
fn tick(world: &mut World, fixture: &mut Fixture) -> Result<(), String> {
    let sources = world.resource::<kedra_scene::assets::PrefabSources>();
    if let Some(error) = &sources.error {
        return Err(error.clone());
    }
    if !sources.ready() {
        return Ok(());
    }
    let snapshot = sources.snapshot();
    let compiled = |id: u128| {
        snapshot
            .compiled
            .iter()
            .find_map(|(key, p)| (key.0.as_u128() == id).then(|| p.clone()))
            .ok_or("Fixture UI source is missing".to_string())
    };
    if fixture.root.is_none() {
        let root = UiView::mount(
            world,
            &compiled(0x19a80000000040008000000000000001)?,
            None,
            &BTreeMap::new(),
            "fixture",
            1.,
        )
        .map_err(|e| e.to_string())?;
        world
            .entity_mut(root.root)
            .insert(kedra_ui::ScreenId("inspector-fixture".into()));
        for (key, command) in [("toggle", Command::Toggle), ("refresh", Command::Refresh)] {
            world
                .entity_mut(root.element(key).map_err(|e| e.to_string())?)
                .insert(command);
        }
        fixture.templates = Some(FieldTemplates {
            row: compiled(0x19a00000000040008000000000000001)?,
            value: compiled(0x19a10000000040008000000000000001)?,
            section: compiled(0x19a20000000040008000000000000001)?,
            density: 1.,
        });
        fixture.root = Some(root);
        eprintln!("Shared property kit mounted; input/frame acceptance is separate");
    }
    let root = fixture.root.as_ref().ok_or("Fixture root missing")?;
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
        templates: fixture.templates.as_ref().ok_or("Kit missing")?,
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
