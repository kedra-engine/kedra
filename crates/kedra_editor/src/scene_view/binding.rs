//! Explicit startup delivery, pinned to a project source root. No implicit builds.
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

#[derive(Resource)]
pub(crate) struct Delivery {
    pub source: PathBuf,
    pub manifest: Manifest,
}
impl Delivery {
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

pub(super) struct Loader<'a> {
    pub server: AssetServer,
    pub source: &'a SourceSnapshot,
    pub delivery: &'a Delivery,
    pub error: Option<String>,
}
impl LoadFromPath for Loader<'_> {
    fn load_from_path_erased(
        &mut self,
        type_id: TypeId,
        path: AssetPath<'static>,
    ) -> UntypedHandle {
        let text = path.to_string();
        let record = text
            .split_once("://")
            .filter(|(scope, _)| *scope == "game")
            .and_then(|(_, locator)| {
                let locator = locator.split('#').next()?;
                self.delivery.manifest.assets.iter().find(|a| {
                    a.path == locator
                        || locator
                            == format!(
                                "{}/asset.{}",
                                a.id,
                                a.path.rsplit('.').next().unwrap_or_default()
                            )
                })
            });
        let valid = (type_id == TypeId::of::<Image>()
            || type_id == TypeId::of::<kedra_scene::environment::Environment>()
            || type_id == TypeId::of::<WorldAsset>()
            || type_id == TypeId::of::<bevy_flair::style::StyleSheet>())
            && record.is_some_and(|record| {
                self.source.assets.iter().any(|a| {
                    a.path == record.path
                        && a.id == Some(record.id)
                        && self
                            .delivery
                            .manifest
                            .files
                            .get(&a.path)
                            .is_some_and(|f| f.sha256 == a.hash)
                })
            });
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
