//! Authored environment bytes overlay the host's existing native source reader.
use super::*;
use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceBuilders, ErasedAssetReader,
    PathStream, Reader, VecReader,
};
use std::sync::{Arc, RwLock};

/// Admitted source snapshot shared with asynchronous native environment reads.
/// This is not a filesystem watcher: hosts publish admission, and clear it when
/// retiring the project. Other asset types continue through the verified reader.
#[derive(Resource, Clone, Default)]
pub struct Sources(Arc<RwLock<Option<SourceSnapshot>>>);
impl Sources {
    /// Publish the host's current catalog. Readers verify identity, bytes and schema
    /// before serving an environment. None retires the authoring overlay.
    pub fn publish(&self, source: Option<&SourceSnapshot>) -> Result<(), String> {
        let mut current = self
            .0
            .write()
            .map_err(|_| "Environment source lock failed")?;
        if current
            .as_ref()
            .map(|s| (&s.namespace, &s.assets, &s.environments))
            != source.map(|s| (&s.namespace, &s.assets, &s.environments))
        {
            *current = source.cloned();
        }
        Ok(())
    }
}
/// Wrap an existing source before AssetPlugin consumes its builders. An absent
/// source gets an empty reader for non-environment requests. Install once per App;
/// game runtimes do not install this authoring-only adapter.
pub fn install(app: &mut App, namespace: &'static str) {
    app.init_resource::<AssetSourceBuilders>();
    let sources = Sources::default();
    app.insert_resource(sources.clone());
    let mut builders = app.world_mut().resource_mut::<AssetSourceBuilders>();
    if builders.get_mut(namespace).is_none() {
        builders.insert(
            namespace,
            AssetSourceBuilder::new(|| {
                Box::new(bevy::asset::io::memory::MemoryAssetReader::default())
            }),
        );
    }
    if let Some(builder) = builders.get_mut(namespace) {
        let mut original = std::mem::replace(
            &mut builder.reader,
            Box::new(|| Box::new(bevy::asset::io::memory::MemoryAssetReader::default())),
        );
        builder.reader = Box::new(move || {
            Box::new(Overlay {
                inner: original(),
                sources: sources.clone(),
                namespace,
            })
        });
    }
}
/// Resolve an environment through the admitted catalog, never by suffix alone.
pub fn accepts(source: &SourceSnapshot, path: &bevy::asset::AssetPath<'_>) -> bool {
    path.source().as_str() == Some(source.namespace.as_str())
        && path.label().is_none()
        && path.path().to_str().is_some_and(|path| {
            source
                .assets
                .iter()
                .any(|asset| asset.path == path && asset.id.is_some())
                && super::environment_source(source, path).is_ok()
        })
}
struct Overlay {
    inner: Box<dyn ErasedAssetReader>,
    sources: Sources,
    namespace: &'static str,
}
impl Overlay {
    fn bytes(&self, path: &Path, meta: bool) -> Result<Option<Vec<u8>>, AssetReaderError> {
        let current = self
            .sources
            .0
            .read()
            .map_err(|_| std::io::Error::other("Environment source lock failed"))?;
        let Some(source) = current.as_ref().filter(|s| s.namespace == self.namespace) else {
            return Ok(None);
        };
        let Some(path) = path
            .to_str()
            .filter(|p| source.environments.contains_key(*p))
        else {
            return Ok(None);
        };
        super::environment_source(source, path).map_err(std::io::Error::other)?;
        let id = source
            .assets
            .iter()
            .find(|a| a.path == path)
            .and_then(|a| a.id)
            .ok_or_else(|| std::io::Error::other("Environment has no admitted identity"))?;
        Ok(Some(if meta {
            format!("(id:\"{id}\",meta_format_version:\"1.0\",asset:Load(loader:\"kedra_scene::environment::EnvironmentLoader\",settings:()))").into_bytes()
        } else {
            source.environments[path].as_bytes().to_vec()
        }))
    }
}
impl AssetReader for Overlay {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        if let Some(bytes) = self.bytes(path, false)? {
            return Ok(Box::new(VecReader::new(bytes)));
        }
        self.inner.read(path).await
    }
    async fn read_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        if let Some(bytes) = self.bytes(path, true)? {
            return Ok(Box::new(VecReader::new(bytes)));
        }
        self.inner.read_meta(path).await
    }
    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        self.inner.read_directory(path).await
    }
    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        self.inner.is_directory(path).await
    }
}
