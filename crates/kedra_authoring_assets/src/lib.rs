//! Source-matched native asset admission shared by authoring tools. No widgets,
//! viewport ownership, implicit builds or source publication authority is installed.
use bevy::world_serialization::WorldAsset;
use bevy::{
    asset::{AssetPath, LoadFromPath, UntypedHandle},
    prelude::*,
};
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
    /// File suffixes alone never establish an asset's native type.
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
                        && self.manifest.files.get(&asset.path).is_some_and(|file| {
                            file.sha256 == asset.hash && file.bytes == asset.bytes
                        })
                })
            })
    }
    /// Check complete identity/content correspondence without writing, building or
    /// loading assets. A mismatch leaves sources and delivery unchanged.
    pub fn matches(&self, root: &Path, source: &SourceSnapshot) -> Result<(), String> {
        if root != self.source || source.namespace != "game" {
            return Err("Scene View delivery belongs to a different project/source scope. Relaunch with --project-assets for this project.".into());
        }
        // The first profile supports byte-preserving document and image deliveries.
        // Processed resources need source provenance before support can be claimed.
        for document in &source.documents {
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
                        .is_some_and(|file| file.sha256 == input.hash)
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
