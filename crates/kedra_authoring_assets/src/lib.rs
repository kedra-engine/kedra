//! Source-matched native asset admission shared by authoring tools. No widgets,
//! viewport ownership, implicit builds or source publication authority is installed.
use bevy::world_serialization::WorldAsset;
mod compilation;
pub mod environments;
use bevy::{
    asset::{AssetPath, LoadFromPath, UntypedHandle},
    prelude::*,
};
pub use compilation::compile_preview;
use kedra_assets::delivery::Manifest;
use kedra_authoring::persistence::SourceSnapshot;
use std::{
    any::TypeId,
    path::{Path, PathBuf},
};

#[derive(Resource, Clone)]
/// Verified delivery pinned to a host-admitted source root. Hosts own its lifetime.
pub struct Delivery {
    /// Native source locator, never a product UI label.
    pub source: PathBuf,
    /// Already verified immutable delivery manifest.
    pub manifest: Manifest,
}
impl Delivery {
    /// Match native type and exact published identity in the existing inventory.
    /// File suffixes alone never establish an asset's native type. Admitted
    /// environment roots may have authored revisions; preview hosts must call
    /// `refresh_environment` before materialization, including after native loads.
    pub fn accepts(&self, source: &SourceSnapshot, type_id: TypeId, path: &AssetPath<'_>) -> bool {
        let loader = if type_id == TypeId::of::<Image>() {
            "bevy_image::image_loader::ImageLoader"
        } else if type_id == TypeId::of::<kedra_scene::environment::Environment>() {
            "kedra_scene::environment::EnvironmentLoader"
        } else if type_id == TypeId::of::<WorldAsset>()
            || type_id == TypeId::of::<bevy::gltf::Gltf>()
        {
            "bevy_gltf::loader::GltfLoader"
        } else if type_id == TypeId::of::<bevy_flair::style::StyleSheet>() {
            "bevy_flair_css_parser::loader::CssStyleSheetLoader"
        } else {
            return false;
        };
        let text = path.to_string();
        text.split_once("://")
            .filter(|(scope, _)| *scope == source.namespace && *scope == "game")
            .and_then(|(_, locator)| locator.split('#').next())
            .and_then(|locator| {
                self.manifest.assets.iter().find(|record| {
                    record.loader == loader
                        && (record.path == locator
                            || locator
                                == format!(
                                    "{}/asset.{}",
                                    record.id,
                                    record.path.rsplit('.').next().unwrap_or_default()
                                ))
                })
            })
            .is_some_and(|record| {
                source.assets.iter().any(|asset| {
                    asset.path == record.path
                        && asset.id == Some(record.id)
                        && ((type_id == TypeId::of::<kedra_scene::environment::Environment>()
                            && environment_source(source, &asset.path).is_ok())
                            || self.manifest.files.get(&asset.path).is_some_and(|file| {
                                file.sha256 == asset.hash && file.bytes == asset.bytes
                            }))
                })
            })
    }
    /// Check complete identity/content correspondence without writing, building or
    /// loading assets. A mismatch leaves sources and delivery unchanged.
    pub fn matches(&self, root: &Path, source: &SourceSnapshot) -> Result<(), String> {
        self.matches_inventory(root, source, true)
    }
    /// Match the source scope and every delivered non-document resource, retaining
    /// identity and byte checks for indirect dependencies such as GLTF textures.
    /// Validated environment source roots may differ from their delivered bytes;
    /// their current values are applied with `refresh_environment` before rendering.
    /// Scene/prefab source edits, additions and removals do not invalidate passive
    /// resource previews. This does not authorize document compilation: use `matches`
    /// for that, and `accepts` for each native resource request. No assets are loaded
    /// or mutated; stale, missing or foreign resources return an error.
    pub fn matches_resources(&self, root: &Path, source: &SourceSnapshot) -> Result<(), String> {
        self.matches_inventory(root, source, false)
    }
    fn matches_inventory(
        &self,
        root: &Path,
        source: &SourceSnapshot,
        documents: bool,
    ) -> Result<(), String> {
        if root != self.source || source.namespace != "game" {
            return Err("Scene View delivery belongs to a different project/source scope. Relaunch with --project-assets for this project.".into());
        }
        // The first profile supports byte-preserving document and image deliveries.
        // Processed resources need source provenance before support can be claimed.
        for document in source.documents.iter().filter(|_| documents) {
            let id = kedra_scene::document::PrefabDocument::parse(&document.source)
                .map_err(|e| e.to_string())?
                .id
                .0;
            if !self
                .manifest
                .assets
                .iter()
                .any(|a| a.path == document.path && a.id == id)
                || self
                    .manifest
                    .files
                    .get(&document.path)
                    .is_none_or(|f| f.sha256 != document.hash)
            {
                return Err(format!(
                    "Scene View source/delivery mismatch: {}. Build a matching delivery and relaunch; accepted sources were kept.",
                    document.path
                ));
            }
        }
        for asset in &self.manifest.assets {
            if !documents && asset.loader == "kedra_scene::assets::DocumentLoader" {
                continue;
            }
            if !documents
                && asset.loader == "kedra_scene::environment::EnvironmentLoader"
                && source
                    .assets
                    .iter()
                    .any(|input| input.path == asset.path && input.id == Some(asset.id))
                && environment_source(source, &asset.path).is_ok()
            {
                continue;
            }
            if source
                .documents
                .iter()
                .any(|document| document.path == asset.path)
            {
                continue;
            }
            if !source.assets.iter().any(|input| {
                input.path == asset.path
                    && input.id == Some(asset.id)
                    && self
                        .manifest
                        .files
                        .get(&asset.path)
                        .is_some_and(|file| file.sha256 == input.hash && file.bytes == input.bytes)
            }) {
                return Err(format!(
                    "Scene View delivery asset differs from the admitted inventory: {}. This profile requires byte-preserving resources; processed resources need source provenance.",
                    asset.path
                ));
            }
        }
        Ok(())
    }
}

/// Read a validated environment from the admitted source catalog. This is a
/// bounded authored root, not an imported binary: identity stays in `assets`,
/// and bytes must match its hash/length before the native schema is decoded.
pub fn environment_source(
    source: &SourceSnapshot,
    path: &str,
) -> Result<kedra_scene::environment::Environment, String> {
    let asset = source
        .assets
        .iter()
        .find(|asset| asset.path == path)
        .ok_or("Environment is outside the admitted source catalog")?;
    let text = source
        .environments
        .get(path)
        .ok_or("Environment source unavailable")?;
    if !path.ends_with(".environment.ron")
        || asset.hash != kedra_authoring::persistence::content_hash(text)
        || asset.bytes != text.len() as u64
    {
        return Err("Environment content differs from the admitted source catalog".into());
    }
    kedra_scene::environment::Environment::parse(text).map_err(|e| e.to_string())
}

/// Overlay an already loaded editor-preview environment with its accepted source
/// value. The existing source catalog and AssetServer resolve the logical path;
/// no handle registry, filesystem read or runtime delivery mutation is introduced.
/// Call before preview materialization each update so a completed native load
/// cannot replace the author's working value. Returns false while it is not loaded.
pub fn refresh_environment(
    world: &mut World,
    source: &SourceSnapshot,
    path: &str,
    value: &kedra_scene::environment::Environment,
) -> Result<bool, String> {
    environment_source(source, path)?;
    value.validate().map_err(|e| e.to_string())?;
    let logical = format!("{}://{path}", source.namespace);
    let Some(handle) = world
        .get_resource::<AssetServer>()
        .ok_or("Environment preview requires AssetServer")?
        .get_handle::<kedra_scene::environment::Environment>(logical)
    else {
        return Ok(false);
    };
    let mut assets = world
        .get_resource_mut::<Assets<kedra_scene::environment::Environment>>()
        .ok_or("Environment preview assets are not installed")?;
    if assets.get(&handle).is_none() {
        return Ok(false);
    }
    if assets.get(&handle) != Some(value) {
        assets
            .insert(handle.id(), value.clone())
            .map_err(|e| e.to_string())?;
    }
    Ok(true)
}

/// Source-limited native loading; imported model cameras/lights/animations stay off.
/// Callers must inspect `error` before accepting a compiled candidate.
pub struct Loader<'a> {
    /// Host's existing Bevy asset server.
    pub server: AssetServer,
    /// Current admitted inventory, not a second asset identity registry.
    pub source: &'a SourceSnapshot,
    /// Host-verified delivery whose source root was matched before loading.
    pub delivery: &'a Delivery,
    /// Last refused dependency; never silently treat a nil handle as success.
    pub error: Option<String>,
}
impl LoadFromPath for Loader<'_> {
    fn load_from_path_erased(
        &mut self,
        type_id: TypeId,
        path: AssetPath<'static>,
    ) -> UntypedHandle {
        let text = path.to_string();
        let valid = self.delivery.accepts(self.source, type_id, &path);
        if type_id == TypeId::of::<kedra_scene::environment::Environment>()
            && environments::accepts(self.source, &path)
        {
            return self
                .server
                .load::<kedra_scene::environment::Environment>(path)
                .untyped();
        }
        if valid && type_id == TypeId::of::<WorldAsset>() {
            self.server
                .load_builder()
                .with_settings::<bevy::gltf::GltfLoaderSettings>(|settings| {
                    settings.load_cameras = false;
                    settings.load_lights = false;
                    settings.load_animations = false;
                })
                .load::<WorldAsset>(path)
                .untyped()
        } else if valid {
            self.server.load_from_path_erased(type_id, path)
        } else {
            self.error = Some(format!(
                "Scene View dependency is outside the supported matching asset inventory or is processed without source provenance: {text}"
            ));
            UntypedHandle::Uuid {
                type_id,
                uuid: uuid::Uuid::nil(),
            }
        }
    }
}
