//! Independent native docking capability. No editor, importer or project writer.
//! Native operations and projection, not diagnostic action sequences or key maps.
//! The editor separately tests its production gesture controller.
mod scripting;
use bevy::prelude::*;
use bevy_flair::style::components::ClassList;
use kedra_docking::{
    command::Command,
    model::{HostId, ViewId, Workspace},
    snapshot::Snapshot,
};
use kedra_scene::{
    assets::{PrefabCatalogAsset, PrefabSources},
    document::PrefabDocument,
};
use kedra_ui::{
    authored::view::UiView,
    docking::{Control, Projection, Skin},
};
use std::collections::BTreeMap;

#[derive(Resource)]
struct Lab {
    model: Workspace,
    original: Snapshot,
    host: HostId,
    root: Entity,
    stash: Entity,
    skin: Skin,
    projection: Projection,
    titles: BTreeMap<ViewId, String>,
    rendered: Option<u64>,
}
pub(super) fn install(app: &mut App) {
    app.add_systems(Update, run).add_observer(activate);
    scripting::install(app);
}
pub(super) fn bind(world: &mut World, canvas: &UiView) -> Result<(), String> {
    let Some(initial) = world.get::<kedra_ui::docking::scene::InitialLayout>(canvas.root) else {
        return Ok(());
    };
    let model = initial.workspace().map_err(|e| e.to_string())?;
    if model.snapshot().hosts.len() != 1 || model.snapshot().views.len() != 4 {
        return Err("Projection requires one authored host and four views".into());
    }
    let sources = world.resource::<PrefabSources>();
    if let Some(error) = &sources.error {
        return Err(error.clone());
    }
    if !sources.ready() {
        return Ok(());
    }
    let compiled = sources.snapshot();
    let get = |path: &str| {
        let asset_path = bevy::asset::AssetPath::try_parse(path)
            .map_err(|error| format!("Invalid projection prefab path: {error}"))?;
        if !matches!(asset_path.source(), bevy::asset::io::AssetSourceId::Name(_))
            || asset_path.label().is_some()
        {
            return Err(
                "Projection prefabs require a full source-qualified asset path without a label"
                    .into(),
            );
        }
        let handle = world
            .resource::<AssetServer>()
            .get_handle::<PrefabDocument>(asset_path)
            .ok_or("Projection prefab path is not loaded")?;
        let admitted = world
            .resource::<Assets<PrefabCatalogAsset>>()
            .get(&sources.handle)
            .is_some_and(|catalog| catalog.documents.contains(&handle));
        if !admitted {
            return Err("Projection prefab path is outside the loaded source catalog".into());
        }
        let document = world
            .resource::<Assets<PrefabDocument>>()
            .get(&handle)
            .ok_or("Projection prefab is not materialized")?;
        if compiled.catalog.documents.get(&document.id) != Some(document) {
            return Err("Projection prefab path differs from the published source revision".into());
        }
        compiled
            .compiled
            .get(&document.id)
            .cloned()
            .ok_or_else(|| "Laboratory prefab is absent".to_string())
    };
    let root = canvas.root;
    let skin = Skin {
        group: get("editor://prefabs/dock-group.prefab.ron")?,
        tab: get("editor://prefabs/dock-tab.prefab.ron")?,
        split: get("editor://prefabs/dock-split.prefab.ron")?,
    };
    let host = *model.snapshot().hosts.keys().next().ok_or("Missing host")?;
    let views: [ViewId; 4] = model
        .snapshot()
        .views
        .keys()
        .copied()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "Missing views")?;
    let mut titles = BTreeMap::new();
    let mut projection = Projection::default();
    let templates = views
        .iter()
        .map(|id| {
            get(model.snapshot().views[id]
                .restore_key
                .as_deref()
                .ok_or("Missing panel source path")?)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let stash = world
        .spawn((
            Node::default(),
            ClassList::new_with_classes(["dock-stash"]),
            ChildOf(root),
        ))
        .id();
    for (id, template) in views.iter().copied().zip(templates) {
        let descriptor = &model.snapshot().views[&id];
        let name = &descriptor.kind;
        titles.insert(id, format!("Panel {name}"));
        let view = UiView::mount(
            world,
            &template,
            Some(stash),
            &BTreeMap::new(),
            &format!("lab.{name}"),
            4.,
        )
        .map_err(|e| e.to_string())?;
        let heading = view.element("heading").map_err(|e| e.to_string())?;
        world
            .get_mut::<Text>(heading)
            .ok_or("Native panel heading is absent")?
            .0 = format!("Panel {name}");
        projection.register(id, view.root)?;
    }
    let original = model.snapshot().clone();
    world.insert_resource(Lab {
        model,
        original,
        host,
        root,
        stash,
        skin,
        projection,
        titles,
        rendered: None,
    });
    eprintln!("READY: independent native A/B/C/D projection and editable content mounted");
    Ok(())
}
fn run(world: &mut World) {
    let result = (|| {
        if !world.contains_resource::<Lab>() {
            return Ok(());
        }
        world.resource_scope(|world, mut lab: Mut<Lab>| {
            if lab.rendered != Some(lab.model.revision()) {
                let Lab {
                    model,
                    host,
                    root,
                    stash,
                    skin,
                    projection,
                    titles,
                    ..
                } = &mut *lab;
                projection.sync(
                    world,
                    model.snapshot(),
                    &BTreeMap::from([(*host, *root)]),
                    *stash,
                    skin,
                    titles,
                    "lab",
                    4.,
                )?;
                let menus: Vec<_> = world
                    .query::<(Entity, &Control)>()
                    .iter(world)
                    .filter_map(|(entity, control)| {
                        matches!(control, Control::Menu(_)).then_some(entity)
                    })
                    .collect();
                for entity in menus {
                    world
                        .entity_mut(entity)
                        .insert(bevy::ui::InteractionDisabled);
                }
                lab.rendered = Some(lab.model.revision());
            }
            lab.projection.constrain(world, lab.model.snapshot());
            Ok::<_, String>(())
        })
    })();
    if let Err(error) = result {
        error!("Independent docking host: {error}");
        world.write_message(AppExit::error());
    }
}
fn activate(
    event: On<bevy::ui_widgets::Activate>,
    controls: Query<&Control>,
    mut commands: Commands,
) {
    let Ok(control) = controls.get(event.entity).copied() else {
        return;
    };
    commands.queue(move |world: &mut World| {
        if !world.contains_resource::<Lab>() {
            return;
        }
        world.resource_scope(|_, mut lab: Mut<Lab>| {
            let command = match control {
                Control::Tab(view) => Some(Command::Activate(view)),
                Control::Close(view) => Some(Command::Hide(view)),
                Control::Menu(_) => None,
                Control::ResetSplit(split) => Some(Command::Resize {
                    split,
                    fraction: 0.5,
                }),
            };
            if let Some(command) = command
                && let Err(error) = lab.model.execute(command)
            {
                warn!("Lab action refused: {error}");
            }
        });
    });
}
