//! Native in-memory asset handles/catalog for CPU-only editor view regressions.
use super::*;
use bevy::asset::{AssetApp, io::memory::MemoryAssetReader};

#[derive(Resource, Default)]
struct SourceReader(MemoryAssetReader);

/// Install before AssetPlugin, so synthetic documents use the real path resolver.
pub(crate) fn install(app: &mut App) {
    let reader = MemoryAssetReader::default();
    let source = reader.clone();
    app.register_asset_source(
        "editor",
        bevy::asset::io::AssetSourceBuilder::new(move || Box::new(source.clone())),
    );
    app.insert_resource(SourceReader(reader));
}

/// Materialize the exact same documents that will be published/compiled by a test.
/// Async reads see the same bytes; there is no UUID-to-path bypass of AssetServer.
pub(crate) fn materialize(
    world: &mut World,
    documents: impl IntoIterator<Item = (String, PrefabDocument)>,
) -> Result<Handle<PrefabCatalogAsset>, Box<dyn std::error::Error>> {
    let mut handles = Vec::new();
    for (path, document) in documents {
        let relative = path
            .strip_prefix("editor://")
            .ok_or("Expected editor source path")?;
        world
            .resource::<SourceReader>()
            .0
            .root
            .insert_asset_text(std::path::Path::new(relative), &document.serialize()?);
        let handle = world.resource::<AssetServer>().load::<PrefabDocument>(path);
        world
            .resource_mut::<Assets<PrefabDocument>>()
            .insert(handle.id(), document)?;
        handles.push(handle);
    }
    Ok(world
        .resource_mut::<Assets<PrefabCatalogAsset>>()
        .add(PrefabCatalogAsset { documents: handles }))
}
