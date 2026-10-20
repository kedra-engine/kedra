//! Independent native docking projection host. No editor, importer or project writer.
//! KEDRA_LAB_ASSETS selects a verified editor-theme delivery (visual assets only).
//! F1 reorder D, F2 split D, F3 activate B, F4 maximize D, F5 hide/show D,
//! F6 restore topology; F12 exits. The editor separately tests its gesture controller.
use bevy::prelude::*;
use bevy_flair::style::components::{ClassList, Styled};
use kedra_docking::{
    command::{Command, Destination, Selection},
    model::{Edge, HostId, View, ViewId, Workspace},
    snapshot::Snapshot,
};
use kedra_scene::{
    assets::{PrefabPlugin, PrefabSources},
    document::DocumentId,
};
use kedra_ui::{
    authored::{AuthoredUiPlugin, view::UiView},
    docking::{Control, Projection, Skin},
};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Resource)]
struct Lab {
    model: Workspace,
    original: Snapshot,
    host: HostId,
    root: Entity,
    stash: Entity,
    skin: Skin,
    projection: Projection,
    views: [ViewId; 4],
    titles: BTreeMap<ViewId, String>,
    rendered: Option<u64>,
}
fn main() -> Result<(), String> {
    let assets = std::env::var_os("KEDRA_LAB_ASSETS")
        .map(PathBuf::from)
        .ok_or("Set KEDRA_LAB_ASSETS to a verified editor-theme delivery")?;
    let credentials = std::env::var_os("KEDRA_QA_CREDENTIALS")
        .map(PathBuf::from)
        .ok_or("Set KEDRA_QA_CREDENTIALS to a NEW credential path")?;
    let frames = std::env::var_os("KEDRA_QA_ARTIFACTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| "target/qa/docking-lab/frames".into());
    eprintln!(
        "Verifying independent docking laboratory assets: {}",
        assets.display()
    );
    let (assets, manifest) = kedra_assets::delivery::read_directory(&assets, "editor-desktop")?;
    let mut app = App::new();
    kedra_assets::register_source(
        &mut app,
        "editor",
        assets
            .to_str()
            .ok_or("Lab asset root must be Unicode")?
            .into(),
        manifest,
    );
    app.add_plugins((
        DefaultPlugins.set(AssetPlugin {
            use_asset_processor_override: Some(false),
            watch_for_changes_override: Some(false),
            ..default()
        }),
        kedra_app::KedraAppPlugin,
        bevy::input_focus::tab_navigation::TabNavigationPlugin,
        AuthoredUiPlugin,
        PrefabPlugin {
            catalog_path: "editor://scenes/startup.prefabs.ron".into(),
        },
    ))
    .add_systems(Update, run)
    .add_observer(activate);
    let endpoint = kedra_app::transport::enable(&mut app, 0, &credentials, &frames)?;
    eprintln!(
        "Independent docking host: {}; credentials: {}",
        endpoint.url,
        credentials.display()
    );
    eprintln!(
        "F1 reorder · F2 split · F3 activate · F4 maximize · F5 hide/show · F6 restore · F12 exit"
    );
    let exit = app.run();
    if exit.is_error() {
        Err("Independent docking host failed; see live diagnostics".into())
    } else {
        eprintln!("SUCCESS: independent docking host closed");
        Ok(())
    }
}
fn document(value: u128) -> DocumentId {
    DocumentId(uuid::Uuid::from_u128(value))
}
fn start(world: &mut World) -> Result<(), String> {
    let sources = world.resource::<PrefabSources>();
    if let Some(error) = &sources.error {
        return Err(error.clone());
    }
    if !sources.ready() {
        return Ok(());
    }
    let compiled = sources.snapshot();
    let get = |id| {
        compiled
            .compiled
            .get(&document(id))
            .cloned()
            .ok_or_else(|| "Laboratory prefab is absent".to_string())
    };
    let sheet = world
        .resource::<AssetServer>()
        .load("editor://styles/workbench.css");
    let root = world
        .spawn((
            bevy::ui::Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            ClassList::new_with_classes(["theme", "dock-host"]),
            Styled::new(sheet),
            kedra_ui::ScreenId("docking.lab".into()),
        ))
        .id();
    world.spawn(Camera2d);
    let stash = world
        .spawn((
            bevy::ui::Node::default(),
            ClassList::new_with_classes(["dock-stash"]),
            ChildOf(root),
        ))
        .id();
    let skin = Skin {
        group: get(0xd0100000000040008000000000000001)?,
        tab: get(0xd0200000000040008000000000000001)?,
        split: get(0xd0300000000040008000000000000001)?,
    };
    let host = HostId::new();
    let mut model = Workspace::new(host);
    let views = std::array::from_fn(|_| ViewId::new());
    let mut titles = BTreeMap::new();
    let mut projection = Projection::default();
    for (index, id) in views.iter().copied().enumerate() {
        let destination = if index == 0 {
            Destination::EmptyHost(host)
        } else {
            let target = model
                .snapshot()
                .location(views[if index == 3 { 2 } else { 0 }])
                .ok_or("Lab group is absent")?
                .0;
            match index {
                1 => Destination::Edge {
                    target,
                    edge: Edge::Right,
                    fraction: 0.75,
                },
                2 => {
                    let target = model
                        .snapshot()
                        .location(views[1])
                        .ok_or("Lab right group is absent")?
                        .0;
                    Destination::Edge {
                        target,
                        edge: Edge::Bottom,
                        fraction: 0.4,
                    }
                }
                _ => Destination::Tabs {
                    group: target,
                    index: 1,
                },
            }
        };
        model
            .execute(Command::Open {
                id,
                view: View {
                    kind: format!("lab.{}", index),
                    restore_key: None,
                },
                destination,
            })
            .map_err(|e| e.to_string())?;
        let name = ["A", "B", "C", "D"][index];
        titles.insert(id, format!("Panel {name}"));
        let view = UiView::mount(
            world,
            &get(0xd0400000000040008000000000000001)?,
            Some(stash),
            &BTreeMap::new(),
            &format!("lab.{name}"),
            4.,
        )
        .map_err(|e| e.to_string())?;
        for (key,text)in [("heading",format!("Panel {name}")),("information","Independent renderer consumer. F1 reorder D, F2 split, F4 maximize, F5 hide/show, F6 restore. Drafts are session-only.".into())]{
            let entity=view.element(key).map_err(|e|e.to_string())?;world.get_mut::<Text>(entity).ok_or("Native lab text is absent")?.0=text;
        }
        projection.register(id, view.root)?;
    }
    model
        .execute(Command::Activate(views[2]))
        .map_err(|e| e.to_string())?;
    let original = model.snapshot().clone();
    world.insert_resource(Lab {
        model,
        original,
        host,
        root,
        stash,
        skin,
        projection,
        views,
        titles,
        rendered: None,
    });
    eprintln!("READY: independent native A/B/C/D projection and editable content mounted");
    Ok(())
}
fn run(world: &mut World) {
    let result = (|| {
        if !world.contains_resource::<Lab>() {
            start(world)?;
            return Ok(());
        }
        world.resource_scope(|world, mut lab: Mut<Lab>| {
            let command = command(world, &lab)?;
            if let Some(command) = command {
                lab.model.execute(command).map_err(|e| e.to_string())?;
            }
            if world
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::F6)
            {
                lab.model = lab
                    .original
                    .clone()
                    .into_workspace()
                    .map_err(|e| e.to_string())?;
                lab.rendered = None;
            }
            if world
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::F12)
            {
                world.write_message(AppExit::Success);
            }
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
fn command(world: &World, lab: &Lab) -> Result<Option<Command>, String> {
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let d = lab.views[3];
    let target = lab
        .model
        .snapshot()
        .location(lab.views[0])
        .ok_or("Lab target A is absent")?
        .0;
    let command = if keys.just_pressed(KeyCode::F1) {
        let (group, _) = lab
            .model
            .snapshot()
            .location(d)
            .ok_or("Show D before reordering")?;
        Some(Command::Move {
            selection: Selection::View(d),
            destination: Destination::Tabs { group, index: 0 },
        })
    } else if keys.just_pressed(KeyCode::F2) {
        Some(Command::Move {
            selection: Selection::View(d),
            destination: Destination::Edge {
                target,
                edge: Edge::Right,
                fraction: 0.5,
            },
        })
    } else if keys.just_pressed(KeyCode::F3) {
        Some(Command::Activate(lab.views[1]))
    } else if keys.just_pressed(KeyCode::F4) {
        let group = lab
            .model
            .snapshot()
            .location(d)
            .ok_or("Show D before maximizing")?
            .0;
        Some(Command::Maximize {
            host: lab.host,
            group: if lab.model.snapshot().hosts[&lab.host].maximized == Some(group) {
                None
            } else {
                Some(group)
            },
        })
    } else if keys.just_pressed(KeyCode::F5) {
        Some(if lab.model.snapshot().hidden.contains_key(&d) {
            Command::Show {
                view: d,
                destination: Destination::Tabs {
                    group: target,
                    index: 0,
                },
            }
        } else {
            Command::Hide(d)
        })
    } else {
        None
    };
    Ok(command)
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
