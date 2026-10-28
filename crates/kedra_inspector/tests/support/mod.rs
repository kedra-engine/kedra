//! Real authored kit templates for independent inspector consumers.
use bevy::{
    asset::{AssetApp, AssetPlugin},
    prelude::*,
};
use kedra_scene::{
    composition::Catalog, document::PrefabDocument, instance::CompiledPrefab,
    reflection::ComponentPolicies,
};
use kedra_ui::{authored::AuthoredUiPlugin, fields::FieldTemplates};
use std::{collections::BTreeMap, sync::Arc};
struct FixtureImages<'a>(&'a AssetServer);
impl bevy::asset::LoadFromPath for FixtureImages<'_> {
    fn load_from_path_erased(
        &mut self,
        ty: std::any::TypeId,
        path: bevy::asset::AssetPath<'static>,
    ) -> bevy::asset::UntypedHandle {
        if ty == std::any::TypeId::of::<Image>() {
            self.0.load::<Image>("memory://icon.cpu").untyped()
        } else {
            self.0.load_from_path_erased(ty, path)
        }
    }
}

#[derive(Default, TypePath)]
struct CpuImageLoader;
impl bevy::asset::AssetLoader for CpuImageLoader {
    type Asset = Image;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        _: &mut dyn bevy::asset::io::Reader,
        _: &(),
        _: &mut bevy::asset::LoadContext<'_>,
    ) -> std::result::Result<Image, Self::Error> {
        Ok(Image::default())
    }
    fn extensions(&self) -> &[&str] {
        &["cpu"]
    }
}

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub fn fixture() -> TestResult<(App, Entity, FieldTemplates)> {
    let mut app = App::new();
    let memory = bevy::asset::io::memory::Dir::default();
    memory.insert_asset_text(
        std::path::Path::new("icon.cpu"),
        "Explicit CPU-only image input; not a Figma reference",
    );
    app.register_asset_source(
        "memory",
        bevy::asset::io::AssetSourceBuilder::new(move || {
            Box::new(bevy::asset::io::memory::MemoryAssetReader {
                root: memory.clone(),
            })
        }),
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            meta_check: bevy::asset::AssetMetaCheck::Never,
            ..Default::default()
        },
    ))
    .init_asset::<Font>()
    .init_asset::<Image>()
    .init_asset_loader::<CpuImageLoader>()
    .init_resource::<AppTypeRegistry>()
    .add_plugins(AuthoredUiPlugin);
    let compile = |text: &str| -> TestResult<Arc<CompiledPrefab>> {
        let doc = PrefabDocument::parse(text)?;
        let id = doc.id;
        let mut catalog = Catalog::default();
        catalog.insert(doc)?;
        Ok(Arc::new(CompiledPrefab::compile(
            catalog.resolve(id)?,
            &app.world().resource::<AppTypeRegistry>().read(),
            app.world().resource::<ComponentPolicies>(),
            &mut FixtureImages(app.world().resource::<AssetServer>()),
            &BTreeMap::new(),
        )?))
    };
    let templates = FieldTemplates {
        command: compile(include_str!(
            "../../../../assets/editor/prefabs/button.prefab.ron"
        ))?,
        row: compile(include_str!(
            "../../../../assets/editor/prefabs/property-row.prefab.ron"
        ))?,
        value: compile(include_str!(
            "../../../../assets/editor/prefabs/property-value.prefab.ron"
        ))?,
        section: compile(include_str!(
            "../../../../assets/editor/prefabs/property-section.prefab.ron"
        ))?,
        vector: compile(include_str!(
            "../../../../assets/editor/prefabs/property-vector.prefab.ron"
        ))?,
        density: 1.,
    };
    // Real path-bearing loads preserve source compilation's ephemeral-handle guard.
    // These 1px inputs exercise ownership, not SVG decoding or reference pixels.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let mut ready = true;
        for template in [
            &templates.row,
            &templates.value,
            &templates.section,
            &templates.vector,
        ] {
            ready &= template.asset_status(app.world().resource::<AssetServer>())?;
        }
        if ready {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "CPU image dependencies did not load"
        );
        app.update();
        std::thread::yield_now();
    }
    let sheet = app
        .world_mut()
        .resource_mut::<Assets<bevy_flair::style::StyleSheet>>()
        .add(bevy_flair::style::StyleSheet::empty());
    let parent = app
        .world_mut()
        .spawn((
            Node::default(),
            bevy_flair::style::components::Styled::new(sheet),
        ))
        .id();
    Ok((app, parent, templates))
}
