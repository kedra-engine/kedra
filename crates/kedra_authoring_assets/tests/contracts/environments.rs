//! Authored source reads keep the native logical path and validate catalog bytes.
use bevy::{
    asset::{
        AssetApp,
        io::{AssetSourceBuilder, memory::MemoryAssetReader},
    },
    prelude::*,
};
use kedra_authoring::persistence::{SourceAsset, SourceSnapshot, content_hash};
use kedra_authoring_assets::environments::{Sources, accepts, install};
use std::path::Path;

#[test]
fn native_source_overlay_reads_new_profiles_and_rejects_invalid_admission()
-> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();
    let memory = MemoryAssetReader::default();
    memory
        .root
        .insert_asset_text(Path::new("retained.txt"), "verified fallback");
    app.register_asset_source(
        "game",
        AssetSourceBuilder::new(move || Box::new(memory.clone())),
    );
    install(&mut app, "game");
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
    let text = "(version:1,background:Srgba((red:0.1,green:0.2,blue:0.3,alpha:1.0)),ambient_color:Srgba((red:1.0,green:1.0,blue:1.0,alpha:1.0)),ambient_brightness:275.0)";
    let mut source = SourceSnapshot {
        namespace: "game".into(),
        generation: 1,
        documents: vec![],
        index: None,
        assets: vec![SourceAsset {
            id: Some(uuid::Uuid::new_v4()),
            path: "new.environment.ron".into(),
            hash: content_hash(text),
            bytes: text.len() as u64,
        }],
        environments: [("new.environment.ron".into(), text.into())].into(),
    };
    let sources = app.world().resource::<Sources>().clone();
    sources.publish(Some(&source))?;
    assert!(accepts(
        &source,
        &bevy::asset::AssetPath::parse("game://new.environment.ron")
    ));
    assert!(!accepts(
        &source,
        &bevy::asset::AssetPath::parse("editor://new.environment.ron")
    ));
    assert!(!accepts(
        &source,
        &bevy::asset::AssetPath::parse("game://new.environment.ron#Other")
    ));
    let server = app.world().resource::<AssetServer>();
    let native = server.get_source("game")?;
    bevy::tasks::block_on(async {
        use bevy::tasks::futures_lite::AsyncReadExt;
        let mut text = String::new();
        native
            .reader()
            .read(Path::new("new.environment.ron"))
            .await?
            .read_to_string(&mut text)
            .await?;
        assert_eq!(
            kedra_scene::environment::Environment::parse(&text)?.ambient_brightness,
            275.0
        );
        let mut meta = String::new();
        native
            .reader()
            .read_meta(Path::new("new.environment.ron"))
            .await?
            .read_to_string(&mut meta)
            .await?;
        assert!(meta.contains(&source.assets[0].id.ok_or("Identity missing")?.to_string()));
        source
            .environments
            .insert("new.environment.ron".into(), "invalid".into());
        sources.publish(Some(&source))?;
        assert!(
            native
                .reader()
                .read(Path::new("new.environment.ron"))
                .await
                .is_err()
        );
        sources.publish(None)?;
        assert!(
            native
                .reader()
                .read(Path::new("new.environment.ron"))
                .await
                .is_err()
        );
        let mut fallback = String::new();
        native
            .reader()
            .read(Path::new("retained.txt"))
            .await?
            .read_to_string(&mut fallback)
            .await?;
        assert_eq!(fallback, "verified fallback");
        Ok::<_, Box<dyn std::error::Error>>(())
    })
}
