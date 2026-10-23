//! Source projection and live admission errors preserve immutable snapshots and worlds.
use bevy::{asset::AssetPlugin, prelude::*};
use kedra_scene::{
    composition::Catalog, document::*, instance::CompiledPrefab, reflection::ComponentPolicies,
};
use std::{collections::BTreeMap, sync::Arc};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
struct Settings {
    value: u32,
    #[entities]
    target: Option<Entity>,
}
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
struct Other;

fn setup() -> Result<(App, ComponentPolicies, PrefabDocument)> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    let types = app.world().resource::<AppTypeRegistry>();
    let mut policies = ComponentPolicies::default();
    policies.register::<Settings>(&mut types.write(), "settings", 1, |_| Ok(()))?;
    policies.register::<Other>(&mut types.write(), "other", 1, |_| Ok(()))?;
    let mut source = PrefabDocument::new("Settings");
    source.ports.insert("owner".into(), false);
    source.nodes[0].components.insert(
        "settings".into(),
        ComponentData::ron(1, "(value:1,target:Some(Port(\"owner\")))")?,
    );
    source.overrides.push(Operation::SetField {
        node: source.root.into(),
        component: "settings".into(),
        version: 1,
        path: "value".into(),
        value: ComponentData::ron(1, "2")?.value,
    });
    Ok((app, policies, source))
}

#[test]
fn authored_projection_keeps_symbolic_ports_and_reports_missing_targets() -> Result<()> {
    let (app, policies, source) = setup()?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let mut catalog = Catalog::default();
    catalog.insert(source.clone())?;
    let snapshot = CompiledPrefab::compile(
        catalog.resolve(source.id)?,
        &types.read(),
        &policies,
        &mut loader,
        &BTreeMap::new(),
    )?;
    let entity = snapshot.source_entities()[&source.root.into()];
    assert_eq!(
        snapshot
            .authoring_world()
            .get::<Settings>(entity)
            .ok_or("settings missing")?
            .target,
        None
    );
    let (value, references) = snapshot
        .authored_component(
            &source.root.into(),
            "settings",
            &policies,
            &types.read(),
            &mut loader,
        )?
        .ok_or("local component not recovered")?;
    let concrete = Settings::from_reflect(value.as_ref()).ok_or("projection type missing")?;
    assert_eq!(concrete.value, 2);
    assert!(concrete.target.is_some());
    let encoded = policies.encode("settings", value.as_ref(), &types.read(), &references)?;
    assert!(encoded.value.get_ron().contains("Some(Port(\"owner\"))"));
    assert_eq!(
        snapshot.references().len(),
        1,
        "absent port is not a runtime entity"
    );
    let symbolic = snapshot.symbolic_source(&types.read(), &policies)?;
    assert!(
        symbolic
            .field_value(
                &source.root.into(),
                "settings",
                "target",
                &[],
                &policies,
                &types.read()
            )?
            .get_ron()
            .contains("Port(\"owner\")")
    );
    assert!(
        symbolic
            .encode_value(
                &Settings {
                    value: 3,
                    target: Some(entity)
                },
                &policies,
                &types.read()
            )?
            .get_ron()
            .contains("Local(")
    );

    for (node, key, path) in [
        (NodeId::new().into(), "settings", "value"),
        (source.root.into(), "missing", "value"),
        (source.root.into(), "other", "value"),
        (source.root.into(), "settings", "missing"),
    ] {
        assert!(
            snapshot
                .field_value(&node, key, path, &[], &policies, &types.read())
                .is_err()
        );
        if path != "missing" {
            assert!(
                snapshot
                    .component(&node, key, &policies, &types.read(), &BTreeMap::new())
                    .is_err()
            );
            assert!(
                snapshot
                    .authored_component(&node, key, &policies, &types.read(), &mut loader)
                    .is_err()
            );
        }
    }
    let mut parent = PrefabDocument::new("Parent");
    let site = InstanceId::new();
    let alias = NodeId::new();
    parent
        .overrides
        .push(Operation::RemapReferences(BTreeMap::from([(
            alias.into(),
            parent.root.into(),
        )])));
    parent.instances.push(NestedInstance {
        id: site,
        asset: source.id,
        parent: parent.root.into(),
        bindings: BTreeMap::from([("owner".into(), Reference::Local(alias.into()))]),
        overrides: vec![],
    });
    catalog.insert(parent.clone())?;
    let nested = CompiledPrefab::compile(
        catalog.resolve(parent.id)?,
        &types.read(),
        &policies,
        &mut loader,
        &BTreeMap::new(),
    )?;
    let scoped = NodeAddress {
        instances: vec![site],
        node: source.root,
    };
    assert!(
        nested
            .field_value(
                &scoped,
                "settings",
                "target",
                &[site],
                &policies,
                &types.read()
            )?
            .get_ron()
            .contains("Port(\"owner\")")
    );
    assert!(
        nested
            .authored_component(
                &NodeAddress {
                    instances: vec![site],
                    node: source.root
                },
                "settings",
                &policies,
                &types.read(),
                &mut loader
            )?
            .is_none()
    );
    Ok(())
}

#[test]
fn live_parent_registry_and_port_presence_failures_allocate_nothing() -> Result<()> {
    let (mut app, policies, source) = setup()?;
    let types = app.world().resource::<AppTypeRegistry>().clone();
    let mut catalog = Catalog::default();
    catalog.insert(source.clone())?;
    let snapshot = Arc::new(CompiledPrefab::compile(
        catalog.resolve(source.id)?,
        &types.read(),
        &policies,
        &mut app.world().resource::<AssetServer>(),
        &BTreeMap::new(),
    )?);
    let stale = app.world_mut().spawn_empty().id();
    app.world_mut().despawn(stale);
    let external = app.world_mut().spawn_empty().id();
    for (parent, bindings) in [
        (Some(stale), BTreeMap::new()),
        (None, BTreeMap::from([("unknown".into(), None)])),
        (None, BTreeMap::from([("owner".into(), Some(external))])),
    ] {
        let before = app.world().entities().count_spawned();
        assert!(
            snapshot
                .instantiate(app.world_mut(), parent, &bindings)
                .is_err()
        );
        assert_eq!(app.world().entities().count_spawned(), before);
        assert!(app.world().entities().contains(external));
    }
    let mut empty = World::new();
    let before = empty.entities().count_spawned();
    assert!(
        snapshot
            .instantiate(&mut empty, None, &BTreeMap::new())
            .is_err()
    );
    assert_eq!(empty.entities().count_spawned(), before);
    assert!(
        CompiledPrefab::compile(
            catalog.resolve(source.id)?,
            &types.read(),
            &policies,
            &mut app.world().resource::<AssetServer>(),
            &BTreeMap::from([("unknown".into(), true)])
        )
        .is_err()
    );
    catalog
        .documents
        .get_mut(&source.id)
        .ok_or("source missing")?
        .ports
        .insert("owner".into(), true);
    assert!(
        CompiledPrefab::compile(
            catalog.resolve(source.id)?,
            &types.read(),
            &policies,
            &mut app.world().resource::<AssetServer>(),
            &BTreeMap::from([("owner".into(), false)])
        )
        .is_err()
    );
    Ok(())
}

#[derive(Asset, Reflect)]
struct TestAsset;
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
struct AssetReference {
    asset: Handle<TestAsset>,
}

#[test]
fn source_recompile_rejects_untracked_asset_dependencies_and_bad_field_schemas() -> Result<()> {
    use bevy::asset::AssetApp;
    let (mut app, mut policies, source) = setup()?;
    app.init_asset::<TestAsset>()
        .register_asset_reflect::<TestAsset>();
    let types = app.world().resource::<AppTypeRegistry>();
    policies.register::<AssetReference>(&mut types.write(), "asset", 1, |_| Ok(()))?;
    let mut loader = app.world().resource::<AssetServer>();
    let mut catalog = Catalog::default();
    catalog.insert(source.clone())?;
    let snapshot = CompiledPrefab::compile(
        catalog.resolve(source.id)?,
        &types.read(),
        &policies,
        &mut loader,
        &BTreeMap::new(),
    )?;
    let mut candidate = source.clone();
    // Let the native serializer provide the exact current Handle source syntax.
    let asset = AssetReference {
        asset: app.world().resource::<AssetServer>().load("new.test"),
    };
    candidate.nodes[0].components.insert(
        "asset".into(),
        policies.encode("asset", &asset, &types.read(), &BTreeMap::new())?,
    );
    catalog.documents.insert(candidate.id, candidate);
    assert!(
        snapshot
            .recompile(catalog.resolve(source.id)?, &types.read(), &policies)
            .is_err()
    );
    for (path, version, value) in [
        ("missing", 1, "2"),
        ("value", 2, "2"),
        ("value", 1, "\"wrong type\""),
    ] {
        let mut candidate = source.clone();
        candidate.overrides = vec![Operation::SetField {
            node: source.root.into(),
            component: "settings".into(),
            version,
            path: path.into(),
            value: ComponentData::ron(1, value)?.value,
        }];
        catalog.documents.insert(candidate.id, candidate);
        assert!(
            snapshot
                .recompile(catalog.resolve(source.id)?, &types.read(), &policies)
                .is_err()
        );
    }
    assert_eq!(snapshot.graph().document, source.id);
    assert_eq!(
        snapshot
            .authoring_world()
            .get::<Settings>(snapshot.source_entities()[&source.root.into()])
            .ok_or("settings missing")?
            .value,
        2
    );
    Ok(())
}
