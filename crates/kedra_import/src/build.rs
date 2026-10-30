use bevy::{
    asset::{
        AssetApp, AssetLoader, AssetMode, AssetPlugin,
        meta::{AssetAction, AssetMeta},
        processor::{AssetProcessor, FileTransactionLogFactory, ProcessStatus},
    },
    prelude::*,
    tasks::{IoTaskPool, block_on, poll_once},
};
use kedra_assets::delivery::{
    AssetRecord, FileDigest, MANIFEST, MAX_MANIFEST_BYTES, Manifest, digest, normalized,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Deserialize)]
struct Identity {
    id: Uuid,
}

fn paths(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().is_symlink() {
            return Err(format!(
                "Symlink source is not supported: {}",
                entry.path().display()
            ));
        }
        if !entry.file_type().is_file() && !entry.file_type().is_dir() {
            return Err(format!(
                "Asset sources and sidecars must be regular files: {}",
                entry.path().display()
            ));
        }
        if entry.file_type().is_file() && entry.path().extension().is_none_or(|ext| ext != "meta") {
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("Asset paths must be Unicode")?
                .replace(std::path::MAIN_SEPARATOR, "/");
            if !normalized(&relative) {
                return Err(format!("Invalid source asset path: {relative}"));
            }
            paths.push(entry.path().to_path_buf());
        }
    }
    paths.sort();
    Ok(paths)
}

fn encode_manifest(manifest: &Manifest) -> Result<Vec<u8>, String> {
    // Delivery metadata is a machine-readable artifact. Spend its bounded byte
    // budget on inventory data, while retaining the reader's exact size limit.
    let bytes = serde_json::to_vec(manifest).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(format!(
            "Asset delivery manifest exceeds {MAX_MANIFEST_BYTES} bytes; delivery was not published"
        ));
    }
    Ok(bytes)
}

fn load<L: AssetLoader>(id: Uuid) -> Result<String, String>
where
    L::Settings: Default,
{
    encoded(
        id,
        AssetMeta::<L, ()>::new(AssetAction::Load {
            loader: L::type_path().into(),
            settings: L::Settings::default(),
        }),
    )
}
fn encoded<L: AssetLoader, P: bevy::asset::processor::Process>(
    id: Uuid,
    meta: AssetMeta<L, P>,
) -> Result<String, String> {
    let body = ron::to_string(&meta).map_err(|e| e.to_string())?;
    Ok(format!(
        "(id:{:?},{}",
        id.to_string(),
        body.strip_prefix('(')
            .ok_or("Invalid native asset metadata")?
    ))
}

/// Assign persistent UUID/native Bevy metadata to missing sidecars only.
/// Existing sidecars are never overwritten. Each new sidecar is independently
/// atomic; a failed pass can be rerun without changing previously assigned IDs.
/// Static symlinks, special files and non-addressable native asset paths fail
/// admission before sidecars are written. Sources must not be concurrently replaced.
pub fn initialize(source: &Path, mut progress: impl FnMut(&str)) -> Result<usize, String> {
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    let mut count = 0;
    for path in paths(&source)? {
        let meta = path.with_file_name(format!(
            "{}.meta",
            path.file_name()
                .and_then(|n| n.to_str())
                .ok_or("Asset filenames must be Unicode")?
        ));
        if meta.exists() {
            continue;
        }
        let document = path
            .to_str()
            .is_some_and(|p| p.ends_with(".scene.ron") || p.ends_with(".prefab.ron"));
        let id = if document {
            kedra_scene::document::PrefabDocument::parse(
                &fs::read_to_string(&path).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?
            .id
            .0
        } else {
            Uuid::new_v4()
        };
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let text = if document {
            load::<kedra_scene::assets::DocumentLoader>(id)?
        } else if path.to_str().is_some_and(|p| p.ends_with(".prefabs.ron")) {
            load::<kedra_scene::assets::CatalogLoader>(id)?
        } else if path
            .to_str()
            .is_some_and(|p| p.ends_with(".environment.ron"))
        {
            load::<kedra_scene::environment::EnvironmentLoader>(id)?
        } else {
            match extension.as_str() {
                "svg" => encoded::<(), crate::processors::SvgToImage>(
                    id,
                    AssetMeta::new(AssetAction::Process {
                        processor: crate::processors::SvgToImage::type_path().into(),
                        settings: crate::processors::SvgSettings::default(),
                    }),
                )?,
                "png" => load::<bevy::image::ImageLoader>(id)?,
                "glb" => encoded::<bevy::gltf::GltfLoader, ()>(
                    id,
                    AssetMeta::new(AssetAction::Load {
                        loader: bevy::gltf::GltfLoader::type_path().into(),
                        settings: bevy::gltf::GltfLoaderSettings {
                            load_cameras: false,
                            load_lights: false,
                            load_animations: false,
                            ..default()
                        },
                    }),
                )?,
                "ttf" | "otf" => load::<bevy::text::FontLoader>(id)?,
                "css" => encoded::<bevy_flair::parser::CssStyleSheetLoader, ()>(
                    id,
                    AssetMeta::new(AssetAction::Load {
                        loader: bevy_flair::parser::CssStyleSheetLoader::type_path().into(),
                        settings: bevy_flair::parser::CssStyleLoaderSetting {
                            error_mode: bevy_flair::parser::CssStyleLoaderErrorMode::ReturnError,
                        },
                    }),
                )?,
                "txt" | "json" | "ron" => load::<kedra_assets::data::DataLoader>(id)?,
                _ => encoded::<(), ()>(id, AssetMeta::new(AssetAction::Ignore))?,
            }
        };
        let parent = meta.parent().ok_or("Missing sidecar parent")?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        temp.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        temp.persist_noclobber(&meta).map_err(|e| e.to_string())?;
        count += 1;
        progress(&format!(
            "Assigned asset identity: {}",
            path.strip_prefix(&source)
                .map_err(|e| e.to_string())?
                .display()
        ));
    }
    Ok(count)
}

/// Process sources using Bevy, validate runtime loading and publish a fresh
/// delivery directory. Existing destinations are never overwritten. Failed
/// processing leaves source files and previous deliveries untouched.
///
/// Processing is headless. Progress is supplied by the CLI/host, never hidden in
/// a spinner-only protocol. This first implementation uses a clean transaction
/// directory on each invocation; incremental cache reuse is not claimed.
/// Source admission rejects static symlinks/special files and paths that change
/// meaning as native asset URIs. The exact encoded manifest must fit the runtime
/// byte limit before the public output is created. These checks are not an OS
/// sandbox against concurrent filesystem replacement.
pub fn build(
    source: &Path,
    output: &Path,
    profile: &str,
    namespace: &str,
    mut progress: impl FnMut(&str),
) -> Result<Manifest, String> {
    if profile.is_empty() || profile.len() > 64 {
        return Err("A processing profile of 1..64 bytes is required".into());
    }
    if namespace.is_empty()
        || namespace.len() > 64
        || !namespace
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err("Asset namespace must contain 1..64 ASCII letters, digits, '_' or '-'".into());
    }
    if output.exists() {
        return Err(
            "Delivery output must be a new directory; existing delivery was not changed".into(),
        );
    }
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let parent = parent.canonicalize().map_err(|e| e.to_string())?;
    let output = parent.join(output.file_name().ok_or("Missing delivery name")?);
    if output.starts_with(&source) || source.starts_with(&output) {
        return Err("Source and delivery roots must be disjoint".into());
    }
    let temp = tempfile::tempdir_in(&parent).map_err(|e| e.to_string())?;
    let processed = temp.path().join("processed");
    let files = paths(&source)?;
    let mut expected = Vec::new();
    let mut inputs = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for file in &files {
        let relative = file
            .strip_prefix(&source)
            .map_err(|e| e.to_string())?
            .to_str()
            .ok_or("Asset paths must be Unicode")?
            .replace(std::path::MAIN_SEPARATOR, "/");
        if !normalized(&relative) {
            return Err(format!("Invalid source asset path: {relative}"));
        }
        let bytes = fs::read(file).map_err(|e| e.to_string())?;
        // AssetProcessor reads raw sources, bypassing verified delivery readers.
        // Admit every GLB before constructing it, even if metadata ignores it.
        let model_checked = bytes.starts_with(b"glTF")
            || file
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("glb"));
        if model_checked {
            kedra_assets::model::validate_glb(&bytes)
                .map_err(|error| format!("{relative}: {error}"))?;
        }
        let meta_path = format!("{relative}.meta");
        let meta = fs::read(source.join(&meta_path))
            .map_err(|e| format!("{meta_path}: {e}; initialize asset metadata first"))?;
        let identity: Identity =
            ron::de::from_bytes(&meta).map_err(|e| format!("{meta_path}: {e}"))?;
        if identity.id.is_nil() || !ids.insert(identity.id) {
            return Err(format!("Invalid/duplicate asset UUID: {relative}"));
        }
        if relative.ends_with(".scene.ron") || relative.ends_with(".prefab.ron") {
            let document = kedra_scene::document::PrefabDocument::parse(
                std::str::from_utf8(&bytes).map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("{relative}: {e}"))?;
            if document.id.0 != identity.id {
                return Err(format!(
                    "Scene/document UUID differs from native metadata: {relative}"
                ));
            }
        }
        let native: bevy::asset::meta::AssetMetaMinimal =
            ron::de::from_bytes(&meta).map_err(|e| e.to_string())?;
        if !model_checked
            && matches!(&native.asset,
            bevy::asset::meta::AssetActionMinimal::Load { loader }
            if loader == "bevy_gltf::loader::GltfLoader")
        {
            kedra_assets::model::validate_glb(&bytes)
                .map_err(|error| format!("{relative}: {error}"))?;
        }
        let deliver = !matches!(native.asset, bevy::asset::meta::AssetActionMinimal::Ignore);
        if file
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("css"))
        {
            let css: AssetMeta<bevy_flair::parser::CssStyleSheetLoader, ()> =
                ron::de::from_bytes(&meta).map_err(|e| format!("{meta_path}: {e}"))?;
            if bytes.len() > 65_536
                || !matches!(css.asset,AssetAction::Load {ref loader,ref settings}
                if loader==bevy_flair::parser::CssStyleSheetLoader::type_path()
                    && matches!(settings.error_mode,bevy_flair::parser::CssStyleLoaderErrorMode::ReturnError))
            {
                return Err(format!(
                    "CSS requires a bounded source and strict native loader metadata: {relative}"
                ));
            }
        }
        if deliver
            && file
                .extension()
                .is_some_and(|ext| ext == "ttf" || ext == "otf")
        {
            read_fonts::FontRef::new(&bytes)
                .map_err(|e| format!("Invalid font {relative}: {e}"))?;
        }
        inputs.insert(relative.clone(), digest(&bytes));
        inputs.insert(meta_path, digest(&meta));
        expected.push((relative, identity.id, deliver));
    }
    progress(&format!(
        "Processing {} sources with native Bevy processors",
        files.len()
    ));
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: source.to_str().ok_or("Source root must be Unicode")?.into(),
            processed_file_path: processed
                .to_str()
                .ok_or("Output root must be Unicode")?
                .into(),
            mode: AssetMode::Processed,
            use_asset_processor_override: Some(true),
            watch_for_changes_override: Some(false),
            ..default()
        },
        bevy::image::ImagePlugin::default(),
    ));
    app.register_asset_loader(bevy::image::ImageLoader::new(
        bevy::image::CompressedImageFormats::all(),
    ))
    .init_asset::<Font>()
    .init_asset_loader::<bevy::text::FontLoader>()
    .add_plugins((
        kedra_assets::KedraAssetsPlugin,
        crate::KedraImportPlugin,
        kedra_scene::assets::SceneAssetPlugin,
        kedra_ui::styles::StyleAssetsPlugin,
    ));
    crate::verification::install_models(&mut app);
    app.finish();
    app.cleanup();
    let processor = app.world().resource::<AssetProcessor>().clone();
    processor
        .data()
        .set_log_factory(Box::new(FileTransactionLogFactory {
            file_path: temp.path().join("transactions"),
        }))
        .map_err(|e| e.to_string())?;
    app.update();
    let waiting = processor.clone();
    let mut task =
        IoTaskPool::get().spawn(async move { waiting.data().wait_until_finished().await });
    let deadline = Instant::now() + Duration::from_secs(60);
    while block_on(poll_once(&mut task)).is_none() {
        if Instant::now() > deadline {
            return Err(
                "Asset processing timed out after 60 seconds; sources were not modified".into(),
            );
        }
        app.update();
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut manifest = Manifest {
        format: 1,
        profile: profile.into(),
        assets: Vec::new(),
        files: BTreeMap::new(),
    };
    for (path, id, deliver) in expected {
        if !deliver {
            continue;
        }
        if block_on(processor.data().wait_until_processed(path.clone().into()))
            != ProcessStatus::Processed
        {
            return Err(format!("Asset processing failed: {path}"));
        }
        let meta = fs::read(processed.join(format!("{path}.meta"))).map_err(|e| e.to_string())?;
        let native: bevy::asset::meta::AssetMetaMinimal =
            ron::de::from_bytes(&meta).map_err(|e| e.to_string())?;
        let bevy::asset::meta::AssetActionMinimal::Load { loader } = native.asset else {
            return Err(format!("Processed asset is not loadable: {path}"));
        };
        manifest.assets.push(AssetRecord {
            id,
            path: path.clone(),
            loader,
        });
        for member in [path.clone(), format!("{path}.meta")] {
            let bytes = fs::read(processed.join(&member)).map_err(|e| e.to_string())?;
            manifest.files.insert(
                member,
                FileDigest {
                    bytes: bytes.len() as u64,
                    sha256: digest(&bytes),
                },
            );
        }
        progress(&format!("Processed: {path}"));
    }
    let marker_bytes = encode_manifest(&manifest)?;
    crate::verification::verify_processed(&processed, &manifest, namespace, &mut progress)?;
    manifest.verify(profile, |path| {
        fs::read(processed.join(path)).map_err(|e| e.to_string())
    })?;
    for (path, hash) in inputs {
        if digest(&fs::read(source.join(path)).map_err(|e| e.to_string())?) != hash {
            return Err("Sources changed during import; delivery was not published".into());
        }
    }
    if paths(&source)? != files {
        return Err("Source inventory changed during import".into());
    }
    fs::create_dir(&output).map_err(|e| e.to_string())?;
    for path in manifest.files.keys() {
        let target = output.join(path);
        fs::create_dir_all(target.parent().ok_or("Missing delivery parent")?)
            .map_err(|e| e.to_string())?;
        fs::copy(processed.join(path), target).map_err(|e| e.to_string())?;
    }
    let mut marker = tempfile::NamedTempFile::new_in(&output).map_err(|e| e.to_string())?;
    marker.write_all(&marker_bytes).map_err(|e| e.to_string())?;
    marker.as_file().sync_all().map_err(|e| e.to_string())?;
    marker
        .persist_noclobber(output.join(MANIFEST))
        .map_err(|e| e.to_string())?;
    progress("Asset delivery verified and published");
    Ok(manifest)
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
