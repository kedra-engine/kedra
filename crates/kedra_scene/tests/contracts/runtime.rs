//! Independent game-style components exercise the same runtime used by authored UI.
use bevy::{
    asset::{AssetApp, AssetPlugin},
    prelude::*,
};
use kedra_scene::{
    assets::{PrefabSources, compile_catalog},
    composition::Catalog,
    document::{
        ComponentData, DocumentKind, InstanceId, NestedInstance, Node as SourceNode, NodeAddress,
        NodeId, Operation, PrefabDocument,
    },
    instance::{CompiledPrefab, PrefabInstance},
    reflection::{ComponentPolicies, ReferenceContext},
};
use std::{collections::BTreeMap, sync::Arc};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Component, Clone, Reflect, Debug, PartialEq)]
#[reflect(Component)]
struct Tuning {
    speed: f32,
    health: u32,
}
#[derive(Component, Clone, Reflect, Debug)]
#[reflect(Component)]
struct Links {
    #[entities]
    target: Entity,
    #[entities]
    optional: Option<Entity>,
}
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
struct BrokenLink {
    target: Entity,
}
#[derive(Asset, Reflect, Clone)]
struct TestAsset;
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
struct AssetLink {
    asset: Handle<TestAsset>,
}

fn setup() -> Result<(App, ComponentPolicies)> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_resource::<AppTypeRegistry>();
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let mut policies = ComponentPolicies::default();
    policies.register::<Tuning>(&mut registry.write(), "game.tuning", 1, |value| {
        let tuning = Tuning::from_reflect(value)
            .ok_or_else(|| kedra_scene::Error("invalid tuning".into()))?;
        if !tuning.speed.is_finite() || tuning.speed < 0. {
            return Err(kedra_scene::Error(
                "speed must be finite and nonnegative".into(),
            ));
        }
        Ok(())
    })?;
    policies.register::<Links>(&mut registry.write(), "game.links", 1, |_| Ok(()))?;
    Ok((app, policies))
}

fn sample() -> Result<PrefabDocument> {
    let mut source = PrefabDocument::new("Independent game actor");
    source.nodes[0].components.insert(
        "game.tuning".into(),
        ComponentData::ron(1, "(speed:1.0,health:10)")?,
    );
    Ok(source)
}

fn compile(
    app: &App,
    policies: &ComponentPolicies,
    catalog: &Catalog,
    source: &PrefabDocument,
) -> Result<Arc<CompiledPrefab>> {
    let mut loader = app.world().resource::<AssetServer>();
    let registry = app.world().resource::<AppTypeRegistry>();
    Ok(Arc::new(CompiledPrefab::compile(
        catalog.resolve(source.id)?,
        &registry.read(),
        policies,
        &mut loader,
        &BTreeMap::new(),
    )?))
}

#[test]
fn source_roundtrip_variants_nested_links_and_independent_owned_instances() -> Result<()> {
    let (mut app, policies) = setup()?;
    let mut base = sample()?;
    let child = NodeId::new();
    base.nodes.push(SourceNode {
        id: child,
        name: "Nested child".into(),
        parent: Some(base.root.into()),
        components: BTreeMap::new(),
    });
    base.nodes[0].components.insert(
        "game.links".into(),
        ComponentData::ron(
            1,
            &format!(
                "(target:Local((instances:[],node:{:?})),optional:None)",
                child.to_string()
            ),
        )?,
    );
    let mut variant = base.variant("Faster actor");
    variant.overrides.push(Operation::SetField {
        node: base.root.into(),
        component: "game.tuning".into(),
        version: 1,
        path: "speed".into(),
        value: ron::value::RawValue::from_ron("3.0")?.to_owned(),
    });
    let mut scene = PrefabDocument::new("Game scene");
    scene.kind = DocumentKind::Scene;
    let site = InstanceId::new();
    scene.instances.push(NestedInstance {
        id: site,
        asset: variant.id,
        parent: scene.root.into(),
        bindings: BTreeMap::new(),
        overrides: Vec::new(),
    });
    assert_eq!(PrefabDocument::parse(&scene.serialize()?)?, scene);
    let mut catalog = Catalog::default();
    for document in [&base, &variant, &scene] {
        catalog.insert(document.clone())?;
    }
    let snapshot = compile(&app, &policies, &catalog, &scene)?;
    let parent = app.world_mut().spawn_empty().id();
    let external = app.world_mut().spawn_empty().id();
    let first = snapshot.instantiate(app.world_mut(), Some(parent), &BTreeMap::new())?;
    let second = snapshot.instantiate(app.world_mut(), Some(parent), &BTreeMap::new())?;
    let actor = NodeAddress {
        instances: vec![site],
        node: base.root,
    };
    let first_actor = app
        .world()
        .get::<PrefabInstance>(first)
        .ok_or("first instance missing")?
        .nodes[&actor];
    let second_actor = app
        .world()
        .get::<PrefabInstance>(second)
        .ok_or("second instance missing")?
        .nodes[&actor];
    assert_ne!(first_actor, second_actor);
    assert_eq!(
        app.world()
            .get::<Tuning>(first_actor)
            .ok_or("tuning missing")?
            .speed,
        3.
    );
    let target = app
        .world()
        .get::<Links>(first_actor)
        .ok_or("link missing")?
        .target;
    assert_eq!(
        app.world()
            .get::<ChildOf>(target)
            .ok_or("child parent missing")?
            .parent(),
        first_actor
    );
    app.world_mut()
        .get_mut::<Tuning>(first_actor)
        .ok_or("tuning missing")?
        .health = 1;
    assert_eq!(
        app.world()
            .get::<Tuning>(second_actor)
            .ok_or("tuning missing")?
            .health,
        10
    );
    app.world_mut().despawn(first);
    assert!(!app.world().entities().contains(first_actor));
    assert!(app.world().entities().contains(second_actor));
    assert!(app.world().entities().contains(parent));
    assert!(app.world().entities().contains(external));
    Ok(())
}

#[test]
fn invalid_graph_schema_field_mapping_and_values_fail_without_live_mutation() -> Result<()> {
    let (app, mut policies) = setup()?;
    let base = sample()?;
    let mut catalog = Catalog::default();
    catalog.insert(base.clone())?;
    assert!(catalog.insert(base.clone()).is_err());
    let before = app.world().entities().count_spawned();
    for value in ["(speed:-1.0,health:10)", "(speed:NaN,health:10)"] {
        let mut invalid = base.clone();
        invalid.nodes[0]
            .components
            .insert("game.tuning".into(), ComponentData::ron(1, value)?);
        catalog.documents.insert(invalid.id, invalid.clone());
        assert!(compile(&app, &policies, &catalog, &invalid).is_err());
    }
    let mut unknown = base.clone();
    unknown.nodes[0]
        .components
        .insert("unknown".into(), ComponentData::ron(1, "()")?);
    catalog.documents.insert(unknown.id, unknown.clone());
    assert!(compile(&app, &policies, &catalog, &unknown).is_err());
    let mut wrong = base.clone();
    wrong.nodes[0]
        .components
        .get_mut("game.tuning")
        .ok_or("source missing")?
        .version = 2;
    catalog.documents.insert(wrong.id, wrong.clone());
    assert!(compile(&app, &policies, &catalog, &wrong).is_err());
    let mut cycle = base.clone();
    cycle.base = Some(cycle.id);
    catalog.documents.insert(cycle.id, cycle.clone());
    assert!(catalog.resolve(cycle.id).is_err());
    let mut duplicate = base.clone();
    duplicate.nodes.push(duplicate.nodes[0].clone());
    catalog.documents.insert(duplicate.id, duplicate.clone());
    assert!(catalog.resolve(duplicate.id).is_err());
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    policies.register::<BrokenLink>(&mut registry.write(), "broken", 1, |_| Ok(()))?;
    let mut broken = base.clone();
    broken.nodes[0].components.insert(
        "broken".into(),
        ComponentData::ron(
            1,
            &format!(
                "(target:Local((instances:[],node:{:?})))",
                broken.root.to_string()
            ),
        )?,
    );
    catalog.documents.insert(broken.id, broken.clone());
    assert!(compile(&app, &policies, &catalog, &broken).is_err());
    assert_eq!(app.world().entities().count_spawned(), before);
    Ok(())
}

#[test]
fn ports_are_explicit_and_stale_bindings_allocate_nothing() -> Result<()> {
    let (mut app, policies) = setup()?;
    let mut source = sample()?;
    source.ports.insert("owner".into(), true);
    source.ports.insert("optional".into(), false);
    source.nodes[0].components.insert(
        "game.links".into(),
        ComponentData::ron(
            1,
            "(target:Port(\"owner\"),optional:Some(Port(\"optional\")))",
        )?,
    );
    let mut catalog = Catalog::default();
    catalog.insert(source.clone())?;
    let snapshot = compile(&app, &policies, &catalog, &source)?;
    let external = app.world_mut().spawn_empty().id();
    let before = app.world().entities().count_spawned();
    assert!(
        snapshot
            .instantiate(app.world_mut(), None, &BTreeMap::new())
            .is_err()
    );
    assert_eq!(app.world().entities().count_spawned(), before);
    let root = snapshot.instantiate(
        app.world_mut(),
        None,
        &BTreeMap::from([("owner".into(), Some(external))]),
    )?;
    let links = app.world().get::<Links>(root).ok_or("links missing")?;
    assert_eq!(links.target, external);
    assert_eq!(links.optional, None);
    app.world_mut().despawn(external);
    let before = app.world().entities().count_spawned();
    assert!(
        snapshot
            .instantiate(
                app.world_mut(),
                None,
                &BTreeMap::from([("owner".into(), Some(external))])
            )
            .is_err()
    );
    assert_eq!(app.world().entities().count_spawned(), before);
    Ok(())
}

#[test]
fn native_handle_processors_preserve_labels_and_reject_ephemeral_assets() -> Result<()> {
    let (mut app, mut policies) = setup()?;
    app.init_asset::<TestAsset>()
        .register_asset_reflect::<TestAsset>();
    let types = app.world().resource::<AppTypeRegistry>().clone();
    policies.register::<AssetLink>(&mut types.write(), "game.asset", 1, |_| Ok(()))?;
    let handle = app
        .world()
        .resource::<AssetServer>()
        .load("models/model.test#Mesh0");
    let data = policies.encode(
        "game.asset",
        &AssetLink { asset: handle },
        &types.read(),
        &BTreeMap::new(),
    )?;
    assert!(data.value.get_ron().contains("model.test#Mesh0"));
    let mut loader = app.world().resource::<AssetServer>();
    let mut context = ReferenceContext {
        entities: &BTreeMap::new(),
        ports: &BTreeMap::new(),
        prefix: &[],
        loader: &mut loader,
    };
    let decoded = policies.decode("game.asset", &data, &types.read(), &mut context)?;
    let asset = AssetLink::from_reflect(decoded.as_ref()).ok_or("asset decode failed")?;
    assert_eq!(
        asset.asset.path().ok_or("path lost")?.to_string(),
        "models/model.test#Mesh0"
    );
    let ephemeral = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset);
    assert!(
        policies
            .encode(
                "game.asset",
                &AssetLink { asset: ephemeral },
                &types.read(),
                &BTreeMap::new()
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn pinned_revisions_and_instances_own_their_exact_compilation() -> Result<()> {
    let (mut app, policies) = setup()?;
    let source = sample()?;
    let mut catalog = Catalog::default();
    catalog.insert(source.clone())?;
    let types = app.world().resource::<AppTypeRegistry>().clone();
    let compiled = compile_catalog(
        &catalog,
        &types.read(),
        &policies,
        &mut app.world().resource::<AssetServer>(),
    )?;
    let mut sources = PrefabSources::default();
    sources.publish(catalog.clone(), compiled)?;
    assert!(sources.ready());
    sources.pin()?;
    assert!(sources.pin().is_err());
    let pinned = sources.snapshot();
    let root = pinned.compiled[&source.id].instantiate(app.world_mut(), None, &BTreeMap::new())?;
    let old = Arc::downgrade(&pinned.compiled[&source.id]);
    let mut next = source.clone();
    next.nodes[0].components.insert(
        "game.tuning".into(),
        ComponentData::ron(1, "(speed:8.0,health:20)")?,
    );
    catalog.documents.insert(next.id, next);
    let compiled = compile_catalog(
        &catalog,
        &types.read(),
        &policies,
        &mut app.world().resource::<AssetServer>(),
    )?;
    sources.publish(catalog, compiled)?;
    assert_eq!(sources.snapshot().revision, 1);
    assert_eq!(sources.published().revision, 2);
    sources.unpin();
    drop(pinned);
    assert!(old.upgrade().is_some());
    assert_eq!(
        app.world()
            .get::<Tuning>(root)
            .ok_or("tuning missing")?
            .health,
        10
    );
    app.world_mut().despawn(root);
    assert!(old.upgrade().is_none());
    Ok(())
}

#[test]
fn partial_dynamic_world_write_reclaims_only_owned_allocations() -> Result<()> {
    let (mut app, policies) = setup()?;
    let source = sample()?;
    let mut catalog = Catalog::default();
    catalog.insert(source.clone())?;
    let snapshot = compile(&app, &policies, &catalog, &source)?;
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let mut target = bevy::reflect::TypeRegistry::default();
    target.register::<Name>();
    target.register::<ChildOf>();
    target.register::<Children>();
    *registry.write() = target;
    let parent = app.world_mut().spawn_empty().id();
    let before = app.world().entities().count_spawned();
    assert!(
        snapshot
            .instantiate(app.world_mut(), Some(parent), &BTreeMap::new())
            .is_err()
    );
    assert_eq!(app.world().entities().count_spawned(), before);
    assert!(app.world().entities().contains(parent));
    Ok(())
}

#[test]
fn source_limits_versions_and_nil_ids_are_not_compatibility_paths() -> Result<()> {
    let source = sample()?;
    let mut invalid = source.clone();
    invalid.version = 2;
    assert!(PrefabDocument::parse(&invalid.serialize()?).is_err());
    invalid = source;
    invalid.id.0 = uuid::Uuid::nil();
    assert!(PrefabDocument::parse(&invalid.serialize()?).is_err());
    assert!(PrefabDocument::parse(&" ".repeat(65_537)).is_err());
    assert!(ComponentData::ron(1, &" ".repeat(65_537)).is_err());
    Ok(())
}

#[test]
fn failed_publication_keeps_exact_snapshot_and_exposes_rejection() -> Result<()> {
    let (app, policies) = setup()?;
    let source = sample()?;
    let mut catalog = Catalog::default();
    catalog.insert(source.clone())?;
    let types = app.world().resource::<AppTypeRegistry>();
    let mut loader = app.world().resource::<AssetServer>();
    let compiled = compile_catalog(&catalog, &types.read(), &policies, &mut loader)?;
    let mut sources = PrefabSources::default();
    sources.publish(catalog.clone(), compiled.clone())?;
    let snapshot = sources.published().clone();
    let mut changed = source;
    changed.nodes[0].name = "Changed source".into();
    catalog.documents.insert(changed.id, changed);
    assert!(sources.publish(catalog.clone(), compiled).is_err());
    assert!(Arc::ptr_eq(sources.published(), &snapshot));
    assert!(!sources.ready());
    assert!(sources.error.is_some());
    let valid = compile_catalog(&catalog, &types.read(), &policies, &mut loader)?;
    sources.publish(catalog, valid)?;
    assert!(sources.ready());
    assert_eq!(sources.published().revision, 2);
    Ok(())
}

#[test]
fn nil_nesting_and_excessive_depth_or_nodes_are_not_valid_scopes() -> Result<()> {
    let base = sample()?;
    let mut catalog = Catalog::default();
    catalog.insert(base.clone())?;
    let mut outer = PrefabDocument::new("Outer");
    outer.instances.push(NestedInstance {
        id: InstanceId(uuid::Uuid::nil()),
        asset: base.id,
        parent: outer.root.into(),
        bindings: BTreeMap::new(),
        overrides: Vec::new(),
    });
    catalog.insert(outer.clone())?;
    assert!(catalog.resolve(outer.id).is_err());
    catalog.documents.remove(&outer.id);
    let mut deepest = base;
    for _ in 0..64 {
        let next = deepest.variant("Deep variant");
        catalog.insert(next.clone())?;
        deepest = next;
    }
    assert!(catalog.resolve(deepest.id).is_err());
    let mut oversized = PrefabDocument::new("Oversized");
    for _ in 0..4096 {
        oversized.nodes.push(SourceNode {
            id: NodeId::new(),
            name: "Child".into(),
            parent: Some(oversized.root.into()),
            components: BTreeMap::new(),
        });
    }
    let mut catalog = Catalog::default();
    catalog.insert(oversized.clone())?;
    assert!(catalog.resolve(oversized.id).is_err());
    Ok(())
}

#[derive(Component, Clone, Reflect, Default)]
#[reflect(Component)]
#[component(on_add=side_effect)]
struct Hooked;
fn side_effect(mut world: bevy::ecs::world::DeferredWorld, _: bevy::ecs::lifecycle::HookContext) {
    world.commands().spawn_empty();
}
#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
#[require(Hooked)]
struct RequiresHook;

#[test]
fn lifecycle_hooks_and_unregistered_requirements_cannot_execute_in_source_worlds() -> Result<()> {
    let (app, mut policies) = setup()?;
    let registry = app.world().resource::<AppTypeRegistry>();
    assert!(
        policies
            .register::<Hooked>(&mut registry.write(), "hooked", 1, |_| Ok(()))
            .is_err()
    );
    assert!(
        policies
            .register::<RequiresHook>(&mut registry.write(), "requires_hook", 1, |_| Ok(()))
            .is_err()
    );
    assert!(policies.policy("hooked", 1).is_err());
    assert!(policies.policy("requires_hook", 1).is_err());
    Ok(())
}

#[test]
fn duplicate_source_map_keys_are_errors_not_last_value_wins() -> Result<()> {
    let source = sample()?;
    let encoded = source.serialize()?;
    let duplicate = encoded.replace("ports: {}", "ports: {\"same\": true, \"same\": false}");
    assert_ne!(duplicate, encoded, "fixture must exercise duplicate keys");
    assert!(PrefabDocument::parse(&duplicate).is_err());
    Ok(())
}
