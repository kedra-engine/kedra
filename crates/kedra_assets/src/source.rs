use crate::delivery::{Manifest, digest, normalized};
use bevy::{
    asset::{
        AssetApp,
        io::{
            AssetReader, AssetReaderError, AssetSource, AssetSourceBuilder, ErasedAssetReader,
            PathStream, Reader, VecReader,
        },
    },
    prelude::*,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Register a verified source before Bevy's `AssetPlugin`.
///
/// URIs are `source://path/to/asset.ext#Label` (preferred) or
/// `source://UUID/asset.ext#Label`. Both address declared manifest assets only.
/// UUID-address syntax is reserved and cannot also name a physical asset path.
/// Every read verifies content against
/// the supplied immutable manifest. The host validates manifest structure first;
/// bootstrap may load a minimal UI before full delivery verification completes.
/// Normal product startup must still wait for full delivery verification.
/// Hosts loading native models must enable `model-support`, which also admits
/// model topology/expansion before returning data bytes to the native loader.
/// Native filesystem deliveries must remain immutable while registered. Regular
/// members are checked for canonical containment before opening; these portable
/// preflight checks do not sandbox concurrent filesystem replacement.
pub fn register_source(
    app: &mut App,
    name: impl Into<bevy::asset::io::AssetSourceId<'static>>,
    root: String,
    manifest: Manifest,
) {
    let manifest = Arc::new(manifest);
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    let native_root = bevy::asset::io::file::FileAssetReader::new(&root)
        .root_path()
        .canonicalize()
        .map_err(|error| error.to_string());
    let mut reader = AssetSource::get_default_reader(root);
    app.register_asset_source(
        name,
        AssetSourceBuilder::new(move || {
            Box::new(VerifiedReader {
                inner: reader(),
                manifest: manifest.clone(),
                #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
                native_root: native_root.clone(),
            })
        }),
    );
}
struct VerifiedReader {
    inner: Box<dyn ErasedAssetReader>,
    manifest: Arc<Manifest>,
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    native_root: Result<PathBuf, String>,
}
fn invalid(error: impl std::fmt::Display) -> AssetReaderError {
    std::io::Error::other(error.to_string()).into()
}
pub(crate) fn uuid_reference(text: &str) -> Option<uuid::Uuid> {
    let (id, leaf) = text.split_once('/')?;
    (leaf.starts_with("asset.") && leaf.len() > 6 && !leaf.contains('/'))
        .then(|| id.parse::<uuid::Uuid>().ok())
        .flatten()
}
impl VerifiedReader {
    fn mapped(&self, path: &Path) -> Result<PathBuf, AssetReaderError> {
        let text = path
            .to_str()
            .ok_or_else(|| invalid("Asset reference must be UTF-8"))?;
        // Bevy resolves relative references with PathBuf::push, which introduces
        // native separators on Windows. Manifest keys remain portable slash paths.
        // Convert only the platform separator: a Unix backslash is not a separator.
        let text = text.replace(std::path::MAIN_SEPARATOR, "/");
        if !normalized(&text) {
            return Err(invalid(
                "Asset reference must be a normalized delivery-relative path",
            ));
        }
        let identity = uuid_reference(&text);
        self.manifest
            .assets
            .iter()
            .find(|asset| match identity {
                Some(id) => asset.id == id,
                None => asset.path == text,
            })
            .map(|asset| PathBuf::from(&asset.path))
            .ok_or_else(|| AssetReaderError::NotFound(path.into()))
    }
    async fn bytes(&self, path: &Path, meta: bool) -> Result<VecReader, AssetReaderError> {
        let physical = self.mapped(path)?;
        let text = physical
            .to_str()
            .ok_or_else(|| invalid("Invalid delivery path"))?;
        let key = if meta {
            format!("{text}.meta")
        } else {
            text.into()
        };
        let expected = self
            .manifest
            .files
            .get(&key)
            .ok_or_else(|| AssetReaderError::NotFound(physical.clone()))?;
        let limit = expected.read_limit(&key).map_err(invalid)?;
        #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
        crate::filesystem::member(
            self.native_root.as_ref().map_err(invalid)?,
            &key,
            expected.bytes,
        )
        .map_err(invalid)?;
        let reader = if meta {
            self.inner.read_meta(&physical).await?
        } else {
            self.inner.read(&physical).await?
        };
        let mut bytes = Vec::new();
        // Use the maintained async adapter, bypassing unbounded Reader overrides.
        use bevy::tasks::futures_lite::AsyncReadExt;
        reader.take(limit).read_to_end(&mut bytes).await?;
        if bytes.len() as u64 != expected.bytes || digest(&bytes) != expected.sha256 {
            return Err(invalid(format!("Asset delivery integrity failure: {key}")));
        }
        #[cfg(feature = "model-support")]
        if !meta
            && (bytes.starts_with(b"glTF")
                || physical
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("glb"))
                || self.manifest.assets.iter().any(|asset| {
                    asset.path == text && asset.loader == "bevy_gltf::loader::GltfLoader"
                }))
        {
            crate::model::validate_glb(&bytes).map_err(invalid)?;
        }
        Ok(VecReader::new(bytes))
    }
}
impl AssetReader for VerifiedReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        self.bytes(path, false).await
    }
    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        self.bytes(path, true).await
    }
    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        Err(AssetReaderError::NotFound(path.into()))
    }
    async fn is_directory<'a>(&'a self, _: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(false)
    }
}

#[cfg(all(test, not(any(target_arch = "wasm32", target_os = "android"))))]
#[path = "source_tests.rs"]
mod tests;
