//! Independent game-style consumers exercise engine assets without editor/UI.
#![allow(clippy::unwrap_used, reason = "Isolated asset-pipeline assertions")]
use bevy::{
    asset::{AssetApp, AssetPlugin},
    prelude::*,
    tasks::block_on,
};
use kedra_assets::{
    delivery::{self, Manifest},
    images::ImageVariants,
};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="12"><path d="M1 1 L22 10" stroke="red" stroke-width="1.5"/></svg>"#;

fn wait_for<A: Asset>(app: &mut App, handle: &Handle<A>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app
        .world()
        .resource::<AssetServer>()
        .is_loaded_with_dependencies(handle.id())
    {
        app.update();
        if let Some(bevy::asset::LoadState::Failed(error)) = app
            .world()
            .resource::<AssetServer>()
            .get_load_state(handle.id())
        {
            panic!("{error}");
        }
        assert!(
            Instant::now() < deadline,
            "Timed out loading processed delivery"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn consumer(root: &Path, manifest: Manifest) -> App {
    let mut app = App::new();
    kedra_assets::register_source(&mut app, "game", root.to_str().unwrap().into(), manifest);
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            use_asset_processor_override: Some(false),
            ..default()
        },
        bevy::image::ImagePlugin::default(),
        kedra_assets::KedraAssetsPlugin,
    ));
    app.register_asset_loader(bevy::image::ImageLoader::new(
        bevy::image::CompressedImageFormats::all(),
    ));
    app
}

#[test]
fn game_consumes_processed_svg_variants_after_sources_are_unavailable() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("sources");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("game.svg"), SVG).unwrap();
    fs::write(source.join("variants.svg"), SVG).unwrap();
    assert_eq!(kedra_import::initialize(&source, |_| {}).unwrap(), 2);
    let path = source.join("variants.svg.meta");
    let old = fs::read_to_string(&path).unwrap();
    #[derive(serde::Deserialize)]
    struct Identity {
        id: uuid::Uuid,
    }
    let identity: Identity = ron::from_str(&old).unwrap();
    fs::write(&path,format!("(id:{:?},meta_format_version:\"1.0\",asset:Process(processor:\"kedra_import::processors::SvgToImageVariants\",settings:(size:None,densities:[1,2,3,4])))",identity.id.to_string())).unwrap();
    let unchanged = fs::read_to_string(&path).unwrap();
    assert_eq!(kedra_import::initialize(&source, |_| {}).unwrap(), 0);
    assert_eq!(fs::read_to_string(&path).unwrap(), unchanged);
    let output = temp.path().join("delivery");
    let manifest = kedra_import::build(&source, &output, "game-desktop", "game", |_| {}).unwrap();
    assert_eq!(manifest.assets.len(), 2);
    assert!(kedra_import::build(&source, &output, "game-desktop", "game", |_| {}).is_err());
    fs::rename(&source, temp.path().join("unavailable-sources")).unwrap();
    let (_, verified) = delivery::read_directory(&output, "game-desktop").unwrap();
    let single = verified
        .assets
        .iter()
        .find(|asset| asset.path == "game.svg")
        .unwrap()
        .id;
    let variants = verified
        .assets
        .iter()
        .find(|asset| asset.path == "variants.svg")
        .unwrap()
        .id;
    let mut app = consumer(&output, verified);
    let image: Handle<Image> = app
        .world()
        .resource::<AssetServer>()
        .load(format!("game://{single}/asset.svg"));
    let set: Handle<ImageVariants> = app
        .world()
        .resource::<AssetServer>()
        .load(format!("game://{variants}/asset.svg"));
    wait_for(&mut app, &image);
    wait_for(&mut app, &set);
    let path_image: Handle<Image> = app
        .world()
        .resource::<AssetServer>()
        .load("game://game.svg");
    let path_variant: Handle<Image> = app
        .world()
        .resource::<AssetServer>()
        .load("game://variants.svg#2x");
    wait_for(&mut app, &path_image);
    wait_for(&mut app, &path_variant);
    assert_eq!(
        app.world()
            .resource::<Assets<Image>>()
            .get(&path_image)
            .unwrap()
            .width(),
        24
    );
    assert_eq!(
        app.world()
            .resource::<Assets<Image>>()
            .get(&path_variant)
            .unwrap()
            .width(),
        48
    );
    assert_eq!(
        app.world()
            .resource::<Assets<Image>>()
            .get(&image)
            .unwrap()
            .width(),
        24
    );
    let images = app
        .world()
        .resource::<Assets<ImageVariants>>()
        .get(&set)
        .unwrap()
        .images
        .clone();
    for (index, image) in images.iter().enumerate() {
        assert_eq!(
            app.world()
                .resource::<Assets<Image>>()
                .get(image)
                .unwrap()
                .width(),
            24 * (index as u32 + 1)
        );
    }
    let source = app
        .world()
        .resource::<AssetServer>()
        .get_source("game")
        .unwrap();
    // A physical file not declared as an asset cannot bypass the manifest by path.
    fs::write(
        output.join("unlisted.svg"),
        fs::read(output.join("game.svg")).unwrap(),
    )
    .unwrap();
    for path in [
        "unlisted.svg",
        "game.svg.meta",
        "../game.svg",
        "/game.svg",
        "missing.svg",
    ] {
        assert!(
            block_on(source.reader().read(Path::new(path))).is_err(),
            "{path}"
        );
    }
    fs::write(output.join("game.svg"), "corruption").unwrap();
    assert!(block_on(source.reader().read(Path::new("game.svg"))).is_err());
    assert!(
        block_on(
            source
                .reader()
                .read(&std::path::PathBuf::from(format!("{single}/asset.svg")))
        )
        .is_err()
    );
    let meta = output.join("game.svg.meta");
    let mut bytes = fs::read(&meta).unwrap();
    bytes.push(b' ');
    fs::write(&meta, bytes).unwrap();
    assert!(block_on(source.reader().read_meta(Path::new("game.svg"))).is_err());
    assert!(
        block_on(
            source
                .reader()
                .read_meta(Path::new(&format!("{single}/asset.svg")))
        )
        .is_err()
    );
    assert!(delivery::read_directory(&output, "game-desktop").is_err());
}

#[test]
fn paths_are_exact_unicode_delivery_names_and_uuid_survives_source_rename() {
    use kedra_assets::data::DataAsset;
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir_all(source.join("data")).unwrap();
    fs::write(source.join("original.txt"), "Кедра 日本語").unwrap();
    kedra_import::initialize(&source, |_| {}).unwrap();
    let first =
        kedra_import::build(&source, &temp.path().join("first"), "game", "game", |_| {}).unwrap();
    let id = first.assets[0].id;
    let renamed = "data/Кедра 日本語.txt";
    fs::rename(source.join("original.txt"), source.join(renamed)).unwrap();
    fs::rename(
        source.join("original.txt.meta"),
        source.join(format!("{renamed}.meta")),
    )
    .unwrap();
    let output = temp.path().join("renamed");
    let manifest = kedra_import::build(&source, &output, "game", "game", |_| {}).unwrap();
    assert_eq!(manifest.assets[0].id, id);
    fs::rename(&source, temp.path().join("offline")).unwrap();
    let mut app = consumer(&output, manifest);
    let by_path: Handle<DataAsset> = app
        .world()
        .resource::<AssetServer>()
        .load(format!("game://{renamed}"));
    let by_uuid: Handle<DataAsset> = app
        .world()
        .resource::<AssetServer>()
        .load(format!("game://{id}/asset.txt"));
    wait_for(&mut app, &by_path);
    wait_for(&mut app, &by_uuid);
    let data = app.world().resource::<Assets<DataAsset>>();
    assert_eq!(data.get(&by_path).unwrap().bytes, "Кедра 日本語".as_bytes());
    assert_eq!(
        data.get(&by_path).unwrap().bytes,
        data.get(&by_uuid).unwrap().bytes
    );
    let source = app
        .world()
        .resource::<AssetServer>()
        .get_source("game")
        .unwrap();
    for missing in ["original.txt", "data/кедра 日本語.txt"] {
        assert!(
            block_on(source.reader().read(Path::new(missing))).is_err(),
            "{missing}"
        );
    }
}

#[test]
fn invalid_sources_never_publish_a_delivery_or_overwrite_sidecars() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let target = temp.path().join("delivery");
    fs::write(source.join("bad.svg"), "not SVG").unwrap();
    assert!(kedra_import::build(&source, &target, "game", "game", |_| {}).is_err());
    kedra_import::initialize(&source, |_| {}).unwrap();
    let before = fs::read(source.join("bad.svg.meta")).unwrap();
    assert!(kedra_import::build(&source, &target, "game", "game", |_| {}).is_err());
    assert!(!target.exists());
    assert_eq!(fs::read(source.join("bad.svg.meta")).unwrap(), before);
    fs::write(source.join("bad.svg"), SVG).unwrap();
    fs::write(source.join("other.svg"), SVG).unwrap();
    fs::write(source.join("other.svg.meta"), &before).unwrap();
    assert!(kedra_import::build(&source, &target, "game", "game", |_| {}).is_err());
    assert!(!target.exists());
}
