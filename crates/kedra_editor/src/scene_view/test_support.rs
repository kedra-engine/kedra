//! CPU-only primitive projection and view fixtures; no native readiness claim.
use super::*;
use bevy::asset::{AssetPath, LoadFromPath, UntypedHandle};
use kedra_scene::{
    composition::Catalog,
    document::PrefabDocument,
    spatial::{Primitive, Surface},
};

struct NoLoad;
impl LoadFromPath for NoLoad {
    fn load_from_path_erased(
        &mut self,
        _: std::any::TypeId,
        _: AssetPath<'static>,
    ) -> UntypedHandle {
        panic!("Fixture must not load assets")
    }
}

pub(super) fn compiled(
    world: &mut World,
) -> Result<Arc<CompiledPrefab>, Box<dyn std::error::Error>> {
    let registry = AppTypeRegistry::default();
    let mut policies = ComponentPolicies::default();
    kedra_scene::spatial::register(&mut registry.write(), &mut policies)?;
    let mut document = PrefabDocument::new("Primitive");
    for (key, value) in [
        (
            "scene.primitive",
            &Primitive::Cuboid {
                size: Vec3::splat(2.),
            } as &dyn PartialReflect,
        ),
        (
            "scene.surface",
            &Surface {
                color: Color::WHITE,
                texture: None,
                metallic: 0.,
                roughness: 0.5,
            },
        ),
    ] {
        document.nodes[0].components.insert(
            key.into(),
            policies.encode(key, value, &registry.read(), &BTreeMap::new())?,
        );
    }
    document.nodes.push(kedra_scene::document::Node {
        id: kedra_scene::document::NodeId::new(),
        name: "Authored light".into(),
        parent: Some(document.root.into()),
        components: BTreeMap::from([(
            "scene.sun".into(),
            policies.encode(
                "scene.sun",
                &kedra_scene::spatial::Sun {
                    color: Color::WHITE,
                    illuminance: 25.,
                },
                &registry.read(),
                &BTreeMap::new(),
            )?,
        )]),
    });
    let mut catalog = Catalog::default();
    catalog.insert(document.clone())?;
    let compiled = CompiledPrefab::compile(
        catalog.resolve(document.id)?,
        &registry.read(),
        &policies,
        &mut NoLoad,
        &BTreeMap::new(),
    )?;
    world.insert_resource(registry);
    world.init_resource::<Assets<Mesh>>();
    world.init_resource::<Assets<StandardMaterial>>();
    world.init_resource::<Assets<Image>>();
    Ok(Arc::new(compiled))
}

pub(super) fn empty(world: &mut World) -> View {
    let element = world.spawn(Text::default()).id();
    let buttons = (0..4).map(|_| world.spawn_empty().id()).collect();
    let mut controls = controls::Controls::default();
    controls.buttons = buttons;
    View {
        root: element,
        viewport: element,
        render: element,
        information: element,
        heading: element,
        controls,
        marker: default(),
        compiled: None,
        projection: None,
        lifecycle: default(),
        elapsed: 0.,
        raw_model: false,
    }
}

pub(crate) fn ready(world: &mut World) -> Result<View, String> {
    let mut view = empty(world);
    spatial(world, &mut view)?;
    Ok(view)
}

pub(crate) fn spatial(world: &mut World, view: &mut View) -> Result<Entity, String> {
    let compiled = compiled(world).map_err(|e| e.to_string())?;
    let mut projection = projection::Projection::pending(1);
    projection.mount(world, compiled.clone())?;
    let camera = projection.camera().ok_or("Camera missing")?;
    view.compiled = Some(compiled);
    view.projection = Some(projection);
    view.lifecycle.begin()?;
    view.lifecycle.instantiated();
    view.lifecycle.frame();
    Ok(camera)
}
