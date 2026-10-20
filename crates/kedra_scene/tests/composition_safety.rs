//! Reject invalid hierarchy/bindings independently of whether components use them.
use bevy::{asset::AssetPlugin, prelude::*};
use kedra_scene::{
    assets::compile_catalog, composition::Catalog, document::*, reflection::ComponentPolicies,
};
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn alias_parents_are_rejected_instead_of_materializing_orphaned_nodes() -> Result<()> {
    for root_alias in [false, true] {
        let mut document = PrefabDocument::new("Hierarchy");
        let middle = NodeId::new();
        let alias = NodeId::new();
        document.nodes.push(kedra_scene::document::Node {
            id: middle,
            name: "Middle".into(),
            parent: Some(document.root.into()),
            components: BTreeMap::new(),
        });
        document
            .overrides
            .push(Operation::RemapReferences(BTreeMap::from([(
                alias.into(),
                if root_alias { document.root } else { middle }.into(),
            )])));
        document.nodes.push(kedra_scene::document::Node {
            id: NodeId::new(),
            name: "Child".into(),
            parent: Some(alias.into()),
            components: BTreeMap::new(),
        });
        let mut catalog = Catalog::default();
        catalog.insert(document.clone())?;
        assert!(
            catalog.resolve(document.id).is_err(),
            "alias parent admitted"
        );
    }
    Ok(())
}

#[test]
fn unused_nested_bindings_are_validated_before_component_decoding() -> Result<()> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    let registry = app.world().resource::<AppTypeRegistry>();
    let policies = ComponentPolicies::default();
    for required in [false, true] {
        for target in [
            Reference::Local(NodeId::new().into()),
            Reference::Port("missing".into()),
        ] {
            let mut child = PrefabDocument::new("Child");
            child.ports.insert("unused".into(), required);
            let mut parent = PrefabDocument::new("Parent");
            parent.instances.push(NestedInstance {
                id: InstanceId::new(),
                asset: child.id,
                parent: parent.root.into(),
                bindings: BTreeMap::from([("unused".into(), target)]),
                overrides: vec![],
            });
            let mut catalog = Catalog::default();
            catalog.insert(child)?;
            catalog.insert(parent)?;
            assert!(
                compile_catalog(
                    &catalog,
                    &registry.read(),
                    &policies,
                    &mut app.world().resource::<AssetServer>()
                )
                .is_err(),
                "unused bad binding admitted"
            );
        }
    }
    Ok(())
}

#[test]
fn aggregate_composition_work_is_bounded_for_direct_api_candidates() -> Result<()> {
    let mut document = PrefabDocument::new("Work budget");
    document.overrides = vec![
        Operation::RenameNode {
            node: document.root.into(),
            name: "Repeated".into(),
        };
        65_536
    ];
    let mut catalog = Catalog::default();
    catalog.insert(document.clone())?;
    let error = catalog
        .resolve(document.id)
        .err()
        .ok_or("unbounded work admitted")?;
    assert!(error.to_string().contains("work budget"));
    Ok(())
}

#[test]
fn valid_unused_bindings_alias_targets_and_owned_hierarchy_remain_supported() -> Result<()> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    registry.write().register::<Name>();
    registry.write().register::<ChildOf>();
    registry.write().register::<Children>();
    let mut child = PrefabDocument::new("Child");
    child.ports.insert("unused".into(), false);
    let mut parent = PrefabDocument::new("Parent");
    parent.ports.insert("forward".into(), false);
    let alias = NodeId::new();
    parent
        .overrides
        .push(Operation::RemapReferences(BTreeMap::from([(
            alias.into(),
            parent.root.into(),
        )])));
    for binding in [
        Some(Reference::Local(alias.into())),
        Some(Reference::Port("forward".into())),
        None,
    ] {
        parent.instances.push(NestedInstance {
            id: InstanceId::new(),
            asset: child.id,
            parent: parent.root.into(),
            bindings: binding
                .into_iter()
                .map(|target| ("unused".into(), target))
                .collect(),
            overrides: vec![],
        });
    }
    let mut catalog = Catalog::default();
    catalog.insert(child)?;
    catalog.insert(parent.clone())?;
    let compiled = compile_catalog(
        &catalog,
        &registry.read(),
        &ComponentPolicies::default(),
        &mut app.world().resource::<AssetServer>(),
    )?;
    let root = compiled[&parent.id].instantiate(app.world_mut(), None, &BTreeMap::new())?;
    let nodes = app
        .world()
        .get::<kedra_scene::instance::PrefabInstance>(root)
        .ok_or("instance missing")?
        .nodes
        .clone();
    assert_eq!(nodes.len(), 4);
    for entity in nodes.values().filter(|entity| **entity != root) {
        assert_eq!(
            app.world()
                .get::<ChildOf>(*entity)
                .ok_or("unowned node")?
                .parent(),
            root
        );
    }
    app.world_mut().despawn(root);
    assert!(
        nodes
            .values()
            .all(|entity| !app.world().entities().contains(*entity))
    );

    let mut graph = catalog.resolve(parent.id)?;
    let scope = vec![parent.instances[0].id];
    let binding = graph
        .bindings
        .get_mut(&(scope.clone(), "unused".into()))
        .ok_or("binding missing")?;
    *binding = Some(kedra_scene::composition::ScopedReference {
        document: parent.id,
        reference: Reference::Port("unused".into()),
        scope,
    });
    assert!(
        kedra_scene::instance::CompiledPrefab::compile(
            graph,
            &registry.read(),
            &ComponentPolicies::default(),
            &mut app.world().resource::<AssetServer>(),
            &BTreeMap::new()
        )
        .is_err(),
        "unused binding cycle admitted"
    );
    Ok(())
}
