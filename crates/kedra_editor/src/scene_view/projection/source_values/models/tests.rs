//! Static forest replacement owns only recorded imported descendants. Failure and
//! abandoned staging preserve serving nodes, camera and the frame epoch.
use super::*;
use bevy::{
    asset::{AssetPath, LoadFromPath, UntypedHandle},
    world_serialization::WorldAsset,
};
use kedra_scene::{
    composition::Catalog,
    document::{ComponentData, PrefabDocument},
    reflection::ComponentPolicies,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn staged_models_replace_only_owned_descendants_and_failure_preserves_the_serving_view()
-> TestResult {
    struct Loader(BTreeMap<String, Handle<WorldAsset>>);
    impl LoadFromPath for Loader {
        fn load_from_path_erased(
            &mut self,
            _: std::any::TypeId,
            path: AssetPath<'static>,
        ) -> UntypedHandle {
            self.0[&path.to_string()].clone().untyped()
        }
    }
    let mut world = World::new();
    crate::scene_view::test_support::compiled(&mut world)?;
    world.init_resource::<Assets<WorldAsset>>();
    let registry = world.resource::<AppTypeRegistry>().clone();
    registry.write().register::<Name>();
    registry.write().register::<ChildOf>();
    registry.write().register::<Children>();
    let mut policies = ComponentPolicies::default();
    kedra_scene::spatial::register(&mut registry.write(), &mut policies)?;
    let first = Handle::<WorldAsset>::Uuid(uuid::Uuid::new_v4(), default());
    let second = Handle::<WorldAsset>::Uuid(uuid::Uuid::new_v4(), default());
    let mut old_scene = World::new();
    old_scene.spawn((Name::new("old imported root"), Transform::IDENTITY));
    world
        .resource_mut::<Assets<WorldAsset>>()
        .insert(first.id(), WorldAsset::new(old_scene))?;
    let mut new_scene = World::new();
    let root = new_scene
        .spawn((Name::new("new imported root"), Transform::IDENTITY))
        .id();
    new_scene.spawn((
        Name::new("new imported child"),
        Transform::from_xyz(2., 0., 0.),
        ChildOf(root),
    ));
    let mut loader = Loader(BTreeMap::from([
        ("game://first.glb#Scene0".into(), first.clone()),
        ("game://second.glb#Scene0".into(), second.clone()),
    ]));
    let mut document = PrefabDocument::new("Model source");
    let compile = |document: &PrefabDocument,
                   loader: &mut Loader|
     -> Result<CompiledPrefab, Box<dyn std::error::Error>> {
        let mut catalog = Catalog::default();
        catalog.insert(document.clone())?;
        Ok(CompiledPrefab::compile(
            catalog.resolve(document.id)?,
            &registry.read(),
            &policies,
            loader,
            &BTreeMap::new(),
        )?)
    };
    document.nodes[0].components.insert(
        "scene.model".into(),
        ComponentData::ron(1, "(scene:Path(\"game://first.glb#Scene0\"))")?,
    );
    let before = compile(&document, &mut loader)?;
    document.nodes[0].components.insert(
        "scene.model".into(),
        ComponentData::ron(1, "(scene:Path(\"game://second.glb#Scene0\"))")?,
    );
    let next = compile(&document, &mut loader)?;
    let owner = world
        .spawn((
            Transform::IDENTITY,
            Model {
                scene: first.clone(),
            },
        ))
        .id();
    let foreign = world.spawn(Name::new("foreign sibling")).id();
    let camera = world
        .spawn((Camera::default(), Transform::from_xyz(1., 2., 3.)))
        .id();
    let pose = *world.get::<Transform>(camera).ok_or("pose")?;
    let mut projection = Projection::pending(1);
    projection.camera = Some(camera);
    projection.nodes.insert(before.graph().root.clone(), owner);
    projection.models.insert(
        owner,
        crate::scene_view::models::mount(&mut world, owner, &first, 1, 4096)?,
    );
    let old_root = world.get::<Children>(owner).ok_or("old root")?[0];
    let epoch = projection.epoch;
    let nodes = world.entities().len();
    let unsupported = world.resource_mut::<Assets<WorldAsset>>().add({
        let mut source = World::new();
        source.spawn(Camera::default());
        WorldAsset::new(source)
    });
    assert!(kedra_preview::models::validate(&mut world, &unsupported).is_err());
    assert!(world.entities().contains(old_root));
    assert_eq!(world.get::<Transform>(camera), Some(&pose));
    assert_eq!(
        world.entities().len(),
        nodes,
        "unsupported model preflight leaked a staging owner"
    );
    assert!(
        projection
            .refresh_spatial_values(&mut world, &next, before.graph())
            .is_err()
    );
    assert_eq!(world.entities().len(), nodes);
    assert_eq!(projection.epoch, epoch);
    assert!(world.entities().contains(old_root));
    world
        .resource_mut::<Assets<WorldAsset>>()
        .insert(second.id(), WorldAsset::new(new_scene))?;
    kedra_preview::models::validate(&mut world, &second)?;
    let staged = prepare(&mut world, &projection, &next, before.graph())?;
    assert!(world.entities().contains(old_root));
    assert_eq!(
        world.get::<Model>(owner).map(|model| &model.scene),
        Some(&first)
    );
    staged.cancel(&mut world);
    assert!(
        world
            .query::<&Name>()
            .iter(&world)
            .all(|name| !name.as_str().starts_with("new imported")),
        "abandoned staging leaked imported nodes"
    );
    assert_eq!(
        world.query::<&Transform>().iter(&world).count(),
        3,
        "abandoned staging must retire its temporary owner and transforms"
    );
    projection.refresh_spatial_values(&mut world, &next, before.graph())?;
    assert!(!world.entities().contains(old_root));
    assert!(world.entities().contains(foreign));
    assert_eq!(
        world.get::<Model>(owner).map(|model| &model.scene),
        Some(&second)
    );
    assert_eq!(world.get::<Transform>(camera), Some(&pose));
    assert_eq!(projection.models[&owner].len(), 2);
    assert!(projection.epoch > epoch);
    let new_root = world.get::<Children>(owner).ok_or("new root")?[0];
    world.entity_mut(foreign).insert(ChildOf(new_root));
    let epoch = projection.epoch;
    assert!(
        projection
            .refresh_spatial_values(&mut world, &before, next.graph())
            .is_err()
    );
    assert_eq!(projection.epoch, epoch);
    assert!(world.entities().contains(new_root));
    assert_eq!(
        world.get::<ChildOf>(foreign).map(ChildOf::parent),
        Some(new_root)
    );
    assert_eq!(
        world.get::<Model>(owner).map(|model| &model.scene),
        Some(&second)
    );
    Ok(())
}
