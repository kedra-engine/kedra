//! Minimal source/registry fixtures shared by the authoring contract modules.
use bevy::{asset::AssetPlugin, prelude::*};
use kedra_authoring::persistence::{SourceFile, SourceSnapshot, content_hash};
use kedra_scene::{document::*, reflection::ComponentPolicies};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct Settings {
    pub speed: f32,
}

#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct Links {
    #[entities]
    pub optional: Option<Entity>,
}

pub fn setup() -> Result<(App, ComponentPolicies)> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_resource::<AppTypeRegistry>();
    let mut policies = ComponentPolicies::default();
    let types = app.world().resource::<AppTypeRegistry>();
    policies.register::<Settings>(&mut types.write(), "settings", 1, |_| Ok(()))?;
    policies.register::<Links>(&mut types.write(), "links", 1, |_| Ok(()))?;
    Ok((app, policies))
}

pub fn document() -> Result<PrefabDocument> {
    let mut document = PrefabDocument::new("Actor");
    document.nodes[0]
        .components
        .insert("settings".into(), ComponentData::ron(1, "(speed:1.0)")?);
    Ok(document)
}

pub fn baseline(documents: &[PrefabDocument]) -> Result<SourceSnapshot> {
    let documents = documents
        .iter()
        .map(|document| {
            let source = document.serialize()?;
            Ok(SourceFile {
                path: format!("{}.prefab.ron", document.id),
                hash: content_hash(&source),
                source,
            })
        })
        .collect::<kedra_scene::Result<Vec<_>>>()?;
    Ok(SourceSnapshot {
        namespace: "game".into(),
        generation: 1,
        documents,
        assets: Vec::new(),
        index: None,
    })
}
