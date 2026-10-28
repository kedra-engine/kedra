//! Inspector-only labels/icons over shared immutable authoring provenance.
use bevy::reflect::{ParsedPath, PartialReflect};
use kedra_inspector::binding::ValueMetadata;
pub(crate) struct Metadata<'a> {
    source: kedra_authoring::inspection::Metadata<'a>,
    delivery: Option<&'a kedra_authoring_assets::Delivery>,
    model_choices: &'a [String],
}
impl<'a> Metadata<'a> {
    pub(crate) fn new(source: kedra_authoring::inspection::Metadata<'a>) -> Self {
        Self {
            source,
            delivery: None,
            model_choices: &[],
        }
    }
    pub(crate) fn with_delivery(
        mut self,
        delivery: Option<&'a kedra_authoring_assets::Delivery>,
    ) -> Self {
        self.delivery = delivery;
        self
    }
    pub(crate) fn with_model_choices(mut self, paths: &'a [String]) -> Self {
        self.model_choices = paths;
        self
    }
}
impl ValueMetadata for Metadata<'_> {
    fn asset_icon(&self, ty: std::any::TypeId) -> kedra_ui::fields::AssetIcon {
        use kedra_ui::fields::AssetIcon;
        if ty == std::any::TypeId::of::<bevy::prelude::Image>() {
            AssetIcon::Image
        } else if ty == std::any::TypeId::of::<bevy::world_serialization::WorldAsset>() {
            AssetIcon::Model
        } else if ty == std::any::TypeId::of::<kedra_scene::environment::Environment>() {
            AssetIcon::Environment
        } else {
            AssetIcon::File
        }
    }
    fn asset_path(&self, handle: &bevy::asset::UntypedHandle) -> Option<String> {
        self.source
            .compiled
            .dependency_path(handle)
            .map(ToString::to_string)
    }
    fn asset_choices(&self, asset_type: std::any::TypeId) -> Vec<String> {
        let Some(delivery) = self.delivery else {
            return Vec::new();
        };
        if asset_type == std::any::TypeId::of::<bevy::world_serialization::WorldAsset>() {
            return self.model_choices.to_vec();
        }
        delivery
            .manifest
            .assets
            .iter()
            .filter_map(|record| {
                let text = format!("{}://{}", self.source.scope.source.namespace, record.path);
                let path = bevy::asset::AssetPath::try_parse(&text).ok()?;
                delivery
                    .accepts(&self.source.scope.source, asset_type, &path)
                    .then_some(text)
            })
            .collect()
    }
    fn describe(&self, path: Option<&ParsedPath>) -> Option<String> {
        let mut origin = &self.source.component.origin;
        if let Some(path) = path {
            let accesses: Vec<_> = path.0.iter().map(|a| &a.access).collect();
            for field in &self.source.component.fields {
                let field_path = ParsedPath::parse(&field.path).ok()?;
                let field_access: Vec<_> = field_path.0.iter().map(|a| &a.access).collect();
                if accesses.starts_with(&field_access) {
                    origin = &field.origin;
                } else if field_access.starts_with(&accesses) {
                    return Some("Mixed source provenance; inspect child fields".into());
                }
            }
        }
        let kind = if origin.document != self.source.document {
            "Inherited prefab"
        } else if self.source.nested {
            "Instance override"
        } else if self.source.component.origin.document != self.source.document {
            "Variant override"
        } else {
            "Local source"
        };
        Some(format!(
            "{kind}: {}",
            self.source
                .scope
                .path(origin.document)
                .unwrap_or("Source path unavailable")
        ))
    }
    fn limitation(&self, value: &dyn PartialReflect, _: Option<&ParsedPath>) -> Option<String> {
        if value.try_downcast_ref::<bevy::prelude::Entity>().is_some()
            || value
                .try_downcast_ref::<Option<bevy::prelude::Entity>>()
                .is_some()
        {
            Some("Symbolic source reference needs a source-reference presentation; effective optional presence is not authored intent".into())
        } else {
            None
        }
    }
}
