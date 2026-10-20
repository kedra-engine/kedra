//! Independent consumer of the same retained projection, not an editor renderer.
use bevy::{
    asset::{AssetApp, AssetPlugin},
    prelude::*,
    text::EditableText,
};
use kedra_docking::{
    command::{Command, Destination, Selection},
    model::{Edge, HostId, View, ViewId, Workspace},
};
use kedra_scene::{
    composition::Catalog,
    document::{ComponentData, Node as SourceNode, NodeId, PrefabDocument},
    instance::CompiledPrefab,
    reflection::ComponentPolicies,
};
use kedra_ui::{
    authored::AuthoredUiPlugin,
    docking::{Projection, Skin},
};
use std::{collections::BTreeMap, sync::Arc};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[path = "support/docking_headers.rs"]
mod headers;

#[test]
fn dynamic_properties_reject_nonfinite_geometry_and_retired_targets_without_mutation() {
    let mut world = World::new();
    let entity = world.spawn(Node::default()).id();
    for weight in [f32::NAN, f32::INFINITY, -1., 2.] {
        assert!(
            kedra_ui::docking::geometry(
                &mut world,
                entity,
                kedra_ui::docking::Geometry::Flex(weight)
            )
            .is_err()
        );
    }
    assert!(
        kedra_ui::docking::geometry(
            &mut world,
            entity,
            kedra_ui::docking::Geometry::Rectangle(Rect {
                min: Vec2::ZERO,
                max: Vec2::splat(f32::NAN)
            })
        )
        .is_err()
    );
    assert!(
        world
            .get::<bevy_flair::prelude::InlineStyle>(entity)
            .is_none()
    );
    world.despawn(entity);
    assert!(
        kedra_ui::docking::geometry(&mut world, entity, kedra_ui::docking::Geometry::Flex(1.))
            .is_err()
    );
}

fn compile(app: &App, keys: &[&str]) -> Result<Arc<CompiledPrefab>> {
    let mut document = PrefabDocument::new("Independent docking skin");
    let root = document.root;
    document.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    for key in keys {
        let mut node = SourceNode {
            id: NodeId::new(),
            name: (*key).into(),
            parent: Some(root.into()),
            components: BTreeMap::new(),
        };
        node.components
            .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
        node.components.insert(
            "ui.id".into(),
            ComponentData::ron(1, &format!("(\"{key}\")"))?,
        );
        document.nodes.push(node);
    }
    compile_document(app, document)
}

fn compile_document(app: &App, document: PrefabDocument) -> Result<Arc<CompiledPrefab>> {
    let id = document.id;
    let mut catalog = Catalog::default();
    catalog.insert(document)?;
    let registry = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    Ok(Arc::new(CompiledPrefab::compile(
        catalog.resolve(id)?,
        &registry.read(),
        app.world().resource::<ComponentPolicies>(),
        &mut loader,
        &BTreeMap::new(),
    )?))
}

#[test]
fn native_ui_03_rejected_projection_preserves_attachments_focus_and_recovers() -> Result<()> {
    use bevy::input_focus::{FocusCause, InputFocus};
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_resource::<AppTypeRegistry>()
        .add_plugins(AuthoredUiPlugin);
    let skin = Skin {
        group: compile(&app, &["tabs", "body", "menu"])?,
        tab: compile(&app, &["label", "title", "close"])?,
        split: compile(&app, &["divider"])?,
    };
    let bad_group = compile(&app, &["tabs", "body"])?;
    let bad_tab = compile(&app, &["label", "title"])?;
    let bad_split = compile(&app, &[])?;
    let mut non_ui_binding = PrefabDocument::new("Non-UI menu binding");
    non_ui_binding.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    for key in ["tabs", "body", "menu"] {
        let mut components = BTreeMap::from([(
            "ui.id".into(),
            ComponentData::ron(1, &format!("(\"{key}\")"))?,
        )]);
        if key != "menu" {
            components.insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
        }
        non_ui_binding.nodes.push(SourceNode {
            id: NodeId::new(),
            name: key.into(),
            parent: Some(non_ui_binding.root.into()),
            components,
        });
    }
    let non_ui_group = compile_document(&app, non_ui_binding)?;
    // All semantic keys exist, but this later tab mount fails after new group
    // and split mounts have been staged. This exercises owned-allocation rollback.
    let mut missing_dependency = PrefabDocument::new("Unavailable tab dependency");
    missing_dependency.nodes[0]
        .components
        .insert("ui.layout".into(), ComponentData::ron(1, "(node:())")?);
    let sheet = kedra_ui::authored::components::UiStylesheet {
        asset: app
            .world()
            .resource::<AssetServer>()
            .load("unavailable-audit-tab.css"),
    };
    let data = app.world().resource::<ComponentPolicies>().encode(
        "ui.stylesheet",
        &sheet,
        &app.world().resource::<AppTypeRegistry>().read(),
        &BTreeMap::new(),
    )?;
    missing_dependency.nodes[0]
        .components
        .insert("ui.stylesheet".into(), data);
    for key in ["label", "title", "close"] {
        missing_dependency.nodes.push(SourceNode {
            id: NodeId::new(),
            name: key.into(),
            parent: Some(missing_dependency.root.into()),
            components: BTreeMap::from([
                ("ui.layout".into(), ComponentData::ron(1, "(node:())")?),
                (
                    "ui.id".into(),
                    ComponentData::ron(1, &format!("(\"{key}\")"))?,
                ),
            ]),
        });
    }
    let unavailable_tab = compile_document(&app, missing_dependency)?;
    let world = app.world_mut();
    world.init_resource::<InputFocus>();
    let host_id = HostId::new();
    let mut model = Workspace::new(host_id);
    let a = ViewId::new();
    model.execute(Command::Open {
        id: a,
        view: View {
            kind: "a".into(),
            restore_key: None,
        },
        destination: Destination::EmptyHost(host_id),
    })?;
    let original = model.snapshot().clone();
    let group = original.location(a).ok_or("group missing")?.0;
    let host = world.spawn(Node::default()).id();
    let stash = world.spawn(Node::default()).id();
    let content = world.spawn(Node::default()).id();
    let field = world
        .spawn((EditableText::new("Kept 日本語"), ChildOf(content)))
        .id();
    let mut projection = Projection::default();
    projection.register(a, content)?;
    let hosts = BTreeMap::from([(host_id, host)]);
    projection.sync(
        world,
        &original,
        &hosts,
        stash,
        &skin,
        &BTreeMap::new(),
        "test",
        1.,
    )?;
    world
        .resource_mut::<InputFocus>()
        .set(field, FocusCause::Pressed);
    let old_group = projection.group(group);
    let old_tab = projection.tab(a);
    let b = ViewId::new();
    model.execute(Command::Open {
        id: b,
        view: View {
            kind: "b".into(),
            restore_key: None,
        },
        destination: Destination::Edge {
            target: group,
            edge: Edge::Right,
            fraction: 0.5,
        },
    })?;
    let added_group = model.snapshot().location(b).ok_or("new group missing")?.0;
    model.execute(Command::Maximize {
        host: host_id,
        group: Some(added_group),
    })?;
    // A valid snapshot may still violate this projection's stable chrome-kind
    // contract. Swap group/split identities while retaining a valid topology.
    let mut changed_kind = model.snapshot().clone();
    let split_id = changed_kind.hosts[&host_id].root.ok_or("split root")?;
    let group_node = changed_kind.nodes.remove(&group).ok_or("old group")?;
    let split_node = changed_kind.nodes.remove(&split_id).ok_or("new split")?;
    changed_kind.nodes.insert(group, split_node);
    changed_kind.nodes.insert(split_id, group_node);
    if let Some(kedra_docking::model::Node::Split { first, second, .. }) =
        changed_kind.nodes.get_mut(&group)
    {
        for child in [first, second] {
            if *child == group {
                *child = split_id;
            }
        }
    }
    changed_kind.hosts.get_mut(&host_id).ok_or("host")?.root = Some(group);
    changed_kind.validate()?;
    for (candidate, group_skin, tab_skin, split_skin) in [
        (model.snapshot(), &bad_group, &skin.tab, &skin.split),
        (model.snapshot(), &skin.group, &bad_tab, &skin.split),
        (model.snapshot(), &skin.group, &skin.tab, &bad_split),
        (model.snapshot(), &non_ui_group, &skin.tab, &skin.split),
        (model.snapshot(), &skin.group, &unavailable_tab, &skin.split),
        (&changed_kind, &skin.group, &skin.tab, &skin.split),
    ] {
        let broken = Skin {
            group: group_skin.clone(),
            tab: tab_skin.clone(),
            split: split_skin.clone(),
        };
        for _ in 0..2 {
            let count = world.entities().count_spawned();
            let parents: BTreeMap<_, _> = world
                .query::<(Entity, &ChildOf)>()
                .iter(world)
                .map(|(e, p)| (e, p.parent()))
                .collect();
            let children: BTreeMap<_, _> = world
                .query::<(Entity, &Children)>()
                .iter(world)
                .map(|(e, c)| (e, c.to_vec()))
                .collect();
            let focus = world.resource::<InputFocus>().clone();
            assert!(
                projection
                    .sync(
                        world,
                        candidate,
                        &hosts,
                        stash,
                        &broken,
                        &BTreeMap::new(),
                        "test",
                        1.
                    )
                    .is_err()
            );
            assert_eq!(
                world.entities().count_spawned(),
                count,
                "Rejected sync leaked mounts"
            );
            assert_eq!(
                world
                    .query::<(Entity, &ChildOf)>()
                    .iter(world)
                    .map(|(e, p)| (e, p.parent()))
                    .collect::<BTreeMap<_, _>>(),
                parents
            );
            assert_eq!(
                world
                    .query::<(Entity, &Children)>()
                    .iter(world)
                    .map(|(e, c)| (e, c.to_vec()))
                    .collect::<BTreeMap<_, _>>(),
                children
            );
            assert_eq!(world.resource::<InputFocus>(), &focus);
            assert_eq!(projection.group(group), old_group);
            assert_eq!(projection.tab(a), old_tab);
            assert!(projection.group(added_group).is_none());
        }
    }
    projection.sync(
        world,
        model.snapshot(),
        &hosts,
        stash,
        &skin,
        &BTreeMap::new(),
        "test",
        1.,
    )?;
    assert_eq!(projection.group(group), old_group);
    assert_eq!(projection.tab(a), old_tab);
    assert!(projection.group(added_group).is_some());
    assert_eq!(
        world
            .get::<EditableText>(field)
            .ok_or("retired field")?
            .value()
            .to_string(),
        "Kept 日本語"
    );
    assert!(
        world.resource::<InputFocus>().get().is_none(),
        "Successful maximize hides prior focus"
    );
    Ok(())
}
#[test]
fn real_editable_components_survive_reorder_split_float_redock_hide_and_chrome_retirement()
-> Result<()> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_resource::<AppTypeRegistry>()
        .add_plugins(AuthoredUiPlugin);
    let skin = Skin {
        group: compile(&app, &["tabs", "body", "menu"])?,
        tab: compile(&app, &["label", "title", "close"])?,
        split: compile(&app, &["divider"])?,
    };
    let world = app.world_mut();
    world.init_resource::<bevy::input_focus::InputFocus>();
    let main = HostId::new();
    let float = HostId::new();
    let mut model = Workspace::new(main);
    let host = world.spawn(Node::default()).id();
    let float_root = world.spawn(Node::default()).id();
    let stash = world.spawn(Node::default()).id();
    let mut roots = BTreeMap::from([(main, host)]);
    let mut projection = Projection::default();
    let a = ViewId::new();
    let b = ViewId::new();
    model.execute(Command::Open {
        id: a,
        view: View {
            kind: "a".into(),
            restore_key: None,
        },
        destination: Destination::EmptyHost(main),
    })?;
    let group = model.snapshot().location(a).ok_or("missing group")?.0;
    model.execute(Command::Open {
        id: b,
        view: View {
            kind: "b".into(),
            restore_key: None,
        },
        destination: Destination::Tabs { group, index: 1 },
    })?;
    let content = world.spawn(Node::default()).id();
    let field = world
        .spawn((
            Node::default(),
            EditableText::new("Кедра 日本語"),
            ChildOf(content),
        ))
        .id();
    projection.register(a, content)?;
    let other = world.spawn(Node::default()).id();
    projection.register(b, other)?;
    assert!(projection.register(a, other).is_err());
    let titles = BTreeMap::new();
    projection.sync(
        world,
        model.snapshot(),
        &roots,
        stash,
        &skin,
        &titles,
        "independent",
        1.,
    )?;
    let commands = [
        Command::Activate(a),
        Command::Move {
            selection: Selection::View(a),
            destination: Destination::Tabs { group, index: 2 },
        },
        Command::Move {
            selection: Selection::View(a),
            destination: Destination::Edge {
                target: group,
                edge: Edge::Right,
                fraction: 0.5,
            },
        },
        Command::Maximize {
            host: main,
            group: Some(group),
        },
        Command::Maximize {
            host: main,
            group: None,
        },
        Command::Float {
            selection: Selection::View(a),
            host: float,
        },
        Command::Move {
            selection: Selection::View(a),
            destination: Destination::Tabs { group, index: 0 },
        },
        Command::RetireHost(float),
        Command::Hide(a),
        Command::Show {
            view: a,
            destination: Destination::Tabs { group, index: 1 },
        },
    ];
    for command in commands {
        let activates = matches!(&command,Command::Activate(view) if *view==a);
        let hides = matches!(&command,Command::Hide(view) if *view==a);
        model.execute(command)?;
        if model.snapshot().hosts.contains_key(&float) {
            roots.insert(float, float_root);
        } else {
            roots.remove(&float);
        }
        projection.sync(
            world,
            model.snapshot(),
            &roots,
            stash,
            &skin,
            &titles,
            "independent",
            1.,
        )?;
        if activates {
            world
                .resource_mut::<bevy::input_focus::InputFocus>()
                .set(field, bevy::input_focus::FocusCause::Pressed);
        }
        if hides {
            assert!(
                world
                    .resource::<bevy::input_focus::InputFocus>()
                    .get()
                    .is_none()
            );
        }
        if model
            .snapshot()
            .hosts
            .values()
            .any(|host| host.maximized.is_some())
        {
            for node in model.snapshot().nodes.values() {
                if let kedra_docking::model::Node::Split { first, second, .. } = node {
                    for id in [first, second] {
                        let root = projection.group(*id).ok_or("Missing projected ancestor")?;
                        let mut style = world
                            .get::<bevy_flair::prelude::InlineStyle>(root)
                            .ok_or("Missing native geometry")?
                            .clone();
                        assert_eq!(
                            style.get("flex-grow"),
                            Some("1"),
                            "Maximized ancestor must consume all remaining flex space"
                        );
                    }
                }
            }
        }
        assert_eq!(projection.content(a), Some(content));
        assert_eq!(
            world
                .get::<EditableText>(field)
                .ok_or("field was destroyed")?
                .value()
                .to_string(),
            "Кедра 日本語"
        );
        assert!(world.entities().contains(other));
        for entity in world
            .query_filtered::<Entity, With<kedra_ui::docking::Divider>>()
            .iter(world)
        {
            assert!(
                world.get::<bevy::picking::hover::Hovered>(entity).is_some(),
                "Native splitter hover must be observed for its authored :hover state"
            );
        }
    }
    assert_eq!(
        world.resource::<bevy::input_focus::InputFocus>().get(),
        Some(field)
    );
    Ok(())
}
